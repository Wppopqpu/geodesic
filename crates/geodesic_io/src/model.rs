use std::collections::BTreeMap;

use geodesic_core::{Construction, Label};

/// A complete persisted project document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The geometric construction.
    pub construction: Construction,
    /// Explicit application-owned script sections. They are data until dispatched.
    pub scripts: Vec<Script>,
    /// Direct visual overrides indexed by geometric-object label.
    pub object_styles: BTreeMap<Label, ObjectStyle>,
    /// Named reusable point/line appearance definitions.
    pub style_presets: BTreeMap<Label, ObjectStyle>,
    /// Optional named style preset used by each geometric object.
    pub object_presets: BTreeMap<Label, Label>,
}

/// Persisted visual properties shared by point and line renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectStyle {
    /// CSS-compatible RGB hex color.
    pub color: String,
    /// Point radius or line width in SVG units.
    pub size: f64,
    /// Opacity in the closed range zero through one.
    pub opacity: f64,
}

impl Default for ObjectStyle {
    fn default() -> Self {
        Self {
            color: "#1565c0".to_owned(),
            size: 0.15,
            opacity: 1.0,
        }
    }
}

/// A script attached to a named application event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    /// Event name, for example `on_open`.
    pub event: String,
    /// Steel source code.
    pub source: String,
}

/// A read-only representation of an unsupported `GeoGebra` object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityPlaceholder {
    /// Source object label.
    pub label: String,
    /// Source object type.
    pub source_type: String,
    /// Human-readable source definition summary.
    pub definition_summary: String,
    /// Known source dependencies.
    pub dependencies: Vec<String>,
}

/// Structured results of a lossy `.ggb` import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompatibilityReport {
    /// Number of natively converted objects.
    pub converted: usize,
    /// Objects retained as read-only placeholders.
    pub placeholders: Vec<CompatibilityPlaceholder>,
    /// Legacy `GGBScript` is always ignored by this release.
    pub ignored_ggb_script: bool,
}

/// Persistence failure.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The S-expression source cannot be parsed or validated.
    #[error("invalid project document: {0}")]
    Invalid(String),
    /// A validated command could not be applied.
    #[error(transparent)]
    Command(#[from] geodesic_core::CommandError),
    /// A coordinate cannot be represented as a finite number.
    #[error("invalid numeric coordinate `{0}`")]
    Number(String),
    /// The `.ggb` archive is malformed or has no construction XML.
    #[error("invalid .ggb archive: {0}")]
    Ggb(String),
}
