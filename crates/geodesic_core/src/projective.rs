use num_complex::Complex64;

use crate::ProjectiveError;

/// Initial tolerance aligned with `GeoGebra`'s standard comparison precision.
pub const STANDARD_PRECISION: f64 = 1e-8;

/// `f32` projection of [`STANDARD_PRECISION`] for renderer adapters.
pub const STANDARD_PRECISION_F32: f32 = 1e-8;

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

/// Canvas visibility of a projective point in the initial real affine chart.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Renderability {
    /// The point is real and finite in the current affine chart.
    Renderable(RealAffinePoint),
    /// The point is real but belongs to the line at infinity of the chart.
    AtInfinity,
    /// At least one normalized affine coordinate has a non-negligible imaginary part.
    NonReal,
}

impl ProjectivePoint {
    /// Builds a point from non-zero homogeneous complex coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectiveError::ZeroCoordinates`] when every coordinate is
    /// effectively zero under [`STANDARD_PRECISION`].
    pub fn new(
        x: ComplexCoordinate,
        y: ComplexCoordinate,
        z: ComplexCoordinate,
    ) -> Result<Self, ProjectiveError> {
        let coordinates = [x, y, z];
        if coordinates
            .iter()
            .all(|coordinate| is_near_zero(*coordinate))
        {
            Err(ProjectiveError::ZeroCoordinates)
        } else {
            Ok(Self { coordinates })
        }
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
        let [x, y, z] = self.coordinates;
        if is_near_zero(z) {
            return Renderability::AtInfinity;
        }

        let affine_x = x / z;
        let affine_y = y / z;
        if is_near_zero(ComplexCoordinate::new(affine_x.im, 0.0))
            && is_near_zero(ComplexCoordinate::new(affine_y.im, 0.0))
        {
            Renderability::Renderable(RealAffinePoint {
                x: affine_x.re,
                y: affine_y.re,
            })
        } else {
            Renderability::NonReal
        }
    }
}

impl ProjectiveLine {
    /// Builds a line from non-zero homogeneous coefficients `a*x + b*y + c*z = 0`.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectiveError::ZeroCoordinates`] when every coefficient is
    /// effectively zero under [`STANDARD_PRECISION`].
    pub fn new(
        a: ComplexCoordinate,
        b: ComplexCoordinate,
        c: ComplexCoordinate,
    ) -> Result<Self, ProjectiveError> {
        let coefficients = [a, b, c];
        if coefficients
            .iter()
            .all(|coefficient| is_near_zero(*coefficient))
        {
            Err(ProjectiveError::ZeroCoordinates)
        } else {
            Ok(Self { coefficients })
        }
    }

    /// Returns the homogeneous line coefficients exactly as stored.
    #[must_use]
    pub fn coefficients(&self) -> [ComplexCoordinate; 3] {
        self.coefficients
    }

    /// Returns whether this line is incident with a point under the standard tolerance.
    #[must_use]
    pub fn is_incident_with(&self, point: ProjectivePoint) -> bool {
        let [coefficient_x, coefficient_y, coefficient_z] = self.coefficients;
        let [coordinate_x, coordinate_y, coordinate_z] = point.coordinates;
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
        let coordinates = cross_product(self.coefficients, other.coefficients);
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
    let coefficients = cross_product(first.coordinates, second.coordinates);
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
    let [coordinate_x, coordinate_y, coordinate_z] = point.coordinates;
    let [coefficient_x, coefficient_y, _] = line.coefficients;
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
    let [coordinate_x, coordinate_y, coordinate_z] = point.coordinates;
    let [coefficient_x, coefficient_y, _] = line.coefficients;
    ProjectiveLine::new(
        coefficient_y * coordinate_z,
        -coefficient_x * coordinate_z,
        -coefficient_y * coordinate_x + coefficient_x * coordinate_y,
    )
    .map_err(|_| ProjectiveError::NonUniqueIntersection)
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
