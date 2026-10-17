//! Versioned S-expression persistence and read-only `GeoGebra` import reporting.
#![forbid(unsafe_code)]

mod format;
mod ggb;
mod model;
mod style;

#[cfg(test)]
mod tests;

pub use format::{load, save};
pub use ggb::import_ggb;
pub use model::{
    CompatibilityPlaceholder, CompatibilityReport, ObjectStyle, Project, ProjectError, Script,
};
pub use style::{apply_command, effective_style};

/// Current native document schema version.
pub const SCHEMA_VERSION: u32 = 1;
