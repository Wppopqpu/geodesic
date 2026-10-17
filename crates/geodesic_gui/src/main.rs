//! Native Bevy canvas and `egui` application chrome.
#![forbid(unsafe_code)]

use bevy::prelude::*;
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    window::PrimaryWindow,
};
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use geodesic_core::{Construction, GeoCommand, GeometricObject, Label, ProjectivePoint};
use geodesic_io::Project;

const SIDE_PANEL_WIDTH: f32 = 220.0;
const TOOLBAR_HEIGHT: f32 = 36.0;
const COMMAND_BAR_HEIGHT: f32 = 44.0;
const COMPLETION_ROW_HEIGHT: f32 = 24.0;
const MAX_COMPLETIONS: usize = 8;

#[derive(Resource, Default)]
struct GuiState {
    project: Project,
    command: String,
    command_bar_visible: bool,
    command_bar_focus_requested: bool,
    command_history: Vec<String>,
    history_cursor: Option<usize>,
    completion_query: String,
    completion_index: usize,
    message: String,
    theme: UiTheme,
    algebra_width: f32,
    style_width: f32,
    selected: Option<Label>,
    dragging: Option<Label>,
    preview: Option<(Label, ProjectivePoint)>,
    pan_cursor: Option<Vec2>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum UiTheme {
    #[default]
    System,
    Light,
    Dark,
}

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.97, 0.97, 0.98)))
        .insert_resource(GuiState::default())
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (draw_canvas, application_chrome, canvas_input))
        .run();
}

fn setup(mut commands: Commands, mut state: ResMut<GuiState>) {
    state.algebra_width = SIDE_PANEL_WIDTH;
    state.style_width = SIDE_PANEL_WIDTH;
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = 0.02;
    commands.spawn((Camera2d, Projection::Orthographic(projection)));
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    reason = "Bevy system resources are passed through ECS wrappers."
)]
fn canvas_input(
    mut mouse_wheel: EventReader<MouseWheel>,
    mut mouse_motion: EventReader<MouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut state: ResMut<GuiState>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    let Projection::Orthographic(orthographic) = projection.as_mut() else {
        return;
    };
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    if let (Ok(window), Some(cursor)) = (windows.single(), cursor) {
        let anchor = screen_to_world(cursor, window, transform.translation, orthographic.scale);
        for event in mouse_wheel.read() {
            let Some(next_scale) = next_zoom_scale(orthographic.scale, 1.0 - event.y * 0.1) else {
                continue;
            };
            orthographic.scale = next_scale;
            let after = screen_to_world(cursor, window, transform.translation, orthographic.scale);
            let correction = anchor - after;
            transform.translation.x += correction.x;
            transform.translation.y += correction.y;
        }
    }
    if mouse_buttons.pressed(MouseButton::Middle) {
        for event in mouse_motion.read() {
            transform.translation.x -= event.delta.x * orthographic.scale;
            transform.translation.y += event.delta.y * orthographic.scale;
        }
    }

    let (Ok(window), Some(cursor)) = (windows.single(), cursor) else {
        return;
    };
    let world = screen_to_world(cursor, window, transform.translation, orthographic.scale);
    if mouse_buttons.just_pressed(MouseButton::Left) {
        state.selected = state
            .project
            .construction
            .evaluate()
            .ok()
            .and_then(|snapshot| {
                snapshot.iter().find_map(|(label, object)| match object {
                    GeometricObject::Point(point) => match point.renderability() {
                        geodesic_core::Renderability::Renderable(point)
                            if Vec2::new(point.x as f32, point.y as f32).distance(world)
                                <= 0.3 * orthographic.scale =>
                        {
                            Some(label.clone())
                        }
                        _ => None,
                    },
                    GeometricObject::Line(_) => None,
                })
            });
        state.dragging = state.selected.clone();
        state.pan_cursor = (state.dragging.is_none()).then_some(cursor);
    }
    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(label) = &state.dragging {
            state.preview = Some((
                label.clone(),
                ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
            ));
        } else if let Some(previous) = state.pan_cursor {
            let delta = cursor - previous;
            transform.translation.x -= delta.x * orthographic.scale;
            transform.translation.y += delta.y * orthographic.scale;
            state.pan_cursor = Some(cursor);
        }
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some((label, point)) = state.preview.take() {
            state.message = state
                .project
                .construction
                .apply(GeoCommand::MovePoint { label, point })
                .map_or_else(
                    |error| format!("error: {error}"),
                    |()| "moved selected point".to_owned(),
                );
        }
        state.dragging = None;
        state.pan_cursor = None;
    }
}

