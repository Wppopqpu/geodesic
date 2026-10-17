use bevy::prelude::{Resource, Vec2};
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum UiTheme {
    #[default]
    System,
    Light,
    Dark,
}
