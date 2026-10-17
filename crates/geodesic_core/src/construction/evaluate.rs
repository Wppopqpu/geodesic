use std::collections::BTreeMap;

use crate::projective::{
    angle_bisector, line_angle_bisector, line_through, midpoint, parallel_through,
    perpendicular_bisector, perpendicular_through, real_affine_point,
};
use crate::{
    EvaluationError, Label, ProjectiveError, ProjectiveLine, ProjectivePoint, Renderability,
    STANDARD_PRECISION,
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

    #[allow(
        clippy::too_many_lines,
        reason = "The construction-definition match is deliberately exhaustive."
    )]
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
            ObjectDefinition::Intersection { first, second } => {
                let first_locus =
                    self.supporting_locus_dependency(label, first, objects, undefined)?;
                let second_locus =
                    self.supporting_locus_dependency(label, second, objects, undefined)?;
                let intersection = first_locus
                    .line()
                    .and_then(|first_line| {
                        second_locus
                            .line()
                            .and_then(|second_line| first_line.intersection(second_line))
                    })
                    .map_err(|source| projective_error(label, source))?;
                let affine = real_affine_point(intersection)
                    .map_err(|source| projective_error(label, source))?;
                if !first_locus.contains(affine) || !second_locus.contains(affine) {
                    return Err(projective_error(
                        label,
                        ProjectiveError::IntersectionOutsideLocus,
                    ));
                }
                GeometricObject::Point(intersection)
            }
            ObjectDefinition::ParallelThrough { point, line } => GeometricObject::Line(
                parallel_through(
                    self.point_dependency(label, point, objects, undefined)?,
                    self.supporting_locus_dependency(label, line, objects, undefined)?
                        .line()
                        .map_err(|source| projective_error(label, source))?,
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::PerpendicularThrough { point, line } => GeometricObject::Line(
                perpendicular_through(
                    self.point_dependency(label, point, objects, undefined)?,
                    self.supporting_locus_dependency(label, line, objects, undefined)?
                        .line()
                        .map_err(|source| projective_error(label, source))?,
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
            ObjectDefinition::LineAngleBisector {
                first,
                second,
                branch,
                ..
            } => GeometricObject::Line(
                line_angle_bisector(
                    self.supporting_locus_dependency(label, first, objects, undefined)?
                        .line()
                        .map_err(|source| projective_error(label, source))?,
                    self.supporting_locus_dependency(label, second, objects, undefined)?
                        .line()
                        .map_err(|source| projective_error(label, source))?,
                    matches!(branch, super::LineAngleBisectorBranch::Difference),
                )
                .map_err(|source| projective_error(label, source))?,
            ),
            ObjectDefinition::PointOnLocus { locus, parameter } => GeometricObject::Point(
                self.supporting_locus_dependency(label, locus, objects, undefined)?
                    .point_at(*parameter)
                    .map_err(|source| projective_error(label, source))?
                    .projective(),
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

    fn supporting_locus_dependency(
        &self,
        label: &Label,
        dependency: &Label,
        objects: &mut BTreeMap<Label, GeometricObject>,
        undefined: &mut BTreeMap<Label, EvaluationError>,
    ) -> Result<SupportingLocus, EvaluationError> {
        let object = self
            .evaluate_label(dependency, objects, undefined)
            .map_err(|error| dependency_error(label, dependency, error))?;
        SupportingLocus::from_object(object).ok_or_else(|| EvaluationError::WrongDependencyKind {
            label: label.clone(),
            dependency: dependency.clone(),
            expected: "line, segment, or ray",
        })
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

#[derive(Clone, Copy)]
enum SupportingLocus {
    Line(ProjectiveLine),
    Segment(AffineSegment),
    Ray(AffineRay),
}

impl SupportingLocus {
    fn from_object(object: GeometricObject) -> Option<Self> {
        match object {
            GeometricObject::Line(line) => Some(Self::Line(line)),
            GeometricObject::Segment(segment) => Some(Self::Segment(segment)),
            GeometricObject::Ray(ray) => Some(Self::Ray(ray)),
            GeometricObject::Point(_) | GeometricObject::Vector(_) => None,
        }
    }

    fn line(self) -> Result<ProjectiveLine, ProjectiveError> {
        match self {
            Self::Line(line) => Ok(line),
            Self::Segment(segment) => {
                line_through(segment.start.projective(), segment.end.projective())
            }
            Self::Ray(ray) => line_through(ray.origin.projective(), ray.through.projective()),
        }
    }

    fn contains(self, point: crate::RealAffinePoint) -> bool {
        let (start, end, minimum, maximum) = match self {
            Self::Line(_) => return true,
            Self::Segment(segment) => (segment.start, segment.end, 0.0, 1.0),
            Self::Ray(ray) => (ray.origin, ray.through, 0.0, f64::INFINITY),
        };
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let parameter = ((point.x - start.x) * dx + (point.y - start.y) * dy) / (dx * dx + dy * dy);
        parameter >= minimum - STANDARD_PRECISION && parameter <= maximum + STANDARD_PRECISION
    }

    fn point_at(self, parameter: f64) -> Result<crate::RealAffinePoint, ProjectiveError> {
        match self {
            Self::Line(line) => {
                let Renderability::Renderable(line) = line.renderability() else {
                    return Err(ProjectiveError::NonRealAffineLine);
                };
                Ok(crate::RealAffinePoint {
                    x: -line.a * line.c - line.b * parameter,
                    y: -line.b * line.c + line.a * parameter,
                })
            }
            Self::Segment(segment) => Ok(interpolate(
                segment.start,
                segment.end,
                parameter.clamp(0.0, 1.0),
            )),
            Self::Ray(ray) => Ok(interpolate(ray.origin, ray.through, parameter.max(0.0))),
        }
    }
}

fn interpolate(
    start: crate::RealAffinePoint,
    end: crate::RealAffinePoint,
    parameter: f64,
) -> crate::RealAffinePoint {
    crate::RealAffinePoint {
        x: start.x + (end.x - start.x) * parameter,
        y: start.y + (end.y - start.y) * parameter,
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