fn screen_to_world(cursor: Vec2, window: &Window, translation: Vec3, scale: f32) -> Vec2 {
    Vec2::new(
        (cursor.x - window.width() / 2.0) * scale + translation.x,
        (window.height() / 2.0 - cursor.y) * scale + translation.y,
    )
}

fn next_zoom_scale(current: f32, factor: f32) -> Option<f32> {
    let next = current * factor;
    (current.is_finite()
        && current > 0.0
        && factor.is_finite()
        && factor > 0.0
        && next.is_finite()
        && next > f32::MIN_POSITIVE)
        .then_some(next)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Bevy system parameters are owned wrappers and its 2D renderer uses f32."
)]
fn draw_canvas(
    mut gizmos: Gizmos,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    state: Res<GuiState>,
) {
    let Some((left, right, bottom, top, scale)) = visible_world_bounds(&windows, &camera) else {
        return;
    };

    let step = grid_step(scale);
    let first_x = (left / step).floor() * step;
    let first_y = (bottom / step).floor() * step;
    let x_count = (((right - first_x) / step).ceil().max(0.0) as usize) + 1;
    let y_count = (((top - first_y) / step).ceil().max(0.0) as usize) + 1;
    for index in 0..x_count {
        let value = first_x + index as f32 * step;
        let grid = Color::srgb(0.88, 0.88, 0.9);
        gizmos.line_2d(Vec2::new(value, bottom), Vec2::new(value, top), grid);
    }
    for index in 0..y_count {
        let value = first_y + index as f32 * step;
        let grid = Color::srgb(0.88, 0.88, 0.9);
        gizmos.line_2d(Vec2::new(left, value), Vec2::new(right, value), grid);
    }
    let axis = Color::srgb(0.35, 0.35, 0.4);
    gizmos.line_2d(Vec2::new(left, 0.0), Vec2::new(right, 0.0), axis);
    gizmos.line_2d(Vec2::new(0.0, bottom), Vec2::new(0.0, top), axis);
    if let Ok(snapshot) = state.project.construction.evaluate() {
        for (label, object) in snapshot.iter() {
            if let geodesic_core::GeometricObject::Point(point) = object
                && let geodesic_core::Renderability::Renderable(point) = point.renderability()
            {
                let point = state
                    .preview
                    .as_ref()
                    .filter(|(preview_label, _)| *preview_label == *label)
                    .and_then(|(_, point)| match point.renderability() {
                        geodesic_core::Renderability::Renderable(point) => Some(point),
                        _ => None,
                    })
                    .unwrap_or(point);
                let style = geodesic_io::effective_style(&state.project, label);
                let color = if state.selected.as_ref() == Some(label) {
                    Color::srgb(0.9, 0.2, 0.15)
                } else {
                    bevy_color(&style.color)
                };
                gizmos.circle_2d(
                    Vec2::new(point.x as f32, point.y as f32),
                    style.size as f32,
                    color,
                );
            }
            if let geodesic_core::GeometricObject::Line(line) = object {
                let [a, b, c] = line.coefficients();
                if a.im.abs() < geodesic_core::STANDARD_PRECISION
                    && b.im.abs() < geodesic_core::STANDARD_PRECISION
                    && c.im.abs() < geodesic_core::STANDARD_PRECISION
                    && let Some((start, end)) = clipped_line(
                        a.re as f32,
                        b.re as f32,
                        c.re as f32,
                        left,
                        right,
                        bottom,
                        top,
                    )
                {
                    let style = geodesic_io::effective_style(&state.project, label);
                    gizmos.line_2d(start, end, bevy_color(&style.color));
                }
            }
        }
    }
}

fn visible_world_bounds(
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Transform, &Projection), With<Camera2d>>,
) -> Option<(f32, f32, f32, f32, f32)> {
    let window = windows.single().ok()?;
    let (transform, projection) = camera.single().ok()?;
    let Projection::Orthographic(orthographic) = projection else {
        return None;
    };
    let half_width = window.width() * orthographic.scale / 2.0;
    let half_height = window.height() * orthographic.scale / 2.0;
    Some((
        transform.translation.x - half_width,
        transform.translation.x + half_width,
        transform.translation.y - half_height,
        transform.translation.y + half_height,
        orthographic.scale,
    ))
}

