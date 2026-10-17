use num_complex::Complex64;

use crate::ProjectiveError;

/// Initial tolerance aligned with `GeoGebra`'s standard comparison precision.
pub const STANDARD_PRECISION: f64 = 1e-8;

/// `f32` projection of [`STANDARD_PRECISION`] for renderer adapters.
pub const STANDARD_PRECISION_F32: f32 = 1e-8;

/// Upper boundary of the conservative reality-classification ambiguity band.
const REALITY_AMBIGUITY_LIMIT: f64 = 1e-5;

/// A complex coordinate used by homogeneous projective values.
pub type ComplexCoordinate = Complex64;

/// A valid point in the complex projective plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectivePoint {
    coordinates: [ComplexCoordinate; 3],
}

/// A valid line in the complex projective plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectiveLine {
    coefficients: [ComplexCoordinate; 3],
}

/// A point that can be drawn in the initial real affine canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RealAffinePoint {
    /// Horizontal affine coordinate.
    pub x: f64,
    /// Vertical affine coordinate.
    pub y: f64,
}

impl RealAffinePoint {
    /// Returns the Euclidean distance to another affine point.
    #[must_use]
    pub fn distance_to(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }

    /// Converts this value to a finite projective point.
    #[must_use]
    pub fn projective(self) -> ProjectivePoint {
        ProjectivePoint::real_affine(self.x, self.y)
    }
}

/// Real line coefficients with a unit-length affine normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RealAffineLine {
    /// Horizontal normal component.
    pub a: f64,
    /// Vertical normal component.
    pub b: f64,
    /// Constant term in `a*x + b*y + c = 0`.
    pub c: f64,
}

/// Display classification under the kernel's normalized tolerance policy.
///
/// This is a floating-point policy, not a certified interval error bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Renderability<T = RealAffinePoint> {
    /// The object has a supported real affine representation.
    Renderable(T),
    /// The object is real and lies entirely at infinity in the chart.
    AtInfinity,
    /// At least one phase-normalized homogeneous component is non-real.
    NonReal,
    /// Reality is ambiguous, input is non-finite, or affine extraction is ill-conditioned.
    NumericallyUncertain,
}

impl ProjectivePoint {
    /// Builds a point from non-zero homogeneous complex coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectiveError::ZeroCoordinates`] for an all-zero tuple, or
    /// [`ProjectiveError::NonFiniteCoordinates`] for non-finite components.
    pub fn new(
        x: ComplexCoordinate,
        y: ComplexCoordinate,
        z: ComplexCoordinate,
    ) -> Result<Self, ProjectiveError> {
        let coordinates = [x, y, z];
        normalized(coordinates)?;
        Ok(Self { coordinates })
    }

    /// Builds a finite real-affine point.
    #[must_use]
    pub fn real_affine(x: f64, y: f64) -> Self {
        Self {
            coordinates: [
                ComplexCoordinate::new(x, 0.0),
                ComplexCoordinate::new(y, 0.0),
                ComplexCoordinate::new(1.0, 0.0),
            ],
        }
    }

    /// Returns the homogeneous coordinates exactly as stored.
    #[must_use]
    pub fn coordinates(&self) -> [ComplexCoordinate; 3] {
        self.coordinates
    }

    /// Classifies whether the point can be drawn in the initial canvas chart.
    #[must_use]
    pub fn renderability(&self) -> Renderability {
        let Ok(normalized_coordinates) = normalized(self.coordinates) else {
            return Renderability::NumericallyUncertain;
        };
        match real_components(self.coordinates) {
            Renderability::Renderable([x, y, z]) => {
                if normalized_coordinates[2].norm() == 0.0 {
                    Renderability::AtInfinity
                } else if z.abs() <= STANDARD_PRECISION {
                    Renderability::NumericallyUncertain
                } else {
                    // Division can magnify an otherwise negligible imaginary residual.
                    let [cx, cy, cz] = normalized_coordinates;
                    if (cx / cz).im.abs().max((cy / cz).im.abs()) > STANDARD_PRECISION {
                        return Renderability::NumericallyUncertain;
                    }
                    Renderability::Renderable(RealAffinePoint { x: x / z, y: y / z })
                }
            }
            Renderability::NonReal => Renderability::NonReal,
            Renderability::AtInfinity => Renderability::AtInfinity,
            Renderability::NumericallyUncertain => Renderability::NumericallyUncertain,
        }
    }
}

