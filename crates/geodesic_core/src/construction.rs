use std::collections::{BTreeMap, btree_map::Entry};

use crate::{
    HistoryError, Label, ProjectiveError, ProjectiveLine, ProjectivePoint,
    error::{CommandError, EvaluationError},
    projective::{line_through, parallel_through, perpendicular_through},
};

/// A declared construction rule for a geometric object.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectDefinition {
    /// An independently supplied point.
    FreePoint(ProjectivePoint),
    /// An independently supplied line.
    FreeLine(ProjectiveLine),
    /// A line derived from two existing points.
    LineThrough { first: Label, second: Label },
    /// A point derived from two existing lines.
    Intersection { first: Label, second: Label },
    /// A line parallel to an existing line through an existing point.
    ParallelThrough { point: Label, line: Label },
    /// A Euclidean-affine perpendicular to an existing line through an existing point.
    PerpendicularThrough { point: Label, line: Label },
}

/// An evaluated geometric value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeometricObject {
    /// A point in the complex projective plane.
    Point(ProjectivePoint),
    /// A line in the complex projective plane.
    Line(ProjectiveLine),
}

/// A command accepted by the core construction boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum GeoCommand {
    /// Adds a free point.
    AddPoint {
        label: Label,
        point: ProjectivePoint,
    },
    /// Moves an existing free point to a new projective position.
    MovePoint {
        label: Label,
        point: ProjectivePoint,
    },
    /// Adds a free line.
    AddLine { label: Label, line: ProjectiveLine },
    /// Adds a line derived from two points.
    AddLineThrough {
        label: Label,
        first: Label,
        second: Label,
    },
    /// Adds an intersection point derived from two lines.
    AddIntersection {
        label: Label,
        first: Label,
        second: Label,
    },
    /// Adds a line parallel to an existing line through an existing point.
    AddParallelThrough {
        label: Label,
        point: Label,
        line: Label,
    },
    /// Adds a Euclidean-affine perpendicular through an existing point.
    AddPerpendicularThrough {
        label: Label,
        point: Label,
        line: Label,
    },
    /// Removes a definition and its evaluated value.
    Remove { label: Label },
}

/// The ordered, labeled graph of construction definitions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Construction {
    definitions: BTreeMap<Label, ObjectDefinition>,
    undo_stack: Vec<BTreeMap<Label, ObjectDefinition>>,
    redo_stack: Vec<BTreeMap<Label, ObjectDefinition>>,
}

/// An immutable evaluated view used by renderers and adapters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeometrySnapshot {
    objects: BTreeMap<Label, GeometricObject>,
}

impl Construction {
    /// Creates an empty construction.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

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
    /// Returns [`HistoryError::NothingToUndo`] when no command can be undone.
    pub fn undo(&mut self) -> Result<(), HistoryError> {
        let prior = self.undo_stack.pop().ok_or(HistoryError::NothingToUndo)?;
        self.redo_stack
            .push(std::mem::replace(&mut self.definitions, prior));
        Ok(())
    }

    /// Restores the construction state most recently reverted by [`Self::undo`].
    ///
    /// # Errors
    ///
    /// Returns [`HistoryError::NothingToRedo`] when no command can be redone.
    pub fn redo(&mut self) -> Result<(), HistoryError> {
        let next = self.redo_stack.pop().ok_or(HistoryError::NothingToRedo)?;
        self.undo_stack
            .push(std::mem::replace(&mut self.definitions, next));
        Ok(())
    }

