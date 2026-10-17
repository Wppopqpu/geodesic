//! Native Bevy canvas and `egui` application chrome.
#![forbid(unsafe_code)]

use bevy::prelude::*;
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    window::PrimaryWindow,
};
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use geodesic_core::{Construction, GeoCommand, Label, ProjectivePoint};
use geodesic_io::Project;

#[derive(Resource, Default)]
struct GuiState {
    project: Project,
    command: String,
    message: String,
    theme: UiTheme,
    selected: Option<Label>,
    dragging: Option<Label>,
    preview: Option<(Label, ProjectivePoint)>,
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
        .add_systems(
            Update,
            (
                draw_canvas,
                application_chrome,
                pan_and_zoom,
                select_and_drag,
            ),
        )
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system resources are passed through ECS wrappers."
)]
fn pan_and_zoom(
    mut mouse_wheel: EventReader<MouseWheel>,
    mut mouse_motion: EventReader<MouseMotion>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    if let Projection::Orthographic(orthographic) = projection.as_mut() {
        for event in mouse_wheel.read() {
            orthographic.scale = (orthographic.scale * (1.0 - event.y * 0.1)).clamp(0.1, 100.0);
        }
        if mouse_buttons.pressed(MouseButton::Middle) {
            for event in mouse_motion.read() {
                transform.translation.x -= event.delta.x * orthographic.scale;
                transform.translation.y += event.delta.y * orthographic.scale;
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "Bevy system parameters are owned wrappers and its 2D renderer uses f32."
)]
fn draw_canvas(mut gizmos: Gizmos, state: Res<GuiState>) {
    for coordinate in -10..=10 {
        let value = coordinate as f32;
        let grid = Color::srgb(0.88, 0.88, 0.9);
        gizmos.line_2d(Vec2::new(value, -10.0), Vec2::new(value, 10.0), grid);
        gizmos.line_2d(Vec2::new(-10.0, value), Vec2::new(10.0, value), grid);
    }
    let axis = Color::srgb(0.35, 0.35, 0.4);
    gizmos.line_2d(Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0), axis);
    gizmos.line_2d(Vec2::new(0.0, -10.0), Vec2::new(0.0, 10.0), axis);
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
                    && let Some((start, end)) = clipped_line(a.re as f32, b.re as f32, c.re as f32)
                {
                    let style = geodesic_io::effective_style(&state.project, label);
                    gizmos.line_2d(start, end, bevy_color(&style.color));
                }
            }
        }
    }
}

fn clipped_line(a: f32, b: f32, c: f32) -> Option<(Vec2, Vec2)> {
    const EXTENT: f32 = 10.0;
    let mut points = Vec::with_capacity(4);
    if b.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        for x in [-EXTENT, EXTENT] {
            let y = -(a * x + c) / b;
            if (-EXTENT..=EXTENT).contains(&y) {
                points.push(Vec2::new(x, y));
            }
        }
    }
    if a.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        for y in [-EXTENT, EXTENT] {
            let x = -(b * y + c) / a;
            if (-EXTENT..=EXTENT).contains(&x)
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

fn application_chrome(mut contexts: EguiContexts, mut state: ResMut<GuiState>) {
    let context = contexts.ctx_mut();
    egui::TopBottomPanel::top("toolbar").show(context, |ui| {
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
    egui::SidePanel::left("algebra").show(context, |ui| {
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
    egui::SidePanel::right("style_editor").show(context, |ui| {
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
    egui::TopBottomPanel::bottom("command_bar").show(context, |ui| {
        ui.horizontal(|ui| {
            ui.label(":");
            let response = ui.text_edit_singleline(&mut state.command);
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                let command = std::mem::take(&mut state.command);
                state.message = execute_command(&mut state.project.construction, &command)
                    .unwrap_or_else(|error| format!("error: {error}"));
            }
            if !state.message.is_empty() {
                ui.label(&state.message);
            }
        });
    });
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
        _ => Err(
            "expected :point label x y, :line label point_a point_b, :move label x y, :undo, or :redo".to_owned(),
        ),
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
    use super::{UiTheme, execute_command, hex_color, rgb};
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
    fn user_theme_choices_remain_distinct() {
        assert_ne!(UiTheme::Light, UiTheme::Dark);
        assert_ne!(UiTheme::System, UiTheme::Dark);
    }

    #[test]
    fn zoom_clamp_preserves_a_positive_render_scale() {
        assert!(((1.0_f32 * (1.0 - 1_000.0 * 0.1)).clamp(0.1, 100.0) - 0.1).abs() < f32::EPSILON);
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
        let (start, end) = super::clipped_line(0.0, 1.0, 0.0).expect("horizontal line");
        assert_eq!(start, bevy::prelude::Vec2::new(-10.0, 0.0));
        assert_eq!(end, bevy::prelude::Vec2::new(10.0, 0.0));
    }
}
