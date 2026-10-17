use crate::{Label, ProjectiveLine, ProjectivePoint, RealAffinePoint};

/// A closed bounded straight locus between two real finite points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineSegment {
    /// The first endpoint.
    pub start: RealAffinePoint,
    /// The second endpoint.
    pub end: RealAffinePoint,
}

/// A one-sided straight locus from `origin` through `through`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineRay {
    /// The finite start point.
    pub origin: RealAffinePoint,
    /// A distinct point defining direction.
    pub through: RealAffinePoint,
}

/// A directed real-affine displacement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AffineVector {
    /// The tail of the vector.
    pub start: RealAffinePoint,
    /// The head of the vector.
    pub end: RealAffinePoint,
}

/// Which output of a two-line angle-bisector construction this label names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineAngleBisectorBranch {
    /// The normalized signed-distance sum.
    Sum,
    /// The normalized signed-distance difference.
    Difference,
}

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
    /// A bounded segment between two real finite points.
    Segment { start: Label, end: Label },
    /// A ray through two real finite points.
    Ray { origin: Label, through: Label },
    /// A vector between two real finite points.
    Vector { start: Label, end: Label },
    /// The midpoint of two real finite points.
    Midpoint { first: Label, second: Label },
    /// The perpendicular bisector of two real finite points.
    PerpendicularBisector { first: Label, second: Label },
    /// The internal angle bisector through a real finite vertex.
    AngleBisector {
        first: Label,
        vertex: Label,
        second: Label,
    },
    /// One output of a two-line angle-bisector construction shared by `group`.
    LineAngleBisector {
        group: Label,
        peer: Label,
        first: Label,
        second: Label,
        branch: LineAngleBisectorBranch,
    },
    /// A semi-free real-affine point parameterized by a supported locus.
    PointOnLocus { locus: Label, parameter: f64 },
}

/// An evaluated geometric value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeometricObject {
    /// A point in the complex projective plane.
    Point(ProjectivePoint),
    /// A line in the complex projective plane.
    Line(ProjectiveLine),
    /// A bounded real-affine segment.
    Segment(AffineSegment),
    /// A one-sided real-affine ray.
    Ray(AffineRay),
    /// A directed real-affine vector.
    Vector(AffineVector),
}
