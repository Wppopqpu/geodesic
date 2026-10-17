use bevy::prelude::{Camera2d, Color, Projection, Query, Transform, Window, With};
use bevy::window::PrimaryWindow;
use bevy_egui::egui;

use crate::{
    canvas::{grid_step, nice_step, visible_world_bounds},
    command::execute_command,
    state::{GuiState, UiTheme},
};

pub(crate) fn wrapped_text_height(
    context: &egui::Context,
    text: &str,
    available_width: f32,
    style: &egui::TextStyle,
) -> f32 {
    let font = style.resolve(context.style().as_ref());
    let galley = context.fonts(|fonts| {
        fonts.layout(
            text.to_owned(),
            font,
            egui::Color32::PLACEHOLDER,
            available_width.max(1.0),
        )
    });
    galley.size().y
}

pub(crate) fn submit_command(state: &mut GuiState) {
    let command = std::mem::take(&mut state.command);
    if !command.trim().is_empty() {
        state.command_history.push(command.clone());
    }
    state.history_cursor = None;
    state.completion_query.clear();
    state.message = execute_command(&mut state.project.construction, &command)
        .unwrap_or_else(|error| format!("error: {error}"));
    state.command_bar_visible = false;
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Axis labels use the f32 coordinates of the Bevy canvas."
)]
pub(crate) fn draw_axis_labels(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Transform, &Projection), With<Camera2d>>,
) {
    let Some((left, right, bottom, top, scale)) = visible_world_bounds(windows, camera) else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((transform, Projection::Orthographic(orthographic))) = camera.single() else {
        return;
    };
    let grid_step = grid_step(scale);
    let label_step = nice_step((70.0 * scale).max(grid_step));
    let first_x = (left / label_step).ceil() * label_step;
    let first_y = (bottom / label_step).ceil() * label_step;
    let axis_color = egui::Color32::from_rgb(85, 85, 95);

    let x_axis_y = world_to_screen_y(
        0.0,
        transform.translation.y,
        window.height(),
        orthographic.scale,
    );
    let y_axis_x = window.width() / 2.0 - transform.translation.x / orthographic.scale;
    let label_margin = 16.0;
    let x_label_y = if x_axis_y < canvas_rect.top() {
        canvas_rect.top() + 2.0
    } else if x_axis_y > canvas_rect.bottom() {
        canvas_rect.bottom() - 16.0
    } else {
        x_axis_y + 3.0
    };
    for index in 0..=((right - first_x) / label_step).floor().max(0.0) as usize {
        let value = first_x + index as f32 * label_step;
        if value.abs() < grid_step * 0.5 {
            continue;
        }
        let screen_x =
            window.width() / 2.0 + (value - transform.translation.x) / orthographic.scale;
        if !(canvas_rect.left() + label_margin..=canvas_rect.right() - label_margin)
            .contains(&screen_x)
        {
            continue;
        }
        axis_label(
            painter,
            canvas_rect,
            egui::pos2(screen_x + 3.0, x_label_y),
            value,
            label_step,
            axis_color,
            "x",
        );
    }
    let y_label_x = if y_axis_x < canvas_rect.left() {
        canvas_rect.left() + 2.0
    } else if y_axis_x > canvas_rect.right() {
        canvas_rect.right() - 50.0
    } else {
        y_axis_x + 3.0
    };
    for index in 0..=((top - first_y) / label_step).floor().max(0.0) as usize {
        let value = first_y + index as f32 * label_step;
        if value.abs() < grid_step * 0.5 {
            continue;
        }
        let screen_y = world_to_screen_y(
            value,
            transform.translation.y,
            window.height(),
            orthographic.scale,
        );
        if !(canvas_rect.top() + label_margin..=canvas_rect.bottom() - label_margin)
            .contains(&screen_y)
        {
            continue;
        }
        axis_label(
            painter,
            canvas_rect,
            egui::pos2(y_label_x, screen_y - 8.0),
            value,
            label_step,
            axis_color,
            "y",
        );
    }
}

pub(crate) fn world_to_screen_y(
    world_y: f32,
    camera_y: f32,
    screen_height: f32,
    scale: f32,
) -> f32 {
    screen_height / 2.0 - (world_y - camera_y) / scale
}

fn axis_label(
    painter: &egui::Painter,
    canvas_rect: egui::Rect,
    position: egui::Pos2,
    value: f32,
    step: f32,
    color: egui::Color32,
    _axis: &str,
) {
    let font = egui::FontId::proportional(11.0);
    let galley = painter.layout_no_wrap(format_coordinate(value, step), font, color);
    let position = clamp_label_position(position, galley.size(), canvas_rect);
    painter
        .with_clip_rect(canvas_rect)
        .galley(position, galley, color);
}

pub(crate) fn clamp_label_position(
    position: egui::Pos2,
    text_size: egui::Vec2,
    bounds: egui::Rect,
) -> egui::Pos2 {
    let max_x = (bounds.right() - text_size.x).max(bounds.left());
    let max_y = (bounds.bottom() - text_size.y).max(bounds.top());
    egui::pos2(
        position.x.clamp(bounds.left(), max_x),
        position.y.clamp(bounds.top(), max_y),
    )
}

pub(crate) fn format_coordinate(value: f32, step: f32) -> String {
    let value = if value.abs() < step * 1e-4 {
        0.0
    } else {
        value
    };
    if value.abs() >= 1e6 || (value != 0.0 && value.abs() < 1e-4) {
        return scientific_coordinate(value);
    }
    let decimals = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else if step >= 0.001 {
        3
    } else if step >= 0.0001 {
        4
    } else if step >= 0.00001 {
        5
    } else {
        6
    };
    let fixed = format!("{value:.decimals$}");
    if decimals == 0 {
        fixed
    } else {
        fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

fn scientific_coordinate(value: f32) -> String {
    let formatted = format!("{value:.3e}");
    let Some((mantissa, exponent)) = formatted.split_once('e') else {
        return formatted;
    };
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
    let exponent = exponent.parse::<i32>().unwrap_or_default();
    format!("{mantissa}e{exponent}")
}

pub(crate) fn apply_theme(context: &egui::Context, theme: UiTheme) {
    match theme {
        UiTheme::System | UiTheme::Light => context.set_visuals(egui::Visuals::light()),
        UiTheme::Dark => context.set_visuals(egui::Visuals::dark()),
    }
}

pub(crate) fn egui_color(color: &str) -> egui::Color32 {
    let [red, green, blue] = rgb(color);
    egui::Color32::from_rgb(red, green, blue)
}

pub(crate) fn bevy_color(color: &str) -> Color {
    let [red, green, blue] = rgb(color);
    Color::srgb_u8(red, green, blue)
}

pub(crate) fn hex_color(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

pub(crate) fn rgb(color: &str) -> [u8; 3] {
    if color.len() == 7 && color.starts_with('#') {
        let parse = |range| u8::from_str_radix(&color[range], 16).unwrap_or_default();
        [parse(1..3), parse(3..5), parse(5..7)]
    } else {
        [21, 101, 192]
    }
}
