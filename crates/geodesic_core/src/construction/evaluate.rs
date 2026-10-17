use std::collections::BTreeMap;

use crate::projective::{
    angle_bisector, line_through, midpoint, parallel_through, perpendicular_bisector,
    perpendicular_through, real_affine_point,
};
use crate::{
    EvaluationError, Label, ProjectiveError, ProjectiveLine, ProjectivePoint, STANDARD_PRECISION,
};

use super::{
    AffineRay, AffineSegment, AffineVector, Construction, GeometricObject, GeometrySnapshot,
    ObjectDefinition,
};

impl Construction {
    /// Evaluates all construction definitions into an immutable snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError`] for structurally invalid construction data.
    /// Recoverable geometric degeneracies are recorded per object and omitted
    /// from [`GeometrySnapshot::iter`].
    pub fn evaluate(&self) -> Result<GeometrySnapshot, EvaluationError> {
        let mut objects = BTreeMap::new();
        let mut undefined = BTreeMap::new();
        for label in self.definitions.keys() {
            if objects.contains_key(label) || undefined.contains_key(label) {
                continue;
            }
            match self.evaluate_label(label, &mut objects, &mut undefined) {
                Ok(object) => {
                    objects.insert(label.clone(), object);
                }
                Err(error) if is_recoverably_undefined(&error) => {
                    undefined.insert(label.clone(), error);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(GeometrySnapshot { objects, undefined })
    }

    fn evaluate_label(
        &self,
        label: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
        undefined: &mut BTreeMap<Label, EvaluationError>,
    ) -> Result<GeometricObject, EvaluationError> {
        if let Some(object) = objects.get(label) {
            return Ok(*object);
        }
        if let Some(error) = undefined.get(label) {
            return Err(error.clone());
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
                    self.point_dependency(label, first, objects, undefined)?,
                    self.point_dependency(label, second, objects, undefined)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::Intersection { first, second } => GeometricObject::Point(
                self.line_dependency(label, first, objects, undefined)?
                    .intersection(self.line_dependency(label, second, objects, undefined)?)
                    .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::ParallelThrough { point, line } => GeometricObject::Line(
                parallel_through(
                    self.point_dependency(label, point, objects, undefined)?,
                    self.line_dependency(label, line, objects, undefined)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::PerpendicularThrough { point, line } => GeometricObject::Line(
                perpendicular_through(
                    self.point_dependency(label, point, objects, undefined)?,
                    self.line_dependency(label, line, objects, undefined)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::Segment { start, end } => {
                let (start, end) = self.affine_pair(label, start, end, objects, undefined)?;
                GeometricObject::Segment(AffineSegment { start, end })
            }
            ObjectDefinition::Ray { origin, through } => {
                let (origin, through) =
                    self.affine_pair(label, origin, through, objects, undefined)?;
                GeometricObject::Ray(AffineRay { origin, through })
            }
            ObjectDefinition::Vector { start, end } => {
                let (start, end) = self.affine_pair(label, start, end, objects, undefined)?;
                GeometricObject::Vector(AffineVector { start, end })
            }
            ObjectDefinition::Midpoint { first, second } => GeometricObject::Point(
                midpoint(
                    self.point_dependency(label, first, objects, undefined)?,
                    self.point_dependency(label, second, objects, undefined)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::PerpendicularBisector { first, second } => GeometricObject::Line(
                perpendicular_bisector(
                    self.point_dependency(label, first, objects, undefined)?,
                    self.point_dependency(label, second, objects, undefined)?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::AngleBisector {
                first,
                vertex,
                second,
            } => GeometricObject::Line(
                angle_bisector(
                    self.point_dependency(label, first, objects, undefined)?,
                    self.point_dependency(label, vertex, objects, undefined)?,
                    self.point_dependency(label, second, objects, undefined)?,
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
        undefined: &mut BTreeMap<Label, EvaluationError>,
    ) -> Result<ProjectivePoint, EvaluationError> {
        match self
            .evaluate_label(dependency, objects, undefined)
            .map_err(|error| dependency_error(label, dependency, error))?
        {
            GeometricObject::Point(point) => Ok(point),
            GeometricObject::Line(_)
            | GeometricObject::Segment(_)
            | GeometricObject::Ray(_)
            | GeometricObject::Vector(_) => Err(EvaluationError::WrongDependencyKind {
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
        undefined: &mut BTreeMap<Label, EvaluationError>,
    ) -> Result<ProjectiveLine, EvaluationError> {
        match self
            .evaluate_label(dependency, objects, undefined)
            .map_err(|error| dependency_error(label, dependency, error))?
        {
            GeometricObject::Line(line) => Ok(line),
            GeometricObject::Point(_)
            | GeometricObject::Segment(_)
            | GeometricObject::Ray(_)
            | GeometricObject::Vector(_) => Err(EvaluationError::WrongDependencyKind {
                label: label.clone(),
                dependency: dependency.clone(),
                expected: "line",
            }),
        }
    }

    fn affine_pair(
        &self,
        label: &Label,
        first: &Label,
        second: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
        undefined: &mut BTreeMap<Label, EvaluationError>,
    ) -> Result<(crate::RealAffinePoint, crate::RealAffinePoint), EvaluationError> {
        let first = real_affine_point(self.point_dependency(label, first, objects, undefined)?)
            .map_err(|source| projective_error(label, source))?;
        let second = real_affine_point(self.point_dependency(label, second, objects, undefined)?)
            .map_err(|source| projective_error(label, source))?;
        if first.distance_to(second) <= STANDARD_PRECISION {
            return Err(projective_error(
                label,
                ProjectiveError::CoincidentAffinePoints,
            ));
        }
        Ok((first, second))
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

    /// Iterates over definitions currently undefined through recoverable geometry.
    pub fn undefined(&self) -> impl Iterator<Item = (&Label, &EvaluationError)> {
        self.undefined.iter()
    }
}

fn projective_error(label: &Label, source: ProjectiveError) -> EvaluationError {
    EvaluationError::Projective {
        label: label.clone(),
        source,
    }
}

fn is_recoverably_undefined(error: &EvaluationError) -> bool {
    matches!(
        error,
        EvaluationError::Projective { .. } | EvaluationError::UndefinedDependency { .. }
    )
}

fn dependency_error(label: &Label, dependency: &Label, error: EvaluationError) -> EvaluationError {
    if is_recoverably_undefined(&error) {
        EvaluationError::UndefinedDependency {
            label: label.clone(),
            dependency: dependency.clone(),
        }
    } else {
        error
    }
}
