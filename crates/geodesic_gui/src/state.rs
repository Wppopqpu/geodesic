use bevy::prelude::{Handle, Image, Resource, Vec2};
use geodesic_core::{Label, ProjectiveLine, ProjectivePoint};
use geodesic_io::Project;

pub(crate) const SIDE_PANEL_WIDTH: f32 = 220.0;
pub(crate) const TOOLBAR_HEIGHT: f32 = 36.0;
pub(crate) const COMMAND_BAR_HEIGHT: f32 = 44.0;
pub(crate) const COMPLETION_ROW_HEIGHT: f32 = 24.0;
pub(crate) const MESSAGE_BAR_VERTICAL_PADDING: f32 = 8.0;
pub(crate) const MAX_COMPLETIONS: usize = 8;

#[derive(Resource, Default)]
pub(crate) struct GuiState {
    pub(crate) project: Project,
    pub(crate) command: String,
    pub(crate) command_bar_visible: bool,
    pub(crate) command_bar_focus_requested: bool,
    pub(crate) command_history: Vec<String>,
    pub(crate) history_cursor: Option<usize>,
    pub(crate) completion_query: String,
    pub(crate) completion_index: usize,
    pub(crate) message: String,
    pub(crate) theme: UiTheme,
    pub(crate) algebra_width: f32,
    pub(crate) style_width: f32,
    pub(crate) selected: Option<Label>,
    pub(crate) dragging: Option<DragTarget>,
    pub(crate) preview: Option<DragPreview>,
    pub(crate) pan_cursor: Option<Vec2>,
    pub(crate) bottom_ui_height: f32,
    /// Horizontal position of the toolbar's overflow viewport in logical pixels.
    pub(crate) toolbar_scroll_offset: f32,
    /// Whether the toolbar's content exceeded its viewport in the previous frame.
    pub(crate) toolbar_overflows: bool,
    /// The currently open product attribution dialog, if any.
    pub(crate) about_dialog: Option<AboutDialog>,
    pub(crate) active_tool: ActiveTool,
    pub(crate) tool_inputs: Vec<Label>,
}

/// A visible product-attribution surface.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AboutDialog;

/// Loaded toolbar icons with their provenance recorded in `assets/ASSET_MANIFEST.md`.
#[derive(Resource)]
pub(crate) struct ToolbarIcons {
    pub(crate) move_tool: Handle<Image>,
    pub(crate) point: Handle<Image>,
    pub(crate) segment: Handle<Image>,
    pub(crate) ray: Handle<Image>,
    pub(crate) vector: Handle<Image>,
    pub(crate) midpoint: Handle<Image>,
    pub(crate) perpendicular_bisector: Handle<Image>,
    pub(crate) angle_bisector: Handle<Image>,
}

#[derive(Clone)]
pub(crate) enum DragTarget {
    Point(Label),
    Line { label: Label, line: ProjectiveLine },
}

#[derive(Clone)]
pub(crate) enum DragPreview {
    Point {
        label: Label,
        point: ProjectivePoint,
    },
    Line {
        label: Label,
        line: ProjectiveLine,
    },
}

/// A compact construction-tool state. Each completed tool submits one `GeoCommand`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ActiveTool {
    /// Select and move free objects.
    #[default]
    Move,
    /// Create a free point at the cursor.
    Point,
    /// Select two points to create a segment.
    Segment,
    /// Select an origin and a direction point.
    Ray,
    /// Select a tail and a head point.
    Vector,
    /// Select two points to create their midpoint.
    Midpoint,
    /// Select two points to create their perpendicular bisector.
    PerpendicularBisector,
    /// Select three points in first-vertex-second order.
    AngleBisector,
}

impl ActiveTool {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 8] = [
        Self::Move,
        Self::Point,
        Self::Segment,
        Self::Ray,
        Self::Vector,
        Self::Midpoint,
        Self::PerpendicularBisector,
        Self::AngleBisector,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Point => "Point",
            Self::Segment => "Segment",
            Self::Ray => "Ray",
            Self::Vector => "Vector",
            Self::Midpoint => "Midpoint",
            Self::PerpendicularBisector => "Perp bisector",
            Self::AngleBisector => "Angle bisector",
        }
    }

    pub(crate) const fn input_count(self) -> usize {
        match self {
            Self::Move | Self::Point => 0,
            Self::Segment
            | Self::Ray
            | Self::Vector
            | Self::Midpoint
            | Self::PerpendicularBisector => 2,
            Self::AngleBisector => 3,
        }
    }

    pub(crate) const fn label_prefix(self) -> &'static str {
        match self {
            Self::Move | Self::Point => "point",
            Self::Segment => "segment",
            Self::Ray => "ray",
            Self::Vector => "vector",
            Self::Midpoint => "midpoint",
            Self::PerpendicularBisector => "perpendicular_bisector",
            Self::AngleBisector => "angle_bisector",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum UiTheme {
    #[default]
    System,
    Light,
    Dark,
}
