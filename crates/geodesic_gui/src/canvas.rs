use bevy::prelude::*;
use bevy::{
    color::Alpha,
    input::mouse::{MouseMotion, MouseWheel},
    window::PrimaryWindow,
};
use geodesic_core::{
    GeoCommand, GeometricObject, ObjectDefinition, ProjectiveLine, ProjectivePoint,
    parallel_through,
};

use crate::presentation::bevy_color;
use crate::state::{DragPreview, DragTarget, GuiState, TOOLBAR_HEIGHT};

/// The initial camera scale makes one style unit equal to 50 screen pixels.
///
/// Point styles remain persisted in project/SVG units, while the canvas adapts
/// their world-space radius to keep their on-screen radius constant as the
/// camera zooms.
const STYLE_UNITS_PER_PIXEL_AT_DEFAULT_SCALE: f32 = 0.02;
const POINT_SELECTION_RADIUS_PIXELS: f32 = 15.0;
const LINE_SELECTION_RADIUS_PIXELS: f32 = 3.0;

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    reason = "Bevy system resources are passed through ECS wrappers."
)]
pub(crate) fn canvas_input(
    mut mouse_wheel: EventReader<MouseWheel>,
    mut mouse_motion: EventReader<MouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut state: ResMut<GuiState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let cursor_is_on_canvas = canvas_contains(cursor, window, &state);
    if !cursor_is_on_canvas && state.dragging.is_none() && state.pan_cursor.is_none() {
        state.dragging = None;
        state.preview = None;
        state.pan_cursor = None;
        return;
    }
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };
    let Projection::Orthographic(orthographic) = projection.as_mut() else {
        return;
    };
    if cursor_is_on_canvas {
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

    let world = screen_to_world(cursor, window, transform.translation, orthographic.scale);
    if cursor_is_on_canvas && mouse_buttons.just_pressed(MouseButton::Left) {
        state.selected = select_canvas_object(&state.project, world, orthographic.scale);
        state.dragging = state
            .selected
            .as_ref()
            .and_then(|label| drag_target(&state.project.construction, label));
        state.pan_cursor = (state.selected.is_none()).then_some(cursor);
    }
    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(target) = &state.dragging {
            state.preview = drag_preview(target, world);
        } else if let Some(previous) = state.pan_cursor {
            let delta = cursor - previous;
            transform.translation.x -= delta.x * orthographic.scale;
            transform.translation.y += delta.y * orthographic.scale;
            state.pan_cursor = Some(cursor);
        }
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        if let Some(target) = state.dragging.take() {
            state.message = apply_drag(&mut state.project.construction, target, world)
                .unwrap_or_else(|error| format!("error: {error}"));
        }
        state.preview = None;
        state.pan_cursor = None;
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy's 2D renderer uses f32 coordinates."
)]
fn select_canvas_object(
    project: &geodesic_io::Project,
    world: Vec2,
    camera_scale: f32,
) -> Option<geodesic_core::Label> {
    let snapshot = project.construction.evaluate().ok()?;
    snapshot
        .iter()
        .find_map(|(label, object)| match object {
            GeometricObject::Point(point) => match point.renderability() {
                geodesic_core::Renderability::Renderable(point)
                    if point_is_hit(
                        Vec2::new(point.x as f32, point.y as f32),
                        world,
                        geodesic_io::effective_style(project, label).size,
                        camera_scale,
                    ) =>
                {
                    Some(label.clone())
                }
                _ => None,
            },
            GeometricObject::Line(_) => None,
        })
        .or_else(|| {
            snapshot.iter().find_map(|(label, object)| match object {
                GeometricObject::Line(line) if line_is_hit(*line, world, camera_scale) => {
                    Some(label.clone())
                }
                GeometricObject::Point(_) | GeometricObject::Line(_) => None,
            })
        })
}

fn drag_preview(target: &DragTarget, world: Vec2) -> Option<DragPreview> {
    match target {
        DragTarget::Point(label) => Some(DragPreview::Point {
            label: label.clone(),
            point: ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
        }),
        DragTarget::Line { label, line } => parallel_through(
            ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
            *line,
        )
        .ok()
        .map(|line| DragPreview::Line {
            label: label.clone(),
            line,
        }),
    }
}

fn apply_drag(
    construction: &mut geodesic_core::Construction,
    target: DragTarget,
    world: Vec2,
) -> Result<String, String> {
    let (command, message) = match target {
        DragTarget::Point(label) => (
            GeoCommand::MovePoint {
                label,
                point: ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
            },
            "moved selected point",
        ),
        DragTarget::Line { label, line } => (
            GeoCommand::MoveLine {
                label,
                line: parallel_through(
                    ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
                    line,
                )
                .map_err(|error| error.to_string())?,
            },
            "moved selected line",
        ),
    };
    construction
        .apply(command)
        .map(|()| message.to_owned())
        .map_err(|error| error.to_string())
}

