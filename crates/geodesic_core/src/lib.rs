//! Domain-only geometric construction kernel.
//!
//! This crate deliberately has no GUI, persistence, scripting, or platform
//! dependencies. Adapters submit [`GeoCommand`] values and consume immutable
//! [`GeometrySnapshot`] values.

#![forbid(unsafe_code)]

mod construction;
mod error;
mod label;
mod projective;

pub use construction::{
    Construction, GeoCommand, GeometricObject, GeometrySnapshot, ObjectDefinition,
};
pub use error::{CommandError, EvaluationError, HistoryError, LabelError, ProjectiveError};
pub use label::Label;
pub use projective::{
    ComplexCoordinate, ProjectiveLine, ProjectivePoint, RealAffinePoint, Renderability,
    STANDARD_PRECISION, STANDARD_PRECISION_F32, parallel_through, perpendicular_through,
};

/// Version of the domain kernel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
