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
    /// Adds a segment bounded by two real finite points.
    AddSegment {
        label: Label,
        start: Label,
        end: Label,
    },
    /// Adds a ray starting at one real finite point through another.
    AddRay {
        label: Label,
        origin: Label,
        through: Label,
    },
    /// Adds a vector between two real finite points.
    AddVector {
        label: Label,
        start: Label,
        end: Label,
    },
    /// Adds the midpoint of two real finite points.
    AddMidpoint {
        label: Label,
        first: Label,
        second: Label,
    },
    /// Adds the perpendicular bisector of two real finite points.
    AddPerpendicularBisector {
        label: Label,
        first: Label,
        second: Label,
    },
    /// Adds the internal angle bisector of three real finite points.
    AddAngleBisector {
        label: Label,
        first: Label,
        vertex: Label,
        second: Label,
    },
    /// Adds both angle bisectors of two supporting lines in one undo unit.
    AddLineAngleBisectors {
        first_label: Label,
        second_label: Label,
        first: Label,
        second: Label,
    },
    /// Removes a definition and its evaluated value.
    Remove { label: Label },
}
