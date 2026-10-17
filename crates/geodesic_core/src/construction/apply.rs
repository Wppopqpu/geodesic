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
                self.require_line(&first)?;
                self.require_line(&second)?;
                self.insert(label, ObjectDefinition::Intersection { first, second })
            }
            GeoCommand::AddParallelThrough { label, point, line } => {
                self.require_point(&point)?;
                self.require_line(&line)?;
                self.insert(label, ObjectDefinition::ParallelThrough { point, line })
            }
            GeoCommand::AddPerpendicularThrough { label, point, line } => {
                self.require_point(&point)?;
                self.require_line(&line)?;
                self.insert(
                    label,
                    ObjectDefinition::PerpendicularThrough { point, line },
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
            Some(ObjectDefinition::FreePoint(_) | ObjectDefinition::Intersection { .. }) => Ok(()),
            Some(_) => Err(CommandError::WrongObjectKind {
                label: label.clone(),
                expected: "point",
            }),
            None => Err(CommandError::UnknownLabel {
                label: label.clone(),
            }),
        }
    }

    fn require_line(&self, label: &Label) -> Result<(), CommandError> {
        match self.definitions.get(label) {
            Some(
                ObjectDefinition::FreeLine(_)
                | ObjectDefinition::LineThrough { .. }
                | ObjectDefinition::ParallelThrough { .. }
                | ObjectDefinition::PerpendicularThrough { .. },
            ) => Ok(()),
            Some(_) => Err(CommandError::WrongObjectKind {
                label: label.clone(),
                expected: "line",
            }),
            None => Err(CommandError::UnknownLabel {
                label: label.clone(),
            }),
        }
    }
}