/// Chooses a 1/2/5 grid step that keeps adjacent grid lines about 50 pixels apart.
fn grid_step(scale: f32) -> f32 {
    nice_step((50.0 * scale).max(f32::MIN_POSITIVE))
}

fn nice_step(minimum: f32) -> f32 {
    let exponent = minimum.log10().floor();
    let magnitude = 10.0_f32.powf(exponent);
    let normalized = minimum / magnitude;
    let multiplier = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    multiplier * magnitude
}

fn clipped_line(
    a: f32,
    b: f32,
    c: f32,
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
) -> Option<(Vec2, Vec2)> {
    let mut points = Vec::with_capacity(4);
    if b.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        for x in [left, right] {
            let y = -(a * x + c) / b;
            if (bottom..=top).contains(&y) {
                points.push(Vec2::new(x, y));
            }
        }
    }
    if a.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        for y in [bottom, top] {
            let x = -(b * y + c) / a;
            if (left..=right).contains(&x)
                && !points.iter().any(|point| {
                    point.distance(Vec2::new(x, y)) < geodesic_core::STANDARD_PRECISION_F32
                })
            {
                points.push(Vec2::new(x, y));
            }
        }
    }
    points.first().copied().zip(points.get(1).copied())
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    reason = "Bevy systems use owned wrappers and the renderer uses f32 coordinates."
)]
#[allow(dead_code)]
fn select_and_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    mut state: ResMut<GuiState>,
) {
    let (Ok(window), Ok((transform, projection))) = (windows.single(), camera.single()) else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Projection::Orthographic(orthographic) = projection else {
        return;
    };
    let world = Vec2::new(
        (cursor.x - window.width() / 2.0) * orthographic.scale + transform.translation.x,
        (window.height() / 2.0 - cursor.y) * orthographic.scale + transform.translation.y,
    );
    if mouse_buttons.just_pressed(MouseButton::Left) {
        state.selected = state
            .project
            .construction
            .evaluate()
            .ok()
            .and_then(|snapshot| {
                snapshot.iter().find_map(|(label, object)| match object {
                    geodesic_core::GeometricObject::Point(point) => match point.renderability() {
                        geodesic_core::Renderability::Renderable(point)
                            if Vec2::new(point.x as f32, point.y as f32).distance(world)
                                <= 0.3 * orthographic.scale =>
                        {
                            Some(label.clone())
                        }
                        _ => None,
                    },
                    geodesic_core::GeometricObject::Line(_) => None,
                })
            });
        state.dragging = state.selected.clone();
    }
    if mouse_buttons.pressed(MouseButton::Left)
        && let Some(label) = &state.dragging
    {
        state.preview = Some((
            label.clone(),
            ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
        ));
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some((label, point)) = state.preview.take() {
            state.message = state
                .project
                .construction
                .apply(GeoCommand::MovePoint { label, point })
                .map_or_else(
                    |error| format!("error: {error}"),
                    |()| "moved selected point".to_owned(),
                );
        }
        state.dragging = None;
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    reason = "The application chrome keeps related egui panels in one system."
)]
fn application_chrome(
    mut contexts: EguiContexts,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    mut state: ResMut<GuiState>,
) {
    let context = contexts.ctx_mut();
    if !state.command_bar_visible
        && !context.wants_keyboard_input()
        && context.input(|input| input.key_pressed(egui::Key::Colon))
    {
        state.command_bar_visible = true;
        state.command_bar_focus_requested = true;
    }
    // Consume Tab before any toolbar widgets are laid out. Consuming it in the
    // command-bar panel is too late: the toolbar's focusable buttons are
    // created first and can receive focus during egui's traversal.
    let command_bar_tab_pressed = state.command_bar_visible
        && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
    if command_bar_tab_pressed {
        state.command_bar_focus_requested = true;
    }
    draw_axis_labels(
        context,
        &windows,
        &camera,
        state.command_bar_visible,
        state.algebra_width,
        state.style_width,
    );
    egui::TopBottomPanel::top("toolbar")
        .exact_height(TOOLBAR_HEIGHT)
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Geodesic");
                ui.label("Real affine canvas · complex projective kernel");
                ui.separator();
                ui.label("Theme:");
                for (theme, name) in [
                    (UiTheme::System, "System"),
                    (UiTheme::Light, "Light"),
                    (UiTheme::Dark, "Dark"),
                ] {
                    if ui.selectable_label(state.theme == theme, name).clicked() {
                        state.theme = theme;
                        apply_theme(context, theme);
                    }
                }
            });
        });
    egui::SidePanel::left("algebra")
        .default_width(state.algebra_width)
        .min_width(160.0)
        .resizable(true)
        .show(context, |ui| {
            state.algebra_width = ui.available_width();
            ui.heading("Algebra");
            match state.project.construction.evaluate() {
                Ok(snapshot) => {
                    for (label, object) in snapshot.iter() {
                        ui.label(format!("{label}: {object:?}"));
                    }
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::RED, error.to_string());
                }
            }
        });
    egui::SidePanel::right("style_editor")
        .default_width(state.style_width)
        .min_width(160.0)
        .resizable(true)
        .show(context, |ui| {
            state.style_width = ui.available_width();
            ui.heading("Style");
            if let Some(label) = state.selected.clone() {
                ui.label(label.to_string());
                let presets: Vec<_> = state.project.style_presets.keys().cloned().collect();
                if ui.button("Create emphasis preset").clicked()
                    && let Ok(preset) = Label::parse("emphasis")
                {
                    state
                        .project
                        .style_presets
                        .entry(preset)
                        .or_insert(geodesic_io::ObjectStyle {
                            color: "#d32f2f".to_owned(),
                            size: 0.25,
                            opacity: 1.0,
                        });
                }
                for preset in presets {
                    if ui.button(format!("Use preset {preset}")).clicked() {
                        state.project.object_presets.insert(label.clone(), preset);
                        state.project.object_styles.remove(&label);
                    }
                }
                if state.project.object_presets.contains_key(&label) {
                    let preset = &state.project.object_presets[&label];
                    ui.label(format!("Using preset {preset}"));
                    if ui.button("Use direct override").clicked() {
                        state.project.object_presets.remove(&label);
                        state
                            .project
                            .object_styles
                            .entry(label.clone())
                            .or_default();
                    }
                } else {
                    let style = state.project.object_styles.entry(label).or_default();
                    let mut color = egui_color(&style.color);
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        style.color = hex_color(color);
                    }
                    ui.add(egui::Slider::new(&mut style.size, 0.02..=1.0).text("Size"));
                    ui.add(egui::Slider::new(&mut style.opacity, 0.0..=1.0).text("Opacity"));
                }
            } else {
                ui.label("Select a real affine point to edit its direct style.");
            }
        });
    if !state.command_bar_visible {
        return;
    }

    let candidates = command_completions(&state.project.construction, &state.command);
    if state.completion_query != state.command {
        let command = state.command.clone();
        state.completion_query.clone_from(&command);
        state.completion_index = 0;
    }
    if !candidates.is_empty() {
        state.completion_index %= candidates.len();
    }

    egui::TopBottomPanel::bottom("command_bar")
        .exact_height(COMMAND_BAR_HEIGHT + COMPLETION_ROW_HEIGHT * candidates.len() as f32)
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.label(":");
                ui.vertical(|ui| {
                    let response = egui::TextEdit::singleline(&mut state.command)
                        .lock_focus(true)
                        .show(ui)
                        .response;
                    let input = ui.input(|input| {
                        (
                            input.key_pressed(egui::Key::ArrowDown),
                            input.key_pressed(egui::Key::ArrowUp),
                            input.key_pressed(egui::Key::Enter),
                            input.key_pressed(egui::Key::Escape),
                        )
                    });
                    if state.command_bar_focus_requested {
                        response.request_focus();
                        state.command_bar_focus_requested = false;
                    }
                    let focused = response.has_focus();
                    // Consume Tab for the whole command-bar pass. Focus traversal happens
                    // against the previous frame's widgets, so checking `focused` here can be
                    // too late after egui has already moved focus to a completion label.
                    let mut accept_completion = false;

                    if focused && !candidates.is_empty() {
                        if input.0 {
                            state.completion_index =
                                (state.completion_index + 1) % candidates.len();
                        } else if input.1 {
                            state.completion_index = state
                                .completion_index
                                .checked_sub(1)
                                .unwrap_or(candidates.len() - 1);
                        }
                        accept_completion = command_bar_tab_pressed;
                    } else if focused && input.0 {
                        let history = state.command_history.clone();
                        let mut command = state.command.clone();
                        let mut cursor = state.history_cursor;
                        history_next(&mut command, &mut cursor, &history);
                        state.command = command;
                        state.history_cursor = cursor;
                    } else if focused && input.1 {
                        let history = state.command_history.clone();
                        let mut command = state.command.clone();
                        let mut cursor = state.history_cursor;
                        history_previous(&mut command, &mut cursor, &history);
                        state.command = command;
                        state.history_cursor = cursor;
                    }

                    if accept_completion {
                        state.command =
                            replace_completion(&state.command, &candidates[state.completion_index]);
                    } else if input.2 {
                        submit_command(&mut state);
                    } else if input.3 {
                        state.command_bar_visible = false;
                    }

                    if !candidates.is_empty() {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_min_width(response.rect.width());
                            for (index, candidate) in candidates.iter().enumerate() {
                                let selected = index == state.completion_index;
                                let button = egui::Button::new(candidate)
                                    .selected(selected)
                                    // Completion choices are mouse-clickable, but must not
                                    // become part of Tab focus traversal.
                                    .sense(egui::Sense::CLICK);
                                if ui.add(button).clicked() {
                                    state.command = replace_completion(&state.command, candidate);
                                }
                            }
                        });
                    }
                    if command_bar_tab_pressed {
                        response.request_focus();
                    }
                    if !state.message.is_empty() {
                        ui.label(&state.message);
                    }
                });
            });
            ui.small(command_hint(&state.command));
        });
}