/// Extracts a real finite affine point for a bounded construction.
///
/// The original complex projective value remains unchanged.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonRealAffinePoint`] if the point is non-real,
/// at infinity, or numerically uncertain in the initial real affine chart.
pub fn real_affine_point(point: ProjectivePoint) -> Result<RealAffinePoint, ProjectiveError> {
    match point.renderability() {
        Renderability::Renderable(point) => Ok(point),
        Renderability::AtInfinity
        | Renderability::NonReal
        | Renderability::NumericallyUncertain => Err(ProjectiveError::NonRealAffinePoint),
    }
}

impl ProjectiveLine {
    /// Builds a line from non-zero homogeneous coefficients `a*x + b*y + c*z = 0`.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectiveError::ZeroCoordinates`] for an all-zero tuple, or
    /// [`ProjectiveError::NonFiniteCoordinates`] for non-finite components.
    pub fn new(
        a: ComplexCoordinate,
        b: ComplexCoordinate,
        c: ComplexCoordinate,
    ) -> Result<Self, ProjectiveError> {
        let coefficients = [a, b, c];
        normalized(coefficients)?;
        Ok(Self { coefficients })
    }

    /// Returns the homogeneous line coefficients exactly as stored.
    #[must_use]
    pub fn coefficients(&self) -> [ComplexCoordinate; 3] {
        self.coefficients
    }

    /// Extracts a real affine line after removing homogeneous scale and phase.
    ///
    /// Non-real lines may contain isolated real points; this method classifies
    /// whether the entire line has a supported real affine representation.
    #[must_use]
    pub fn renderability(&self) -> Renderability<RealAffineLine> {
        let Ok(values) = normalized(self.coefficients) else {
            return Renderability::NumericallyUncertain;
        };
        match real_components(self.coefficients) {
            Renderability::Renderable([a, b, c]) => {
                let length = a.hypot(b);
                if values[0].norm() == 0.0 && values[1].norm() == 0.0 {
                    Renderability::AtInfinity
                } else if length <= STANDARD_PRECISION {
                    Renderability::NumericallyUncertain
                } else {
                    Renderability::Renderable(RealAffineLine {
                        a: a / length,
                        b: b / length,
                        c: c / length,
                    })
                }
            }
            Renderability::NonReal => Renderability::NonReal,
            Renderability::AtInfinity => Renderability::AtInfinity,
            Renderability::NumericallyUncertain => Renderability::NumericallyUncertain,
        }
    }

    /// Returns whether this line is incident with a point under the standard tolerance.
    #[must_use]
    pub fn is_incident_with(&self, point: ProjectivePoint) -> bool {
        let Ok([coefficient_x, coefficient_y, coefficient_z]) = normalized(self.coefficients)
        else {
            return false;
        };
        let Ok([coordinate_x, coordinate_y, coordinate_z]) = normalized(point.coordinates) else {
            return false;
        };
        is_near_zero(
            coefficient_x * coordinate_x
                + coefficient_y * coordinate_y
                + coefficient_z * coordinate_z,
        )
    }

    /// Returns the projective intersection with another line.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectiveError::NonUniqueIntersection`] when the lines are
    /// coincident under [`STANDARD_PRECISION`].
    pub fn intersection(self, other: Self) -> Result<ProjectivePoint, ProjectiveError> {
        let coordinates = projective_cross_product(self.coefficients, other.coefficients)?;
        ProjectivePoint::new(coordinates[0], coordinates[1], coordinates[2])
            .map_err(|_| ProjectiveError::NonUniqueIntersection)
    }
}

/// Returns the line passing through two distinct projective points.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonUniqueIntersection`] when the points are
/// projectively equal under [`STANDARD_PRECISION`].
pub fn line_through(
    first: ProjectivePoint,
    second: ProjectivePoint,
) -> Result<ProjectiveLine, ProjectiveError> {
    let coefficients = projective_cross_product(first.coordinates, second.coordinates)?;
    ProjectiveLine::new(coefficients[0], coefficients[1], coefficients[2])
        .map_err(|_| ProjectiveError::NonUniqueIntersection)
}