    /// Returns whether a successful command is available to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns whether a reverted command is available to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
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
            GeoCommand::Remove { label } => {
                if self.definitions.remove(&label).is_some() {
                    Ok(())
                } else {
                    Err(CommandError::UnknownLabel { label })
                }
            }
        }
    }

    /// Evaluates all construction definitions into an immutable snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError`] when a dependency is invalid or a projective
    /// construction is undefined.
    pub fn evaluate(&self) -> Result<GeometrySnapshot, EvaluationError> {
        let mut objects = BTreeMap::new();
        for label in self.definitions.keys() {
            let object = self.evaluate_label(label, &mut objects)?;
            objects.insert(label.clone(), object);
        }
        Ok(GeometrySnapshot { objects })
    }

    /// Iterates over definitions in deterministic label order.
    pub fn definitions(&self) -> impl Iterator<Item = (&Label, &ObjectDefinition)> {
        self.definitions.iter()
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

    fn evaluate_label(
        &self,
        label: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
    ) -> Result<GeometricObject, EvaluationError> {
        if let Some(object) = objects.get(label) {
            return Ok(*object);
        }

        let definition =
            self.definitions
                .get(label)
                .ok_or_else(|| EvaluationError::MissingDependency {
                    label: label.clone(),
                    dependency: label.clone(),
                })?;

        let object = match definition {
            ObjectDefinition::FreePoint(point) => GeometricObject::Point(*point),
            ObjectDefinition::FreeLine(line) => GeometricObject::Line(*line),
            ObjectDefinition::LineThrough { first, second } => {
                let first = self.point_dependency(label, first, objects)?;
                let second = self.point_dependency(label, second, objects)?;
                let line = line_through(first, second)
                    .map_err(|source| projective_error(label, source))?;
                GeometricObject::Line(line)
            }
            ObjectDefinition::Intersection { first, second } => {
                let first = self.line_dependency(label, first, objects)?;
                let second = self.line_dependency(label, second, objects)?;
                let point = first
                    .intersection(second)
                    .map_err(|source| projective_error(label, source))?;
                GeometricObject::Point(point)
            }
            ObjectDefinition::ParallelThrough { point, line } => {
                let point = self.point_dependency(label, point, objects)?;
                let line = self.line_dependency(label, line, objects)?;
                let line = parallel_through(point, line)
                    .map_err(|source| projective_error(label, source))?;
                GeometricObject::Line(line)
            }
            ObjectDefinition::PerpendicularThrough { point, line } => {
                let point = self.point_dependency(label, point, objects)?;
                let line = self.line_dependency(label, line, objects)?;
                let line = perpendicular_through(point, line)
                    .map_err(|source| projective_error(label, source))?;
                GeometricObject::Line(line)
            }
        };
        objects.insert(label.clone(), object);
        Ok(object)
    }

    fn point_dependency(
        &self,
        label: &Label,
        dependency: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
    ) -> Result<ProjectivePoint, EvaluationError> {
        match self.evaluate_label(dependency, objects)? {
            GeometricObject::Point(point) => Ok(point),
            GeometricObject::Line(_) => Err(EvaluationError::WrongDependencyKind {
                label: label.clone(),
                dependency: dependency.clone(),
                expected: "point",
            }),
        }
    }

    fn line_dependency(
        &self,
        label: &Label,
        dependency: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
    ) -> Result<ProjectiveLine, EvaluationError> {
        match self.evaluate_label(dependency, objects)? {
            GeometricObject::Line(line) => Ok(line),
            GeometricObject::Point(_) => Err(EvaluationError::WrongDependencyKind {
                label: label.clone(),
                dependency: dependency.clone(),
                expected: "line",
            }),
        }
    }
}

impl GeometrySnapshot {
    /// Looks up an evaluated object by its canonical label.
    #[must_use]
    pub fn object(&self, label: &Label) -> Option<GeometricObject> {
        self.objects.get(label).copied()
    }

    /// Iterates over evaluated objects in deterministic label order.
    pub fn iter(&self) -> impl Iterator<Item = (&Label, &GeometricObject)> {
        self.objects.iter()
    }
}