fn submit_command(state: &mut GuiState) {
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
fn draw_axis_labels(
    context: &egui::Context,
    windows: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Transform, &Projection), With<Camera2d>>,
    command_bar_visible: bool,
    algebra_width: f32,
    style_width: f32,
) {
    let Some((left, right, bottom, top, scale)) = visible_world_bounds(windows, camera) else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let canvas_rect = egui::Rect::from_min_max(
        egui::pos2(algebra_width, TOOLBAR_HEIGHT),
        egui::pos2(
            window.width() - style_width,
            window.height()
                - if command_bar_visible {
                    COMMAND_BAR_HEIGHT
                } else {
                    0.0
                },
        ),
    );
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
            context,
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
            context,
            canvas_rect,
            egui::pos2(y_label_x, screen_y - 8.0),
            value,
            label_step,
            axis_color,
            "y",
        );
    }
}

fn world_to_screen_y(world_y: f32, camera_y: f32, screen_height: f32, scale: f32) -> f32 {
    screen_height / 2.0 - (world_y - camera_y) / scale
}

fn axis_label(
    context: &egui::Context,
    canvas_rect: egui::Rect,
    position: egui::Pos2,
    value: f32,
    step: f32,
    color: egui::Color32,
    _axis: &str,
) {
    let painter = context.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("axis_labels"),
    ));
    let font = egui::FontId::proportional(11.0);
    let galley = painter.layout_no_wrap(format_coordinate(value, step), font, color);
    let max_x = (canvas_rect.right() - galley.size().x).max(canvas_rect.left());
    let max_y = (canvas_rect.bottom() - galley.size().y).max(canvas_rect.top());
    let position = egui::pos2(
        position.x.clamp(canvas_rect.left(), max_x),
        position.y.clamp(canvas_rect.top(), max_y),
    );
    painter
        .with_clip_rect(canvas_rect)
        .galley(position, galley, color);
}