/// Returns the line parallel to `line` and incident with `point`.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonUniqueIntersection`] when `line` is the line
/// at infinity and no finite affine parallel is defined.
pub fn parallel_through(
    point: ProjectivePoint,
    line: ProjectiveLine,
) -> Result<ProjectiveLine, ProjectiveError> {
    let [coordinate_x, coordinate_y, coordinate_z] = normalized(point.coordinates)?;
    let [coefficient_x, coefficient_y, _] = normalized(line.coefficients)?;
    ProjectiveLine::new(
        coefficient_x * coordinate_z,
        coefficient_y * coordinate_z,
        -(coefficient_x * coordinate_x + coefficient_y * coordinate_y),
    )
    .map_err(|_| ProjectiveError::NonUniqueIntersection)
}

/// Returns the Euclidean-affine perpendicular to `line` through `point`.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonUniqueIntersection`] when `line` is the line
/// at infinity and no finite affine perpendicular is defined.
pub fn perpendicular_through(
    point: ProjectivePoint,
    line: ProjectiveLine,
) -> Result<ProjectiveLine, ProjectiveError> {
    let [coordinate_x, coordinate_y, coordinate_z] = normalized(point.coordinates)?;
    let [coefficient_x, coefficient_y, _] = normalized(line.coefficients)?;
    ProjectiveLine::new(
        coefficient_y * coordinate_z,
        -coefficient_x * coordinate_z,
        -coefficient_y * coordinate_x + coefficient_x * coordinate_y,
    )
    .map_err(|_| ProjectiveError::NonUniqueIntersection)
}

/// Returns the midpoint between two distinct real finite points.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonRealAffinePoint`] or
/// [`ProjectiveError::CoincidentAffinePoints`] when no bounded midpoint exists.
pub fn midpoint(
    first: ProjectivePoint,
    second: ProjectivePoint,
) -> Result<ProjectivePoint, ProjectiveError> {
    let first = real_affine_point(first)?;
    let second = real_affine_point(second)?;
    require_distinct_affine_points(first, second)?;
    Ok(ProjectivePoint::real_affine(
        first.x.midpoint(second.x),
        first.y.midpoint(second.y),
    ))
}

/// Returns the real-affine perpendicular bisector of two distinct points.
///
/// # Errors
///
/// Returns [`ProjectiveError::NonRealAffinePoint`] or
/// [`ProjectiveError::CoincidentAffinePoints`] when the construction is undefined.
pub fn perpendicular_bisector(
    first: ProjectivePoint,
    second: ProjectivePoint,
) -> Result<ProjectiveLine, ProjectiveError> {
    let first = real_affine_point(first)?;
    let second = real_affine_point(second)?;
    require_distinct_affine_points(first, second)?;
    let midpoint = RealAffinePoint {
        x: first.x.midpoint(second.x),
        y: first.y.midpoint(second.y),
    };
    let a = second.x - first.x;
    let b = second.y - first.y;
    ProjectiveLine::new(
        a.into(),
        b.into(),
        (-(a * midpoint.x + b * midpoint.y)).into(),
    )
}

/// Returns the internal angle bisector of `first`-`vertex`-`second`.
///
/// # Errors
///
/// Returns [`ProjectiveError::DegenerateAffineAngle`] for a zero or straight
/// angle, or [`ProjectiveError::NonRealAffinePoint`] outside the real chart.
pub fn angle_bisector(
    first: ProjectivePoint,
    vertex: ProjectivePoint,
    second: ProjectivePoint,
) -> Result<ProjectiveLine, ProjectiveError> {
    let first = real_affine_point(first)?;
    let vertex = real_affine_point(vertex)?;
    let second = real_affine_point(second)?;
    let first_length = first.distance_to(vertex);
    let second_length = second.distance_to(vertex);
    if first_length <= STANDARD_PRECISION || second_length <= STANDARD_PRECISION {
        return Err(ProjectiveError::DegenerateAffineAngle);
    }
    let direction_x = (first.x - vertex.x) / first_length + (second.x - vertex.x) / second_length;
    let direction_y = (first.y - vertex.y) / first_length + (second.y - vertex.y) / second_length;
    if direction_x.hypot(direction_y) <= STANDARD_PRECISION {
        return Err(ProjectiveError::DegenerateAffineAngle);
    }
    let normal_x = -direction_y;
    let normal_y = direction_x;
    ProjectiveLine::new(
        normal_x.into(),
        normal_y.into(),
        (-(normal_x * vertex.x + normal_y * vertex.y)).into(),
    )
}

fn require_distinct_affine_points(
    first: RealAffinePoint,
    second: RealAffinePoint,
) -> Result<(), ProjectiveError> {
    (first.distance_to(second) > STANDARD_PRECISION)
        .then_some(())
        .ok_or(ProjectiveError::CoincidentAffinePoints)
}

