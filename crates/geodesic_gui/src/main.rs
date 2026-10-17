//! Native Bevy canvas and `egui` application chrome.
#![forbid(unsafe_code)]

mod app;
mod canvas;
mod chrome;
mod command;
mod presentation;
mod state;

#[cfg(test)]
#[path = "tests.rs"]
mod gui_tests;

#[cfg(test)]
pub(crate) use canvas::{
    canvas_contains, clipped_line, grid_step, line_is_hit, next_zoom_scale, point_is_hit,
    point_radius_world, point_selection_radius_world,
};
#[cfg(test)]
pub(crate) use command::{
    command_completions, command_hint, execute_command, history_next, history_previous,
    replace_completion,
};
#[cfg(test)]
pub(crate) use presentation::{
    clamp_label_position, format_coordinate, hex_color, rgb, submit_command, world_to_screen_y,
};
#[cfg(test)]
pub(crate) use state::{GuiState, MAX_COMPLETIONS, UiTheme};

fn main() {
    app::run();
}
