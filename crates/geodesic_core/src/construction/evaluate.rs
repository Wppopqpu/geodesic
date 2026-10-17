use std::collections::BTreeMap;

use crate::projective::{line_through, parallel_through, perpendicular_through};
use crate::{EvaluationError, Label, ProjectiveError, ProjectiveLine, ProjectivePoint};

use super::{Construction, GeometricObject, GeometrySnapshot, ObjectDefinition};

impl Construction {
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
            ObjectDefinition::LineThrough { first, second } => GeometricObject::Line(
                line_through(
                    self.point_dependency(label, first, objects)?,
                    self.point_dependency(label, second, objects)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::Intersection { first, second } => GeometricObject::Point(
                self.line_dependency(label, first, objects)?
                    .intersection(self.line_dependency(label, second, objects)?)
                    .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::ParallelThrough { point, line } => GeometricObject::Line(
                parallel_through(
                    self.point_dependency(label, point, objects)?,
                    self.line_dependency(label, line, objects)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::PerpendicularThrough { point, line } => GeometricObject::Line(
                perpendicular_through(
                    self.point_dependency(label, point, objects)?,
                    self.line_dependency(label, line, objects)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
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