fn format_coordinate(value: f32, step: f32) -> String {
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

fn apply_theme(context: &egui::Context, theme: UiTheme) {
    match theme {
        UiTheme::System | UiTheme::Light => context.set_visuals(egui::Visuals::light()),
        UiTheme::Dark => context.set_visuals(egui::Visuals::dark()),
    }
}

fn egui_color(color: &str) -> egui::Color32 {
    let [red, green, blue] = rgb(color);
    egui::Color32::from_rgb(red, green, blue)
}

fn bevy_color(color: &str) -> Color {
    let [red, green, blue] = rgb(color);
    Color::srgb_u8(red, green, blue)
}

fn hex_color(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

fn rgb(color: &str) -> [u8; 3] {
    if color.len() == 7 && color.starts_with('#') {
        let parse = |range| u8::from_str_radix(&color[range], 16).unwrap_or_default();
        [parse(1..3), parse(3..5), parse(5..7)]
    } else {
        [21, 101, 192]
    }
}

fn execute_command(construction: &mut Construction, source: &str) -> Result<String, String> {
    let fields: Vec<_> = source
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .collect();
    let command = fields.first().ok_or("empty command")?;
    match *command {
        "point" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            let x = parse_coordinate(fields[2])?;
            let y = parse_coordinate(fields[3])?;
            construction
                .apply(GeoCommand::AddPoint {
                    label: label.clone(),
                    point: ProjectivePoint::real_affine(x, y),
                })
                .map_err(|error| error.to_string())?;
            Ok(format!("created point {label}"))
        }
        "line" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            construction
                .apply(GeoCommand::AddLineThrough {
                    label: label.clone(),
                    first: parse_label(fields[2])?,
                    second: parse_label(fields[3])?,
                })
                .map_err(|error| error.to_string())?;
            Ok(format!("created line {label}"))
        }
        "move" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            let x = parse_coordinate(fields[2])?;
            let y = parse_coordinate(fields[3])?;
            construction
                .apply(GeoCommand::MovePoint {
                    label: label.clone(),
                    point: ProjectivePoint::real_affine(x, y),
                })
                .map_err(|error| error.to_string())?;
            Ok(format!("moved point {label}"))
        }
        "undo" if fields.len() == 1 => construction
            .undo()
            .map(|()| "undid command".to_owned())
            .map_err(|error| error.to_string()),
        "redo" if fields.len() == 1 => construction
            .redo()
            .map(|()| "redid command".to_owned())
            .map_err(|error| error.to_string()),
        _ => Err(command_hint(source).to_owned()),
    }
}

