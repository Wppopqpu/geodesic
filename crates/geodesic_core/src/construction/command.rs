use crate::{Label, ProjectiveLine, ProjectivePoint};

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
    /// Moves an existing free line without changing its direction.
    MoveLine { label: Label, line: ProjectiveLine },
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