fn cross_product(
    first: [ComplexCoordinate; 3],
    second: [ComplexCoordinate; 3],
) -> [ComplexCoordinate; 3] {
    [
        first[1] * second[2] - first[2] * second[1],
        first[2] * second[0] - first[0] * second[2],
        first[0] * second[1] - first[1] * second[0],
    ]
}

// Divide by a real component scale before complex division to avoid overflow
// and underflow in the complex denominator. The largest pivot fixes phase.
fn normalized(values: [ComplexCoordinate; 3]) -> Result<[ComplexCoordinate; 3], ProjectiveError> {
    if values
        .iter()
        .any(|value| !value.re.is_finite() || !value.im.is_finite())
    {
        return Err(ProjectiveError::NonFiniteCoordinates);
    }
    let scale = values.iter().fold(0.0_f64, |scale, value| {
        scale.max(value.re.abs()).max(value.im.abs())
    });
    if scale == 0.0 {
        return Err(ProjectiveError::ZeroCoordinates);
    }
    let scaled = values.map(|value| value / scale);
    let pivot = scaled
        .iter()
        .copied()
        .reduce(|a, b| if a.norm_sqr() >= b.norm_sqr() { a } else { b })
        .ok_or(ProjectiveError::ZeroCoordinates)?;
    Ok(scaled.map(|value| value / pivot))
}

fn real_components(values: [ComplexCoordinate; 3]) -> Renderability<[f64; 3]> {
    let Ok(values) = normalized(values) else {
        return Renderability::NumericallyUncertain;
    };
    let residual = values
        .iter()
        .map(|value| value.im.abs())
        .fold(0.0_f64, f64::max);
    if residual <= STANDARD_PRECISION {
        Renderability::Renderable(values.map(|value| value.re))
    } else if residual < REALITY_AMBIGUITY_LIMIT {
        Renderability::NumericallyUncertain
    } else {
        Renderability::NonReal
    }
}

fn projective_cross_product(
    first: [ComplexCoordinate; 3],
    second: [ComplexCoordinate; 3],
) -> Result<[ComplexCoordinate; 3], ProjectiveError> {
    let result = cross_product(normalized(first)?, normalized(second)?);
    if result.iter().all(|value| is_near_zero(*value)) {
        Err(ProjectiveError::NonUniqueIntersection)
    } else {
        Ok(result)
    }
}