const COMMAND_NAMES: [&str; 5] = ["point", "line", "move", "undo", "redo"];

fn command_completions(construction: &Construction, source: &str) -> Vec<String> {
    let trimmed = source.trim_start();
    let without_colon = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let fields: Vec<_> = without_colon.split_whitespace().collect();
    let trailing_space = without_colon.ends_with(char::is_whitespace);
    let position = if trailing_space {
        fields.len()
    } else {
        fields.len().saturating_sub(1)
    };
    let partial = if trailing_space {
        ""
    } else {
        fields.last().copied().unwrap_or_default()
    };

    if fields.is_empty() || position == 0 {
        return COMMAND_NAMES
            .into_iter()
            .filter(|name| name.starts_with(partial))
            .map(str::to_owned)
            .collect();
    }

    let command = fields[0];
    let labels = labels_for_position(construction, command, position);
    labels
        .into_iter()
        .filter(|label| label.starts_with(partial))
        .take(MAX_COMPLETIONS)
        .collect()
}

fn labels_for_position(construction: &Construction, command: &str, position: usize) -> Vec<String> {
    let Ok(snapshot) = construction.evaluate() else {
        return Vec::new();
    };
    snapshot
        .iter()
        .filter(|(_, object)| match command {
            "point" | "line" if position == 1 => true,
            "move" if position == 1 => matches!(object, GeometricObject::Point(_)),
            "line" if position >= 2 => matches!(object, GeometricObject::Point(_)),
            _ => false,
        })
        .map(|(label, _)| label.to_string())
        .collect()
}

fn replace_completion(source: &str, completion: &str) -> String {
    let token_start = source
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or_else(
            || usize::from(source.starts_with(':')),
            |(index, character)| index + character.len_utf8(),
        );
    format!("{}{}", &source[..token_start], completion)
}

fn command_hint(source: &str) -> &'static str {
    let fields: Vec<_> = source
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .collect();
    match fields.first().copied() {
        None => {
            "commands: :point label x y | :line label point_a point_b | :move label x y | :undo | :redo"
        }
        Some("point") => "usage: :point label x y",
        Some("line") => "usage: :line label point_a point_b",
        Some("move") => "usage: :move label x y",
        Some("undo") => "usage: :undo",
        Some("redo") => "usage: :redo",
        Some(_) => "unknown command; use :point, :line, :move, :undo, or :redo",
    }
}

