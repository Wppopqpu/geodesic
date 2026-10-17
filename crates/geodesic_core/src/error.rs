use crate::Label;

/// Failure while parsing a public object label.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LabelError {
    /// Labels cannot be empty.
    #[error("label must not be empty")]
    Empty,
    /// Labels use ASCII `snake_case` so they are stable in scripts and CLI input.
    #[error("label `{value}` must use lowercase ASCII snake_case")]
    Invalid { value: String },
}

/// Failure while creating or evaluating a projective value.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum ProjectiveError {
    /// Homogeneous components must be finite.
    #[error("homogeneous coordinates must be finite")]
    NonFiniteCoordinates,
    /// Homogeneous coordinates must contain a non-zero component.
    #[error("homogeneous coordinates must not all be zero")]
    ZeroCoordinates,
    /// Two lines do not have a unique intersection point.
    #[error("lines do not have a unique projective intersection")]
    NonUniqueIntersection,
}

/// Failure while applying a command to a construction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// A label is already bound in the construction.
    #[error("object label `{label}` already exists")]
    DuplicateLabel { label: Label },
    /// A command references no existing object.
    #[error("object label `{label}` does not exist")]
    UnknownLabel { label: Label },
    /// A command needs an object of a different kind.
    #[error("object `{label}` is not a {expected}")]
    WrongObjectKind {
        label: Label,
        expected: &'static str,
    },
}

/// Failure while evaluating a construction definition.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EvaluationError {
    /// An object definition references no existing object.
    #[error("object `{label}` depends on missing object `{dependency}")]
    MissingDependency { label: Label, dependency: Label },
    /// An object definition expects a point or line but received another kind.
    #[error("object `{label}` depends on `{dependency}`, which is not a {expected}")]
    WrongDependencyKind {
        label: Label,
        dependency: Label,
        expected: &'static str,
    },
    /// The projective operation itself is undefined.
    #[error("cannot evaluate `{label}`: {source}")]
    Projective {
        label: Label,
        #[source]
        source: ProjectiveError,
    },
}

/// Failure while navigating construction command history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HistoryError {
    /// No earlier construction state exists.
    #[error("there is no construction change to undo")]
    NothingToUndo,
    /// No later construction state exists.
    #[error("there is no construction change to redo")]
    NothingToRedo,
}