fn is_near_zero(value: ComplexCoordinate) -> bool {
    value.norm_sqr() < STANDARD_PRECISION.powi(2)
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectiveLine, ProjectivePoint, RealAffinePoint, Renderability, line_through,
        parallel_through, perpendicular_through,
    };

    #[test]
    fn real_extraction_is_invariant_under_complex_homogeneous_scaling() {
        use super::ComplexCoordinate as C;
        for factor in [
            C::new(0.0, 1.0),
            C::new(-3.0, 4.0),
            C::new(1e-200, -1e-200),
            C::new(1e200, 1e200),
        ] {
            let point = ProjectivePoint::new(factor * 2.0, factor * 3.0, factor).expect("point");
            let Renderability::Renderable(point) = point.renderability() else {
                panic!("real point");
            };
            assert!((point.x - 2.0).abs() < 1e-12 && (point.y - 3.0).abs() < 1e-12);
            let line = ProjectiveLine::new(factor, factor * 2.0, factor * -8.0).expect("line");
            let Renderability::Renderable(real) = line.renderability() else {
                panic!("real line");
            };
            assert!((real.a * 2.0 + real.b * 3.0 + real.c).abs() < 1e-12);
            assert!(line.is_incident_with(ProjectivePoint::real_affine(2.0, 3.0)));
        }
    }

    #[test]
    fn scaled_constructions_preserve_incidence_and_degeneracy() {
        use super::ComplexCoordinate as C;
        for factor in [C::new(1e-200, 1e-200), C::new(0.0, 1e200)] {
            let line =
                |a, b, c| ProjectiveLine::new(factor * a, factor * b, factor * c).expect("line");
            let vertical = line(1.0, 0.0, -2.0);
            let horizontal = line(0.0, 1.0, -3.0);
            let intersection = vertical.intersection(horizontal).expect("intersection");
            assert!(vertical.is_incident_with(intersection));
            assert!(horizontal.is_incident_with(intersection));
            assert_eq!(
                vertical
                    .intersection(line(1.0, 0.0, -4.0))
                    .expect("parallel intersection")
                    .renderability(),
                Renderability::AtInfinity
            );
            assert!(vertical.intersection(line(2.0, 0.0, -4.0)).is_err());
            assert!(vertical.intersection(line(1.0, 0.0, -2.0 - 1e-10)).is_err());
            let parallel = parallel_through(ProjectivePoint::real_affine(5.0, 6.0), vertical)
                .expect("parallel");
            assert!(parallel.is_incident_with(ProjectivePoint::real_affine(5.0, -6.0)));
        }
    }

    #[test]
    fn real_extraction_distinguishes_noise_ambiguity_and_non_real_values() {
        use super::ComplexCoordinate as C;
        for (imaginary, expected) in [(1e-10, 0), (1e-7, 1), (0.1, 2)] {
            let line =
                ProjectiveLine::new(C::new(1.0, 0.0), C::new(0.5, imaginary), C::new(0.0, 0.0))
                    .expect("line");
            let category = match line.renderability() {
                Renderability::Renderable(_) => 0,
                Renderability::NumericallyUncertain => 1,
                Renderability::NonReal => 2,
                Renderability::AtInfinity => 3,
            };
            assert_eq!(category, expected);
            assert_eq!(
                line.coefficients()[1].im.to_bits(),
                imaginary.to_bits(),
                "classification must not modify the complex object"
            );
        }
    }

    #[test]
    fn infinity_and_ill_conditioned_affine_division_are_distinct() {
        let point = |z| ProjectivePoint::new(1.0.into(), 0.0.into(), z).expect("point");
        assert_eq!(point(0.0.into()).renderability(), Renderability::AtInfinity);
        assert_eq!(
            point(1e-12.into()).renderability(),
            Renderability::NumericallyUncertain
        );
        assert_eq!(
            point(super::ComplexCoordinate::new(0.0, 1e-12)).renderability(),
            Renderability::NumericallyUncertain
        );
        let non_real = ProjectivePoint::new(
            1.0.into(),
            super::ComplexCoordinate::new(0.0, 1.0),
            0.0.into(),
        )
        .expect("point");
        assert_eq!(non_real.renderability(), Renderability::NonReal);
        assert!(ProjectiveLine::new(f64::NAN.into(), 1.0.into(), 0.0.into()).is_err());
    }

    #[test]
    fn derives_an_incident_line_from_two_points() {
        let point_a = ProjectivePoint::real_affine(0.0, 0.0);
        let point_b = ProjectivePoint::real_affine(2.0, 2.0);

        let line = line_through(point_a, point_b).expect("distinct points define a line");

        assert!(line.is_incident_with(point_a));
        assert!(line.is_incident_with(point_b));
    }

    #[test]
    fn finds_a_finite_intersection() {
        let vertical = ProjectiveLine::new(1.0.into(), 0.0.into(), (-2.0).into())
            .expect("valid vertical line");
        let horizontal = ProjectiveLine::new(0.0.into(), 1.0.into(), (-3.0).into())
            .expect("valid horizontal line");

        assert_eq!(
            vertical
                .intersection(horizontal)
                .expect("unique intersection")
                .renderability(),
            Renderability::Renderable(RealAffinePoint { x: 2.0, y: 3.0 })
        );
    }

    #[test]
    fn marks_non_real_points_as_not_renderable() {
        let complex_point = ProjectivePoint::new(
            num_complex::Complex64::new(1.0, 1.0),
            0.0.into(),
            1.0.into(),
        )
        .expect("non-zero coordinates");

        assert_eq!(complex_point.renderability(), Renderability::NonReal);
    }

    #[test]
    fn constructs_parallel_and_perpendicular_lines() {
        let horizontal =
            ProjectiveLine::new(0.0.into(), 1.0.into(), 0.0.into()).expect("valid horizontal line");
        let point = ProjectivePoint::real_affine(2.0, 3.0);

        let parallel = parallel_through(point, horizontal).expect("finite parallel");
        let perpendicular = perpendicular_through(point, horizontal).expect("finite perpendicular");

        assert!(parallel.is_incident_with(point));
        assert!(perpendicular.is_incident_with(point));
        assert_eq!(parallel.coefficients()[0], 0.0.into());
        assert_eq!(perpendicular.coefficients()[1], 0.0.into());
    }
}
