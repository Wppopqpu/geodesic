use std::collections::BTreeMap;

use geodesic_core::{Construction, Label, ObjectDefinition};

/// A complete persisted project document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The geometric construction.
    pub construction: Construction,
    /// Explicit application-owned script sections. They are data until dispatched.
    pub scripts: Vec<Script>,
    /// Complete concrete styles or named fallback references indexed by object label.
    pub object_styles: BTreeMap<Label, StyleBinding>,
    /// Named complete styles or fallback references.
    pub style_presets: BTreeMap<Label, StyleBinding>,
    /// The named default style for each visual category.
    pub category_presets: BTreeMap<StyleCategory, Label>,
}

/// Visual categories with independently configurable named defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StyleCategory {
    /// Points, intersections, and constrained points.
    Point,
    /// Lines and bounded straight loci.
    Line,
}

impl StyleCategory {
    /// Classifies an object definition for default-style lookup.
    #[must_use]
    pub const fn for_definition(definition: &ObjectDefinition) -> Self {
        match definition {
            ObjectDefinition::FreePoint(_)
            | ObjectDefinition::Intersection { .. }
            | ObjectDefinition::Midpoint { .. }
            | ObjectDefinition::PointOnLocus { .. } => Self::Point,
            ObjectDefinition::FreeLine(_)
            | ObjectDefinition::LineThrough { .. }
            | ObjectDefinition::ParallelThrough { .. }
            | ObjectDefinition::PerpendicularThrough { .. }
            | ObjectDefinition::Segment { .. }
            | ObjectDefinition::Ray { .. }
            | ObjectDefinition::Vector { .. }
            | ObjectDefinition::PerpendicularBisector { .. }
            | ObjectDefinition::AngleBisector { .. }
            | ObjectDefinition::LineAngleBisector { .. } => Self::Line,
        }
    }
}

/// A style is exactly a concrete complete value or a named fallback reference.
#[derive(Clone, Debug, PartialEq)]
pub enum StyleBinding {
    /// Every visual property is explicitly stored on this style.
    Concrete(ObjectStyle),
    /// Resolve all properties through a named style.
    Fallback(Label),
}

/// Persisted visual properties shared by point and line renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectStyle {
    /// CSS-compatible RGB hex color.
    pub color: String,
    /// Point radius or line width in persisted render units.
    ///
    /// The canvas maps `0.03` units to one logical pixel of point radius or
    /// half a logical pixel of line width, independent of camera zoom;
    /// SVG output uses the value directly in its view-box coordinate system.
    pub size: f64,
    /// Opacity in the closed range zero through one.
    pub opacity: f64,
    /// Point marker shape.
    pub point_shape: PointShape,
    /// Stroke pattern for straight loci.
    pub line_pattern: LinePattern,
}

/// Supported point marker shapes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointShape {
    #[default]
    Circle,
    Square,
    Diamond,
    Triangle,
}

/// Supported straight-locus stroke patterns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinePattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl Default for ObjectStyle {
    fn default() -> Self {
        Self {
            color: "#1565c0".to_owned(),
            size: 0.15,
            opacity: 1.0,
            point_shape: PointShape::Circle,
            line_pattern: LinePattern::Solid,
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
