use std::collections::btree_map::Entry;

use crate::{CommandError, Label};

use super::{Construction, GeoCommand, ObjectDefinition};

impl Construction {
    /// Applies one command after validating all referenced labels and object kinds.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] when a label conflicts, a referenced object is
    /// missing, or a command expects the wrong geometric-object kind.
    pub fn apply(&mut self, command: GeoCommand) -> Result<(), CommandError> {
        let before = self.definitions.clone();
        self.apply_without_history(command)?;
        self.undo_stack.push(before);
        self.redo_stack.clear();
        Ok(())
    }

    /// Restores the construction state before the most recent successful command.
    ///
    /// # Errors
    ///
    /// Returns [`crate::HistoryError::NothingToUndo`] when no command can be undone.
    pub fn undo(&mut self) -> Result<(), crate::HistoryError> {
        let prior = self
            .undo_stack
            .pop()
            .ok_or(crate::HistoryError::NothingToUndo)?;
        self.redo_stack
            .push(std::mem::replace(&mut self.definitions, prior));
        Ok(())
    }

    /// Restores the construction state most recently reverted by [`Self::undo`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::HistoryError::NothingToRedo`] when no command can be redone.
    pub fn redo(&mut self) -> Result<(), crate::HistoryError> {
        let next = self
            .redo_stack
            .pop()
            .ok_or(crate::HistoryError::NothingToRedo)?;
        self.undo_stack
            .push(std::mem::replace(&mut self.definitions, next));
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "All command variants share one atomic history boundary."
    )]
    fn apply_without_history(&mut self, command: GeoCommand) -> Result<(), CommandError> {
        match command {
            GeoCommand::AddPoint { label, point } => {
                self.insert(label, ObjectDefinition::FreePoint(point))
            }
            GeoCommand::MovePoint { label, point } => match self.definitions.get_mut(&label) {
                Some(ObjectDefinition::FreePoint(existing)) => {
                    *existing = point;
                    Ok(())
                }
                Some(_) => Err(CommandError::WrongObjectKind {
                    label,
                    expected: "free point",
                }),
                None => Err(CommandError::UnknownLabel { label }),
            },
            GeoCommand::MoveLine { label, line } => match self.definitions.get_mut(&label) {
                Some(ObjectDefinition::FreeLine(existing)) => {
                    *existing = line;
                    Ok(())
                }
                Some(_) => Err(CommandError::WrongObjectKind {
                    label,
                    expected: "free line",
                }),
                None => Err(CommandError::UnknownLabel { label }),
            },
            GeoCommand::AddLine { label, line } => {
                self.insert(label, ObjectDefinition::FreeLine(line))
            }
            GeoCommand::AddLineThrough {
                label,
                first,
                second,
            } => {
                self.require_point(&first)?;
                self.require_point(&second)?;
                self.insert(label, ObjectDefinition::LineThrough { first, second })
            }
            GeoCommand::AddIntersection {
                label,
                first,
                second,
            } => {
                self.require_intersectable_locus(&first)?;
                self.require_intersectable_locus(&second)?;
                self.insert(label, ObjectDefinition::Intersection { first, second })
            }
            GeoCommand::AddParallelThrough { label, point, line } => {
                self.require_point(&point)?;
                self.require_supporting_line(&line)?;
                self.insert(label, ObjectDefinition::ParallelThrough { point, line })
            }
            GeoCommand::AddPerpendicularThrough { label, point, line } => {
                self.require_point(&point)?;
                self.require_supporting_line(&line)?;
                self.insert(
                    label,
                    ObjectDefinition::PerpendicularThrough { point, line },
                )
            }
            GeoCommand::AddSegment { label, start, end } => {
                self.require_real_affine_point(&start)?;
                self.require_real_affine_point(&end)?;
                self.insert(label, ObjectDefinition::Segment { start, end })
            }
            GeoCommand::AddRay {
                label,
                origin,
                through,
            } => {
                self.require_real_affine_point(&origin)?;
                self.require_real_affine_point(&through)?;
                self.insert(label, ObjectDefinition::Ray { origin, through })
            }
            GeoCommand::AddVector { label, start, end } => {
                self.require_real_affine_point(&start)?;
                self.require_real_affine_point(&end)?;
                self.insert(label, ObjectDefinition::Vector { start, end })
            }
            GeoCommand::AddMidpoint {
                label,
                first,
                second,
            } => {
                self.require_real_affine_point(&first)?;
                self.require_real_affine_point(&second)?;
                self.insert(label, ObjectDefinition::Midpoint { first, second })
            }
            GeoCommand::AddPerpendicularBisector {
                label,
                first,
                second,
            } => {
                self.require_real_affine_point(&first)?;
                self.require_real_affine_point(&second)?;
                self.insert(
                    label,
                    ObjectDefinition::PerpendicularBisector { first, second },
                )
            }
            GeoCommand::AddAngleBisector {
                label,
                first,
                vertex,
                second,
            } => {
                self.require_real_affine_point(&first)?;
                self.require_real_affine_point(&vertex)?;
                self.require_real_affine_point(&second)?;
                self.insert(
                    label,
                    ObjectDefinition::AngleBisector {
                        first,
                        vertex,
                        second,
                    },
                )
            }
            GeoCommand::Remove { label } => self
                .definitions
                .remove(&label)
                .map_or_else(|| Err(CommandError::UnknownLabel { label }), |_| Ok(())),
        }
    }

    fn insert(&mut self, label: Label, definition: ObjectDefinition) -> Result<(), CommandError> {
        match self.definitions.entry(label) {
            Entry::Occupied(entry) => Err(CommandError::DuplicateLabel {
                label: entry.key().clone(),
            }),
            Entry::Vacant(entry) => {
                entry.insert(definition);
                Ok(())
            }
        }
    }

    fn require_point(&self, label: &Label) -> Result<(), CommandError> {
        match self.definitions.get(label) {
            Some(
                ObjectDefinition::FreePoint(_)
                | ObjectDefinition::Intersection { .. }
                | ObjectDefinition::Midpoint { .. },
            ) => Ok(()),
            Some(_) => Err(CommandError::WrongObjectKind {
                label: label.clone(),
                expected: "point",
            }),
            None => Err(CommandError::UnknownLabel {
                label: label.clone(),
            }),
        }
    }

    fn require_supporting_line(&self, label: &Label) -> Result<(), CommandError> {
        match self.definitions.get(label) {
            Some(
                ObjectDefinition::FreeLine(_)
                | ObjectDefinition::LineThrough { .. }
                | ObjectDefinition::ParallelThrough { .. }
                | ObjectDefinition::PerpendicularThrough { .. }
                | ObjectDefinition::PerpendicularBisector { .. }
                | ObjectDefinition::AngleBisector { .. }
                | ObjectDefinition::Segment { .. }
                | ObjectDefinition::Ray { .. },
            ) => Ok(()),
            Some(_) => Err(CommandError::WrongObjectKind {
                label: label.clone(),
                expected: "line, segment, or ray",
            }),
            None => Err(CommandError::UnknownLabel {
                label: label.clone(),
            }),
        }
    }

    fn require_intersectable_locus(&self, label: &Label) -> Result<(), CommandError> {
        self.require_supporting_line(label)
    }

    fn require_real_affine_point(&self, label: &Label) -> Result<(), CommandError> {
        self.require_point(label)?;
        let point = self
            .evaluate()
            .ok()
            .and_then(|snapshot| snapshot.object(label))
            .and_then(|object| match object {
                super::GeometricObject::Point(point) => Some(point),
                super::GeometricObject::Line(_)
                | super::GeometricObject::Segment(_)
                | super::GeometricObject::Ray(_)
                | super::GeometricObject::Vector(_) => None,
            });
        match point.and_then(|point| crate::real_affine_point(point).ok()) {
            Some(_) => Ok(()),
            None => Err(CommandError::NotRealAffinePoint {
                label: label.clone(),
            }),
        }
    }
}