fn history_previous(command: &mut String, cursor: &mut Option<usize>, history: &[String]) {
    if history.is_empty() {
        return;
    }
    let index = cursor.map_or(history.len() - 1, |index| index.saturating_sub(1));
    *cursor = Some(index);
    command.clone_from(&history[index]);
}

fn history_next(command: &mut String, cursor: &mut Option<usize>, history: &[String]) {
    let Some(index) = *cursor else {
        return;
    };
    if index + 1 < history.len() {
        let next = index + 1;
        *cursor = Some(next);
        command.clone_from(&history[next]);
    } else {
        *cursor = None;
        command.clear();
    }
}

fn parse_label(source: &str) -> Result<Label, String> {
    Label::parse(source).map_err(|error| error.to_string())
}

fn parse_coordinate(source: &str) -> Result<f64, String> {
    source
        .parse()
        .map_err(|_| format!("invalid coordinate `{source}`"))
}

#[cfg(test)]
mod tests {
    use super::{
        GuiState, UiTheme, command_completions, command_hint, execute_command, hex_color,
        history_next, history_previous, replace_completion, rgb,
    };
    use geodesic_core::Construction;

    #[test]
    fn ex_commands_share_core_history() {
        let mut construction = Construction::new();
        execute_command(&mut construction, ":point point_a 0 0").expect("point a");
        execute_command(&mut construction, ":point point_b 1 1").expect("point b");
        execute_command(&mut construction, ":line diagonal point_a point_b").expect("line");
        assert_eq!(
            construction.evaluate().expect("evaluation").iter().count(),
            3
        );
        execute_command(&mut construction, ":undo").expect("undo");
        assert_eq!(
            construction.evaluate().expect("evaluation").iter().count(),
            2
        );
        execute_command(&mut construction, ":redo").expect("redo");
        assert_eq!(
            construction.evaluate().expect("evaluation").iter().count(),
            3
        );
        execute_command(&mut construction, ":move point_a 2 3").expect("move");
    }

    #[test]
    fn completes_command_names_from_the_current_token() {
        let construction = Construction::new();
        assert_eq!(command_completions(&construction, ":p"), vec!["point"]);
        assert_eq!(command_completions(&construction, ":un"), vec!["undo"]);
        assert_eq!(
            command_completions(&construction, ":"),
            vec!["point", "line", "move", "undo", "redo"]
        );
    }

    #[test]
    fn completes_labels_only_for_label_arguments() {
        let mut construction = Construction::new();
        execute_command(&mut construction, ":point point_a 0 0").expect("point a");
        execute_command(&mut construction, ":point point_b 1 1").expect("point b");

        assert_eq!(
            command_completions(&construction, ":line diagonal point_"),
            vec!["point_a", "point_b"]
        );
        assert!(command_completions(&construction, ":point point_ ").is_empty());
        assert_eq!(
            command_completions(&construction, ":move point_"),
            vec!["point_a", "point_b"]
        );
    }

    #[test]
    fn limits_label_completions_to_the_visible_candidate_count() {
        let mut construction = Construction::new();
        for index in 0..(super::MAX_COMPLETIONS + 2) {
            execute_command(&mut construction, &format!(":point point_{index:02} 0 0"))
                .expect("point");
        }

        let completions = command_completions(&construction, ":move point_");
        assert_eq!(completions.len(), super::MAX_COMPLETIONS);
        assert_eq!(completions.first().map(String::as_str), Some("point_00"));
        assert_eq!(completions.last().map(String::as_str), Some("point_07"));
    }

    #[test]
    fn completion_replaces_only_the_current_token() {
        assert_eq!(
            replace_completion(":line diagonal point_", "point_a"),
            ":line diagonal point_a"
        );
        assert_eq!(replace_completion(":po", "point"), ":point");
        assert_eq!(
            replace_completion(":point point_a  ", "point_b"),
            ":point point_a  point_b"
        );
    }

