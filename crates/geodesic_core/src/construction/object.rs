use crate::{Label, ProjectiveLine, ProjectivePoint};

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
