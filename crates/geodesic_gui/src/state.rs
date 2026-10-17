use std::path::PathBuf;

use bevy::prelude::{Handle, Image, Resource, Vec2};
use geodesic_core::{Label, ProjectiveLine, ProjectivePoint};
use geodesic_io::Project;

pub(crate) const SIDE_PANEL_WIDTH: f32 = 220.0;
pub(crate) const TOOLBAR_HEIGHT: f32 = 36.0;
pub(crate) const COMMAND_BAR_HEIGHT: f32 = 44.0;
pub(crate) const COMPLETION_ROW_HEIGHT: f32 = 24.0;
pub(crate) const MESSAGE_BAR_VERTICAL_PADDING: f32 = 8.0;
pub(crate) const MAX_COMPLETIONS: usize = 8;
pub(crate) const LISP_DRAFT_DEBOUNCE_SECONDS: f64 = 0.3;

#[derive(Resource, Default)]
pub(crate) struct GuiState {
    pub(crate) project: Project,
    /// Native location of the persisted project, if it has been opened or saved.
    pub(crate) document_path: Option<PathBuf>,
    /// Editable document text, separate from the validated formal project.
    pub(crate) lisp_draft: LispDraft,
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
    /// Present only while a middle-button pan that began on the canvas is active.
    pub(crate) middle_pan_cursor: Option<Vec2>,
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

/// Text being edited in the project-data panel.
#[derive(Clone, Debug)]
pub(crate) struct LispDraft {
    pub(crate) source: String,
    pub(crate) state: LispDraftState,
}

impl Default for LispDraft {
    fn default() -> Self {
        Self {
            source: geodesic_io::save(&Project::default()),
            state: LispDraftState::Clean,
        }
    }
}

/// Validation status of [`LispDraft`].
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) enum LispDraftState {
    #[default]
    Clean,
    Pending {
        changed_at_seconds: f64,
    },
    Invalid(String),
}

impl GuiState {
    /// Whether tools may mutate the formal project.
    pub(crate) fn project_is_editable(&self) -> bool {
        matches!(self.lisp_draft.state, LispDraftState::Clean)
    }

    /// Starts debounce validation after a user changes the Lisp source.
    pub(crate) fn edit_lisp_draft(&mut self, changed_at_seconds: f64) {
        self.lisp_draft.state = LispDraftState::Pending { changed_at_seconds };
    }

    /// Applies a valid draft after the debounce interval without executing scripts.
    pub(crate) fn reconcile_lisp_draft(&mut self, now_seconds: f64) {
        let LispDraftState::Pending { changed_at_seconds } = self.lisp_draft.state else {
            return;
        };
        if now_seconds - changed_at_seconds < LISP_DRAFT_DEBOUNCE_SECONDS {
            return;
        }
        match geodesic_io::load(&self.lisp_draft.source) {
            Ok(project) => {
                self.project = project;
                self.selected = None;
                self.dragging = None;
                self.preview = None;
                self.tool_inputs.clear();
                self.lisp_draft.state = LispDraftState::Clean;
                "applied Lisp draft".clone_into(&mut self.message);
            }
            Err(error) => self.lisp_draft.state = LispDraftState::Invalid(error.to_string()),
        }
    }

    /// Replaces the formal project and makes its canonical serialization the draft.
    pub(crate) fn replace_project(&mut self, project: Project) {
        self.project = project;
        self.selected = None;
        self.dragging = None;
        self.preview = None;
        self.tool_inputs.clear();
        self.sync_lisp_draft();
    }

    /// Updates the draft after a canvas, style, or command-bar mutation.
    pub(crate) fn sync_lisp_draft(&mut self) {
        self.lisp_draft.source = geodesic_io::save(&self.project);
        self.lisp_draft.state = LispDraftState::Clean;
    }

    /// Abandons unvalidated source and restores the canonical formal document.
    pub(crate) fn discard_lisp_draft(&mut self) {
        self.sync_lisp_draft();
        "discarded Lisp draft changes".clone_into(&mut self.message);
    }
}

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