fn projective_error(label: &Label, source: ProjectiveError) -> EvaluationError {
    EvaluationError::Projective {
        label: label.clone(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Label, ProjectiveLine, ProjectivePoint,
        construction::{Construction, GeoCommand, GeometricObject},
    };

    fn label(value: &str) -> Label {
        Label::parse(value).expect("test labels are valid")
    }

    #[test]
    fn evaluates_a_line_and_intersection_from_dependencies() {
        let mut construction = Construction::new();
        construction
            .apply(GeoCommand::AddPoint {
                label: label("point_a"),
                point: ProjectivePoint::real_affine(0.0, 0.0),
            })
            .expect("add point a");
        construction
            .apply(GeoCommand::AddPoint {
                label: label("point_b"),
                point: ProjectivePoint::real_affine(2.0, 2.0),
            })
            .expect("add point b");
        construction
            .apply(GeoCommand::AddLineThrough {
                label: label("diagonal"),
                first: label("point_a"),
                second: label("point_b"),
            })
            .expect("add diagonal");
        construction
            .apply(GeoCommand::AddLine {
                label: label("horizontal"),
                line: ProjectiveLine::new(0.0.into(), 1.0.into(), (-1.0).into())
                    .expect("valid horizontal line"),
            })
            .expect("add horizontal line");
        construction
            .apply(GeoCommand::AddIntersection {
                label: label("crossing"),
                first: label("diagonal"),
                second: label("horizontal"),
            })
            .expect("add intersection");

        let snapshot = construction.evaluate().expect("construction is valid");
        let Some(GeometricObject::Point(crossing)) = snapshot.object(&label("crossing")) else {
            panic!("intersection should evaluate to a point");
        };

        assert_eq!(
            crossing.renderability(),
            crate::Renderability::Renderable(crate::RealAffinePoint { x: 1.0, y: 1.0 })
        );
    }

    #[test]
    fn rejects_duplicate_public_labels() {
        let mut construction = Construction::new();
        let command = GeoCommand::AddPoint {
            label: label("point_a"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        };

        construction
            .apply(command.clone())
            .expect("first label is free");
        let error = construction
            .apply(command)
            .expect_err("second label conflicts");

        assert!(matches!(error, crate::CommandError::DuplicateLabel { .. }));
    }

    #[test]
    fn restores_commands_through_undo_and_redo() {
        let mut construction = Construction::new();
        construction
            .apply(GeoCommand::AddPoint {
                label: label("point_a"),
                point: ProjectivePoint::real_affine(0.0, 0.0),
            })
            .expect("command");
        assert!(construction.can_undo());
        construction.undo().expect("undo");
        assert_eq!(
            construction
                .evaluate()
                .expect("empty construction")
                .iter()
                .count(),
            0
        );
        assert!(construction.can_redo());
        construction.redo().expect("redo");
        assert_eq!(
            construction
                .evaluate()
                .expect("restored point")
                .iter()
                .count(),
            1
        );
    }

    #[test]
    fn moves_a_free_point_and_records_the_change() {
        let mut construction = Construction::new();
        construction
            .apply(GeoCommand::AddPoint {
                label: label("point_a"),
                point: ProjectivePoint::real_affine(0.0, 0.0),
            })
            .expect("create");
        construction
            .apply(GeoCommand::MovePoint {
                label: label("point_a"),
                point: ProjectivePoint::real_affine(2.0, 3.0),
            })
            .expect("move");
        let Some(GeometricObject::Point(point)) = construction
            .evaluate()
            .expect("evaluation")
            .object(&label("point_a"))
        else {
            panic!("point")
        };
        assert_eq!(
            point.renderability(),
            crate::Renderability::Renderable(crate::RealAffinePoint { x: 2.0, y: 3.0 })
        );
        construction.undo().expect("undo move");
        let Some(GeometricObject::Point(point)) = construction
            .evaluate()
            .expect("evaluation")
            .object(&label("point_a"))
        else {
            panic!("point")
        };
        assert_eq!(
            point.renderability(),
            crate::Renderability::Renderable(crate::RealAffinePoint { x: 0.0, y: 0.0 })
        );
    }
}