    #[test]
    fn command_hints_describe_the_expected_arguments() {
        assert_eq!(
            command_hint(":"),
            "commands: :point label x y | :line label point_a point_b | :move label x y | :undo | :redo"
        );
        assert_eq!(
            command_hint(":line diagonal"),
            "usage: :line label point_a point_b"
        );
        assert_eq!(
            command_hint(":unknown"),
            "unknown command; use :point, :line, :move, :undo, or :redo"
        );
    }

    #[test]
    fn command_history_moves_older_and_newer_without_losing_entries() {
        let history = vec![":point point_a 0 0".to_owned(), ":undo".to_owned()];
        let mut command = String::new();
        let mut cursor = None;
        history_previous(&mut command, &mut cursor, &history);
        assert_eq!(command, ":undo");
        history_previous(&mut command, &mut cursor, &history);
        assert_eq!(command, ":point point_a 0 0");
        history_next(&mut command, &mut cursor, &history);
        assert_eq!(command, ":undo");
        history_next(&mut command, &mut cursor, &history);
        assert!(command.is_empty());
        assert_eq!(cursor, None);
    }

    #[test]
    fn command_bar_is_hidden_by_default() {
        assert!(!GuiState::default().command_bar_visible);
        assert!(!GuiState::default().command_bar_focus_requested);
    }

    #[test]
    fn submitting_command_does_not_depend_on_text_focus() {
        let mut state = GuiState {
            command_bar_visible: true,
            command: ":point point_a 0 0".to_owned(),
            ..GuiState::default()
        };
        super::submit_command(&mut state);
        assert!(!state.command_bar_visible);
        assert_eq!(
            state
                .project
                .construction
                .evaluate()
                .expect("evaluation")
                .iter()
                .count(),
            1
        );
    }

    #[test]
    fn user_theme_choices_remain_distinct() {
        assert_ne!(UiTheme::Light, UiTheme::Dark);
        assert_ne!(UiTheme::System, UiTheme::Dark);
    }

    #[test]
    fn zoom_rejects_nonpositive_factors() {
        assert_eq!(super::next_zoom_scale(1.0, 0.0), None);
    }

    #[test]
    fn zoom_accepts_large_finite_scales_but_rejects_invalid_results() {
        assert_eq!(super::next_zoom_scale(1_000.0, 2.0), Some(2_000.0));
        assert_eq!(super::next_zoom_scale(1.0, 0.0), None);
        assert_eq!(super::next_zoom_scale(f32::INFINITY, 2.0), None);
    }

    #[test]
    fn formats_editor_colors_as_persistable_hex() {
        assert_eq!(rgb("#1565c0"), [21, 101, 192]);
        assert_eq!(
            hex_color(bevy_egui::egui::Color32::from_rgb(255, 0, 16)),
            "#ff0010"
        );
    }

    #[test]
    fn clips_real_line_to_initial_canvas_extent() {
        let (start, end) =
            super::clipped_line(0.0, 1.0, 0.0, -10.0, 10.0, -10.0, 10.0).expect("horizontal line");
        assert_eq!(start, bevy::prelude::Vec2::new(-10.0, 0.0));
        assert_eq!(end, bevy::prelude::Vec2::new(10.0, 0.0));
    }

    #[test]
    fn selects_125_grid_steps_from_camera_scale() {
        for (scale, expected) in [(0.02, 1.0), (0.2, 10.0), (0.002, 0.1)] {
            assert!((super::grid_step(scale) - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn formats_tick_values_for_the_visible_grid_scale() {
        assert_eq!(super::format_coordinate(12.0, 1.0), "12");
        assert_eq!(super::format_coordinate(0.5, 0.5), "0.5");
        assert_eq!(super::format_coordinate(0.25, 0.05), "0.25");
        assert_eq!(super::format_coordinate(-0.000_001, 0.1), "0");
    }

    #[test]
    fn uses_compact_scientific_notation_for_extreme_ticks() {
        assert_eq!(super::format_coordinate(1_000_000.0, 1.0), "1e6");
        assert_eq!(super::format_coordinate(2.5e7, 1.0), "2.5e7");
        assert_eq!(super::format_coordinate(0.00001, 0.00001), "1e-5");
    }

    #[test]
    fn projects_horizontal_axis_using_camera_translation_direction() {
        assert!((super::world_to_screen_y(0.0, 1.0, 100.0, 0.5) - 52.0).abs() < f32::EPSILON);
    }
}