fn drag_target(
    construction: &geodesic_core::Construction,
    label: &geodesic_core::Label,
) -> Option<DragTarget> {
    construction
        .definitions()
        .find_map(|(candidate, definition)| (candidate == label).then_some(definition))
        .and_then(|definition| match definition {
            ObjectDefinition::FreePoint(_) => Some(DragTarget::Point(label.clone())),
            ObjectDefinition::FreeLine(line) => Some(DragTarget::Line {
                label: label.clone(),
                line: *line,
            }),
            ObjectDefinition::LineThrough { .. }
            | ObjectDefinition::Intersection { .. }
            | ObjectDefinition::ParallelThrough { .. }
            | ObjectDefinition::PerpendicularThrough { .. } => None,
        })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy's 2D renderer uses f32 coordinates."
)]
pub(crate) fn point_radius_world(style_size: f64, camera_scale: f32) -> f32 {
    style_size as f32 * camera_scale / STYLE_UNITS_PER_PIXEL_AT_DEFAULT_SCALE
}

pub(crate) fn point_selection_radius_world(style_size: f64, camera_scale: f32) -> f32 {
    point_radius_world(style_size, camera_scale).max(POINT_SELECTION_RADIUS_PIXELS * camera_scale)
}

pub(crate) fn point_is_hit(point: Vec2, cursor: Vec2, style_size: f64, camera_scale: f32) -> bool {
    point.distance(cursor) <= point_selection_radius_world(style_size, camera_scale)
}

pub(crate) fn line_is_hit(line: ProjectiveLine, cursor: Vec2, camera_scale: f32) -> bool {
    let [a, b, c] = line.coefficients();
    if a.im.abs() >= geodesic_core::STANDARD_PRECISION
        || b.im.abs() >= geodesic_core::STANDARD_PRECISION
        || c.im.abs() >= geodesic_core::STANDARD_PRECISION
    {
        return false;
    }
    let normal_length = a.re.hypot(b.re);
    normal_length >= geodesic_core::STANDARD_PRECISION
        && (a.re * f64::from(cursor.x) + b.re * f64::from(cursor.y) + c.re).abs() / normal_length
            <= f64::from(LINE_SELECTION_RADIUS_PIXELS * camera_scale)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy's color alpha channel uses f32."
)]
fn canvas_color(style: &geodesic_io::ObjectStyle, selected: bool) -> Color {
    let color = if selected {
        Color::srgb(0.9, 0.2, 0.15)
    } else {
        bevy_color(&style.color)
    };
    color.with_alpha(style.opacity.clamp(0.0, 1.0) as f32)
}

pub(crate) fn canvas_contains(cursor: Vec2, window: &Window, state: &GuiState) -> bool {
    let bottom = window.height() - state.bottom_ui_height;
    cursor.x >= state.algebra_width
        && cursor.x <= window.width() - state.style_width
        && cursor.y >= TOOLBAR_HEIGHT
        && cursor.y <= bottom
}

pub(crate) fn screen_to_world(
    cursor: Vec2,
    window: &Window,
    translation: Vec3,
    scale: f32,
) -> Vec2 {
    Vec2::new(
        (cursor.x - window.width() / 2.0) * scale + translation.x,
        (window.height() / 2.0 - cursor.y) * scale + translation.y,
    )
}

pub(crate) fn next_zoom_scale(current: f32, factor: f32) -> Option<f32> {
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
pub(crate) fn draw_canvas(
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
                    .and_then(|preview| match preview {
                        DragPreview::Point {
                            label: preview_label,
                            point,
                        } if preview_label == label => match point.renderability() {
                            geodesic_core::Renderability::Renderable(point) => Some(point),
                            _ => None,
                        },
                        DragPreview::Point { .. } | DragPreview::Line { .. } => None,
                    })
                    .unwrap_or(point);
                let style = geodesic_io::effective_style(&state.project, label);
                let color = canvas_color(&style, state.selected.as_ref() == Some(label));
                gizmos.circle_2d(
                    Vec2::new(point.x as f32, point.y as f32),
                    point_radius_world(style.size, scale),
                    color,
                );
            }
            if let geodesic_core::GeometricObject::Line(line) = object {
                let line = state
                    .preview
                    .as_ref()
                    .and_then(|preview| match preview {
                        DragPreview::Line {
                            label: preview_label,
                            line,
                        } if preview_label == label => Some(*line),
                        DragPreview::Point { .. } | DragPreview::Line { .. } => None,
                    })
                    .unwrap_or(*line);
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
                    gizmos.line_2d(
                        start,
                        end,
                        canvas_color(&style, state.selected.as_ref() == Some(label)),
                    );
                }
            }
        }
    }
}

pub(crate) fn visible_world_bounds(
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
pub(crate) fn grid_step(scale: f32) -> f32 {
    nice_step((50.0 * scale).max(f32::MIN_POSITIVE))
}

pub(crate) fn nice_step(minimum: f32) -> f32 {
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

pub(crate) fn clipped_line(
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
