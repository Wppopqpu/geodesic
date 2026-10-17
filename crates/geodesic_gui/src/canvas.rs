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

use crate::path_renderer::{CanvasBatch, CanvasMesh, PathPainter};
use crate::presentation::bevy_color;
use crate::state::{ActiveTool, DragPreview, DragTarget, GuiState, TOOLBAR_HEIGHT};

/// The initial camera scale maps 0.15 persisted style units to five pixels.
///
/// Point styles remain persisted in project/SVG units, while the canvas adapts
/// their world-space radius to keep their on-screen radius constant as the
/// camera zooms.
const STYLE_UNITS_PER_PIXEL: f32 = 0.03;
const DEFAULT_FREE_POINT_RADIUS_PIXELS: f32 = 5.0;
const DEFAULT_DEPENDENT_POINT_RADIUS_PIXELS: f32 = 4.0;
const MIN_POINT_RADIUS_PIXELS: f32 = 1.0;
const MAX_POINT_RADIUS_PIXELS: f32 = 9.0;
const POINT_SELECTION_RADIUS_PIXELS: f32 = 15.0;
const LINE_SELECTION_RADIUS_PIXELS: f32 = 3.0;
/// Keep panel resize strips and panel borders owned by the chrome, not the canvas.
const CANVAS_CHROME_GUTTER: f32 = 4.0;

#[allow(
    clippy::needless_pass_by_value,
    clippy::cast_possible_truncation,
    reason = "Bevy system resources are passed through ECS wrappers."
)]
pub(crate) fn canvas_input(
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut mouse_motion: MessageReader<MouseMotion>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    mut state: ResMut<GuiState>,
) {
    // Always drain raw messages. Otherwise a wheel or motion event received over
    // a sidebar can be applied later when the cursor returns to the canvas.
    let wheel_events: Vec<_> = mouse_wheel.read().copied().collect();
    let motion_events: Vec<_> = mouse_motion.read().copied().collect();
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let cursor_is_on_canvas = canvas_contains(cursor, window, state.as_ref());
    let canvas_gesture_active =
        state.dragging.is_some() || state.pan_cursor.is_some() || state.middle_pan_cursor.is_some();
    if !cursor_is_on_canvas && !canvas_gesture_active {
        state.dragging = None;
        state.preview = None;
        state.pan_cursor = None;
        state.middle_pan_cursor = None;
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
        for event in wheel_events {
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
    if mouse_buttons.just_pressed(MouseButton::Middle) && cursor_is_on_canvas {
        state.middle_pan_cursor = Some(cursor);
    }
    if mouse_buttons.pressed(MouseButton::Middle) && state.middle_pan_cursor.is_some() {
        for event in motion_events {
            transform.translation.x -= event.delta.x * orthographic.scale;
            transform.translation.y += event.delta.y * orthographic.scale;
        }
    }
    if mouse_buttons.just_released(MouseButton::Middle) {
        state.middle_pan_cursor = None;
    }

    let world = screen_to_world(cursor, window, transform.translation, orthographic.scale);
    if cursor_is_on_canvas && mouse_buttons.just_pressed(MouseButton::Left) {
        if state.active_tool != ActiveTool::Move {
            if !state.project_is_editable() {
                "resolve or discard the Lisp draft before creating objects"
                    .clone_into(&mut state.message);
                return;
            }
            apply_tool_click(state.as_mut(), world, orthographic.scale);
            return;
        }
        state.selected = select_canvas_object(&state.project, world, orthographic.scale);
        state.dragging = state
            .project_is_editable()
            .then(|| {
                state
                    .selected
                    .as_ref()
                    .and_then(|label| drag_target(&state.project.construction, label))
            })
            .flatten();
        state.pan_cursor = (state.selected.is_none()).then_some(cursor);
    }
    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(target) = &state.dragging {
            state.preview = drag_preview(&state.project.construction, target, world);
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
            if !state.message.starts_with("error:") {
                state.sync_lisp_draft();
            }
        }
        state.preview = None;
        state.pan_cursor = None;
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Tool input selection and one-command construction remain together."
)]
fn apply_tool_click(state: &mut GuiState, world: Vec2, camera_scale: f32) {
    if state.active_tool == ActiveTool::Point {
        let Some(label) = next_tool_label(&state.project.construction, ActiveTool::Point) else {
            "error: unable to create an object label".clone_into(&mut state.message);
            return;
        };
        match state.project.construction.apply(GeoCommand::AddPoint {
            label: label.clone(),
            point: ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
        }) {
            Ok(()) => {
                state.selected = Some(label.clone());
                state.message = format!("created point {label}");
                state.sync_lisp_draft();
            }
            Err(error) => state.message = format!("error: {error}"),
        }
        return;
    }
    let expects_supporting_locus = matches!(
        state.active_tool,
        ActiveTool::Intersection
            | ActiveTool::PointOnLocus
            | ActiveTool::Parallel
            | ActiveTool::Perpendicular
    ) && (state.active_tool == ActiveTool::Intersection
        || !state.tool_inputs.is_empty());
    let input = if expects_supporting_locus {
        select_canvas_supporting_locus(&state.project, world, camera_scale)
    } else {
        select_canvas_point(&state.project, world, camera_scale)
    };
    let Some(input) = input else {
        if expects_supporting_locus {
            "select a line, segment, or ray".clone_into(&mut state.message);
        } else {
            "select a real affine point".clone_into(&mut state.message);
        }
        return;
    };
    if state.tool_inputs.last() != Some(&input) {
        state.tool_inputs.push(input);
    }
    let expected = state.active_tool.input_count();
    if state.tool_inputs.len() < expected {
        state.message = format!(
            "select {} more point(s)",
            expected - state.tool_inputs.len()
        );
        return;
    }
    let inputs = std::mem::take(&mut state.tool_inputs);
    let Some(label) = next_tool_label(&state.project.construction, state.active_tool) else {
        "error: unable to create an object label".clone_into(&mut state.message);
        return;
    };
    let command = match state.active_tool {
        ActiveTool::Line => GeoCommand::AddLineThrough {
            label: label.clone(),
            first: inputs[0].clone(),
            second: inputs[1].clone(),
        },
        ActiveTool::Intersection => GeoCommand::AddIntersection {
            label: label.clone(),
            first: inputs[0].clone(),
            second: inputs[1].clone(),
        },
        ActiveTool::PointOnLocus => GeoCommand::AddPointOnLocus {
            label: label.clone(),
            locus: inputs[0].clone(),
            parameter: locus_parameter(&state.project.construction, &inputs[0], world)
                .unwrap_or(0.0),
        },
        ActiveTool::Segment => GeoCommand::AddSegment {
            label: label.clone(),
            start: inputs[0].clone(),
            end: inputs[1].clone(),
        },
        ActiveTool::Ray => GeoCommand::AddRay {
            label: label.clone(),
            origin: inputs[0].clone(),
            through: inputs[1].clone(),
        },
        ActiveTool::Vector => GeoCommand::AddVector {
            label: label.clone(),
            start: inputs[0].clone(),
            end: inputs[1].clone(),
        },
        ActiveTool::Parallel => GeoCommand::AddParallelThrough {
            label: label.clone(),
            point: inputs[0].clone(),
            line: inputs[1].clone(),
        },
        ActiveTool::Perpendicular => GeoCommand::AddPerpendicularThrough {
            label: label.clone(),
            point: inputs[0].clone(),
            line: inputs[1].clone(),
        },
        ActiveTool::Midpoint => GeoCommand::AddMidpoint {
            label: label.clone(),
            first: inputs[0].clone(),
            second: inputs[1].clone(),
        },
        ActiveTool::PerpendicularBisector => GeoCommand::AddPerpendicularBisector {
            label: label.clone(),
            first: inputs[0].clone(),
            second: inputs[1].clone(),
        },
        ActiveTool::AngleBisector => GeoCommand::AddAngleBisector {
            label: label.clone(),
            first: inputs[0].clone(),
            vertex: inputs[1].clone(),
            second: inputs[2].clone(),
        },
        ActiveTool::Move | ActiveTool::Point => return,
    };
    match state.project.construction.apply(command) {
        Ok(()) => {
            state.selected = Some(label.clone());
            state.message = format!(
                "created {} {label}",
                state.active_tool.name().to_lowercase()
            );
            state.sync_lisp_draft();
        }
        Err(error) => state.message = format!("error: {error}"),
    }
}

fn select_canvas_supporting_locus(
    project: &geodesic_io::Project,
    world: Vec2,
    camera_scale: f32,
) -> Option<geodesic_core::Label> {
    let label = select_canvas_object(project, world, camera_scale)?;
    matches!(
        project.construction.evaluate().ok()?.object(&label),
        Some(GeometricObject::Line(_) | GeometricObject::Segment(_) | GeometricObject::Ray(_))
    )
    .then_some(label)
}

fn locus_parameter(
    construction: &geodesic_core::Construction,
    label: &geodesic_core::Label,
    world: Vec2,
) -> Option<f64> {
    match construction.evaluate().ok()?.object(label)? {
        GeometricObject::Line(line) => {
            let geodesic_core::Renderability::Renderable(line) = line.renderability() else {
                return None;
            };
            Some(
                -line.b * (f64::from(world.x) + line.a * line.c)
                    + line.a * (f64::from(world.y) + line.b * line.c),
            )
        }
        GeometricObject::Segment(segment) => {
            Some(affine_locus_parameter(segment.start, segment.end, world).clamp(0.0, 1.0))
        }
        GeometricObject::Ray(ray) => {
            Some(affine_locus_parameter(ray.origin, ray.through, world).max(0.0))
        }
        GeometricObject::Point(_) | GeometricObject::Vector(_) => None,
    }
}

fn affine_locus_parameter(
    start: geodesic_core::RealAffinePoint,
    end: geodesic_core::RealAffinePoint,
    world: Vec2,
) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    ((f64::from(world.x) - start.x) * dx + (f64::from(world.y) - start.y) * dy)
        / (dx * dx + dy * dy)
}

fn next_tool_label(
    construction: &geodesic_core::Construction,
    tool: ActiveTool,
) -> Option<geodesic_core::Label> {
    (1_u32..).find_map(|index| {
        let label = geodesic_core::Label::parse(format!("{}_{index}", tool.label_prefix())).ok()?;
        construction
            .definitions()
            .all(|(existing, _)| existing != &label)
            .then_some(label)
    })
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
                    if point_is_hit_radius(
                        Vec2::new(point.x as f32, point.y as f32),
                        world,
                        point_radius_pixels(project, label),
                        camera_scale,
                    ) =>
                {
                    Some(label.clone())
                }
                _ => None,
            },
            GeometricObject::Line(_)
            | GeometricObject::Segment(_)
            | GeometricObject::Ray(_)
            | GeometricObject::Vector(_) => None,
        })
        .or_else(|| {
            snapshot.iter().find_map(|(label, object)| match object {
                GeometricObject::Line(line) if line_is_hit(*line, world, camera_scale) => {
                    Some(label.clone())
                }
                GeometricObject::Segment(segment)
                    if bounded_is_hit(segment.start, segment.end, world, camera_scale) =>
                {
                    Some(label.clone())
                }
                GeometricObject::Vector(vector)
                    if bounded_is_hit(vector.start, vector.end, world, camera_scale) =>
                {
                    Some(label.clone())
                }
                GeometricObject::Ray(ray) if ray_is_hit(ray, world, camera_scale) => {
                    Some(label.clone())
                }
                GeometricObject::Point(_)
                | GeometricObject::Line(_)
                | GeometricObject::Segment(_)
                | GeometricObject::Ray(_)
                | GeometricObject::Vector(_) => None,
            })
        })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Canvas hit testing is performed in Bevy's f32 world coordinates."
)]
fn select_canvas_point(
    project: &geodesic_io::Project,
    world: Vec2,
    camera_scale: f32,
) -> Option<geodesic_core::Label> {
    let snapshot = project.construction.evaluate().ok()?;
    snapshot.iter().find_map(|(label, object)| match object {
        GeometricObject::Point(point) => match point.renderability() {
            geodesic_core::Renderability::Renderable(point)
                if point_is_hit_radius(
                    Vec2::new(point.x as f32, point.y as f32),
                    world,
                    point_radius_pixels(project, label),
                    camera_scale,
                ) =>
            {
                Some(label.clone())
            }
            _ => None,
        },
        GeometricObject::Line(_)
        | GeometricObject::Segment(_)
        | GeometricObject::Ray(_)
        | GeometricObject::Vector(_) => None,
    })
}

fn drag_preview(
    construction: &geodesic_core::Construction,
    target: &DragTarget,
    world: Vec2,
) -> Option<DragPreview> {
    match target {
        DragTarget::Point(label) => Some(DragPreview::Point {
            label: label.clone(),
            point: ProjectivePoint::real_affine(f64::from(world.x), f64::from(world.y)),
        }),
        DragTarget::ConstrainedPoint { label, locus } => {
            let parameter = locus_parameter(construction, locus, world)?;
            let mut preview = construction.clone();
            preview
                .apply(GeoCommand::MovePointOnLocus {
                    label: label.clone(),
                    parameter,
                })
                .ok()?;
            let GeometricObject::Point(_) = preview.evaluate().ok()?.object(label)? else {
                return None;
            };
            Some(DragPreview::ConstrainedPoint {
                label: label.clone(),
                parameter,
            })
        }
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
        DragTarget::ConstrainedPoint { label, locus } => (
            GeoCommand::MovePointOnLocus {
                label,
                parameter: locus_parameter(construction, &locus, world)
                    .ok_or_else(|| "constrained locus is unavailable".to_owned())?,
            },
            "moved constrained point",
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
            ObjectDefinition::PointOnLocus { locus, .. } => Some(DragTarget::ConstrainedPoint {
                label: label.clone(),
                locus: locus.clone(),
            }),
            ObjectDefinition::FreeLine(line) => Some(DragTarget::Line {
                label: label.clone(),
                line: *line,
            }),
            ObjectDefinition::LineThrough { .. }
            | ObjectDefinition::Intersection { .. }
            | ObjectDefinition::ParallelThrough { .. }
            | ObjectDefinition::PerpendicularThrough { .. }
            | ObjectDefinition::Segment { .. }
            | ObjectDefinition::Ray { .. }
            | ObjectDefinition::Vector { .. }
            | ObjectDefinition::Midpoint { .. }
            | ObjectDefinition::PerpendicularBisector { .. }
            | ObjectDefinition::AngleBisector { .. }
            | ObjectDefinition::LineAngleBisector { .. } => None,
        })
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy's 2D renderer uses f32 coordinates."
)]
#[cfg(test)]
pub(crate) fn point_radius_world(style_size: f64, camera_scale: f32) -> f32 {
    style_size_pixels(style_size) * camera_scale
}

#[cfg(test)]
pub(crate) fn point_selection_radius_world(style_size: f64, camera_scale: f32) -> f32 {
    point_selection_radius_pixels(style_size_pixels(style_size)) * camera_scale
}

#[cfg(test)]
pub(crate) fn point_is_hit(point: Vec2, cursor: Vec2, style_size: f64, camera_scale: f32) -> bool {
    point_is_hit_radius(point, cursor, style_size_pixels(style_size), camera_scale)
}

fn point_is_hit_radius(point: Vec2, cursor: Vec2, radius_pixels: f32, camera_scale: f32) -> bool {
    point.distance(cursor) <= point_selection_radius_pixels(radius_pixels) * camera_scale
}

fn point_selection_radius_pixels(radius_pixels: f32) -> f32 {
    radius_pixels.max(POINT_SELECTION_RADIUS_PIXELS)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Canvas rendering uses f32 screen-pixel dimensions."
)]
pub(crate) fn style_size_pixels(style_size: f64) -> f32 {
    (style_size as f32 / STYLE_UNITS_PER_PIXEL)
        .clamp(MIN_POINT_RADIUS_PIXELS, MAX_POINT_RADIUS_PIXELS)
}

fn line_width_pixels(style_size: f64) -> f32 {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Canvas rendering uses f32 screen-pixel dimensions."
    )]
    let pixels = style_size as f32 / STYLE_UNITS_PER_PIXEL;
    // GeoGebra thickness units are twice the actual screen stroke width.
    pixels.max(0.0) / 2.0
}

pub(crate) fn point_radius_pixels(
    project: &geodesic_io::Project,
    label: &geodesic_core::Label,
) -> f32 {
    if project.object_styles.contains_key(label) {
        return style_size_pixels(geodesic_io::effective_style(project, label).size);
    }
    project
        .construction
        .definitions()
        .find_map(|(candidate, definition)| (candidate == label).then_some(definition))
        .map_or(
            DEFAULT_FREE_POINT_RADIUS_PIXELS,
            |definition| match definition {
                ObjectDefinition::FreePoint(_) => default_point_radius_pixels(true),
                ObjectDefinition::FreeLine(_)
                | ObjectDefinition::LineThrough { .. }
                | ObjectDefinition::Intersection { .. }
                | ObjectDefinition::ParallelThrough { .. }
                | ObjectDefinition::PerpendicularThrough { .. }
                | ObjectDefinition::Segment { .. }
                | ObjectDefinition::Ray { .. }
                | ObjectDefinition::Vector { .. }
                | ObjectDefinition::Midpoint { .. }
                | ObjectDefinition::PerpendicularBisector { .. }
                | ObjectDefinition::AngleBisector { .. }
                | ObjectDefinition::LineAngleBisector { .. }
                | ObjectDefinition::PointOnLocus { .. } => default_point_radius_pixels(false),
            },
        )
}

pub(crate) fn default_point_radius_pixels(is_free: bool) -> f32 {
    if is_free {
        DEFAULT_FREE_POINT_RADIUS_PIXELS
    } else {
        DEFAULT_DEPENDENT_POINT_RADIUS_PIXELS
    }
}

pub(crate) fn line_is_hit(line: ProjectiveLine, cursor: Vec2, camera_scale: f32) -> bool {
    let geodesic_core::Renderability::Renderable(line) = line.renderability() else {
        return false;
    };
    (line.a * f64::from(cursor.x) + line.b * f64::from(cursor.y) + line.c).abs()
        <= f64::from(LINE_SELECTION_RADIUS_PIXELS * camera_scale)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Canvas hit testing is performed in Bevy's f32 world coordinates."
)]
fn bounded_is_hit(
    start: geodesic_core::RealAffinePoint,
    end: geodesic_core::RealAffinePoint,
    cursor: Vec2,
    camera_scale: f32,
) -> bool {
    let start = Vec2::new(start.x as f32, start.y as f32);
    let end = Vec2::new(end.x as f32, end.y as f32);
    let direction = end - start;
    let parameter = (cursor - start).dot(direction) / direction.length_squared();
    let nearest = start + direction * parameter.clamp(0.0, 1.0);
    cursor.distance(nearest) <= LINE_SELECTION_RADIUS_PIXELS * camera_scale
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Canvas hit testing is performed in Bevy's f32 world coordinates."
)]
fn ray_is_hit(ray: &geodesic_core::AffineRay, cursor: Vec2, camera_scale: f32) -> bool {
    let origin = Vec2::new(ray.origin.x as f32, ray.origin.y as f32);
    let through = Vec2::new(ray.through.x as f32, ray.through.y as f32);
    let direction = through - origin;
    let parameter = (cursor - origin).dot(direction) / direction.length_squared();
    let nearest = origin + direction * parameter.max(0.0);
    cursor.distance(nearest) <= LINE_SELECTION_RADIUS_PIXELS * camera_scale
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Bevy's color alpha channel uses f32."
)]
fn canvas_color(style: &geodesic_io::ObjectStyle) -> Color {
    bevy_color(&style.color).with_alpha(style.opacity.clamp(0.0, 1.0) as f32)
}

fn selected_canvas_color(style: &geodesic_io::ObjectStyle) -> Color {
    let color = canvas_color(style);
    color.with_alpha(color.alpha() * 0.2)
}

pub(crate) fn canvas_contains(cursor: Vec2, window: &Window, state: &GuiState) -> bool {
    let bottom = window.height() - state.bottom_ui_height;
    cursor.x > state.algebra_width + CANVAS_CHROME_GUTTER
        && cursor.x < window.width() - state.style_width - CANVAS_CHROME_GUTTER
        && cursor.y > TOOLBAR_HEIGHT + CANVAS_CHROME_GUTTER
        && cursor.y < bottom - CANVAS_CHROME_GUTTER
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
    reason = "Bevy system parameters are owned wrappers."
)]
pub(crate) fn draw_canvas(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    state: Res<GuiState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut batch: Local<CanvasBatch>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((transform, Projection::Orthographic(projection))) = camera.single() else {
        return;
    };
    batch.painter.clear();
    batch.painter.set_pixel_scale(window.scale_factor());
    paint_scene(
        &mut batch.painter,
        state.as_ref(),
        window.size(),
        transform.translation.truncate(),
        projection.scale,
    );
    let error = batch.painter.error().map(str::to_owned);
    if error != batch.last_error {
        if let Some(error) = &error {
            warn!("{error}");
        }
        batch.last_error = error;
    }
    let mesh = batch.painter.mesh();
    let mesh_transform = Transform::from_xyz(transform.translation.x, transform.translation.y, 0.0)
        .with_scale(Vec3::new(projection.scale, projection.scale, 1.0));
    if let Some(handle) = &batch.mesh {
        if let Some(mut asset) = meshes.get_mut(handle) {
            *asset = mesh;
        }
        if let Some(entity) = batch.entity {
            commands.entity(entity).insert(mesh_transform);
        }
    } else {
        let handle = meshes.add(mesh);
        let material = materials.add(ColorMaterial {
            color: Color::WHITE,
            alpha_mode: bevy::sprite_render::AlphaMode2d::Blend,
            ..default()
        });
        batch.entity = Some(
            commands
                .spawn((
                    CanvasMesh,
                    Mesh2d(handle.clone()),
                    MeshMaterial2d(material),
                    mesh_transform,
                ))
                .id(),
        );
        batch.mesh = Some(handle);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "Finite viewport-local pixel coordinates are converted to Bevy f32 meshes."
)]
fn paint_scene(painter: &mut PathPainter, state: &GuiState, size: Vec2, origin: Vec2, scale: f32) {
    if !scale.is_finite() || scale <= 0.0 || !origin.is_finite() || !size.is_finite() {
        return;
    }
    let half = size / 2.0;
    let step = f64::from(grid_step(scale));
    let spacing = step / f64::from(scale);
    for (vertical, extent, center) in [(true, half.x, origin.x), (false, half.y, origin.y)] {
        let start =
            ((f64::from(center) - f64::from(extent) * f64::from(scale)) / step).ceil() * step;
        let start = (start - f64::from(center)) / f64::from(scale);
        let count = (f64::from(extent) * 2.0 / spacing).ceil() as usize + 1;
        for index in 0..count {
            let coordinate = (start + index as f64 * spacing) as f32;
            let (a, b) = if vertical {
                (
                    Vec2::new(coordinate, -half.y),
                    Vec2::new(coordinate, half.y),
                )
            } else {
                (
                    Vec2::new(-half.x, coordinate),
                    Vec2::new(half.x, coordinate),
                )
            };
            painter.line(a, b, 1.0, Color::srgb(0.88, 0.88, 0.9));
        }
    }
    let axis = Color::srgb(0.35, 0.35, 0.4);
    let zero = -origin / scale;
    if zero.y.abs() <= half.y {
        painter.line(
            Vec2::new(-half.x, zero.y),
            Vec2::new(half.x, zero.y),
            1.0,
            axis,
        );
    }
    if zero.x.abs() <= half.x {
        painter.line(
            Vec2::new(zero.x, -half.y),
            Vec2::new(zero.x, half.y),
            1.0,
            axis,
        );
    }
    let Some(snapshot) = preview_snapshot(state) else {
        return;
    };
    // Painter order is explicit: grid, axes, lines, then point markers.
    for (label, object) in snapshot.iter() {
        if let GeometricObject::Line(line) = object {
            paint_line(painter, state, label, *line, half, origin, scale);
        }
        if let GeometricObject::Segment(segment) = object {
            paint_bounded_line(
                painter,
                state,
                label,
                segment.start,
                segment.end,
                origin,
                scale,
            );
        }
        if let GeometricObject::Ray(ray) = object {
            paint_ray(painter, state, label, *ray, half, origin, scale);
        }
        if let GeometricObject::Vector(vector) = object {
            paint_vector(painter, state, label, *vector, origin, scale);
        }
    }
    for (label, object) in snapshot.iter() {
        if let GeometricObject::Point(point) = object {
            paint_point(painter, state, label, *point, half, origin, scale);
        }
    }
}

pub(crate) fn preview_snapshot(state: &GuiState) -> Option<geodesic_core::GeometrySnapshot> {
    let mut construction = state.project.construction.clone();
    match state.preview.as_ref() {
        Some(DragPreview::Point { label, point }) => construction
            .apply(GeoCommand::MovePoint {
                label: label.clone(),
                point: *point,
            })
            .ok()?,
        Some(DragPreview::Line { label, line }) => construction
            .apply(GeoCommand::MoveLine {
                label: label.clone(),
                line: *line,
            })
            .ok()?,
        Some(DragPreview::ConstrainedPoint {
            label, parameter, ..
        }) => construction
            .apply(GeoCommand::MovePointOnLocus {
                label: label.clone(),
                parameter: *parameter,
            })
            .ok()?,
        None => {}
    }
    construction.evaluate().ok()
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Viewport-local mesh coordinates use f32."
)]
fn affine_to_screen(point: geodesic_core::RealAffinePoint, origin: Vec2, scale: f32) -> Vec2 {
    Vec2::new(
        ((point.x - f64::from(origin.x)) / f64::from(scale)) as f32,
        ((point.y - f64::from(origin.y)) / f64::from(scale)) as f32,
    )
}

fn paint_bounded_line(
    painter: &mut PathPainter,
    state: &GuiState,
    label: &geodesic_core::Label,
    start: geodesic_core::RealAffinePoint,
    end: geodesic_core::RealAffinePoint,
    origin: Vec2,
    scale: f32,
) {
    let start = affine_to_screen(start, origin, scale);
    let end = affine_to_screen(end, origin, scale);
    let style = geodesic_io::effective_style(&state.project, label);
    let width = line_width_pixels(style.size);
    if state.selected.as_ref() == Some(label) {
        painter.line(
            start,
            end,
            2.0 * width.max(1.0) + 2.0,
            selected_canvas_color(&style),
        );
    }
    painter.line(start, end, width, canvas_color(&style));
}

fn paint_ray(
    painter: &mut PathPainter,
    state: &GuiState,
    label: &geodesic_core::Label,
    ray: geodesic_core::AffineRay,
    half: Vec2,
    origin: Vec2,
    scale: f32,
) {
    let start = affine_to_screen(ray.origin, origin, scale);
    let through = affine_to_screen(ray.through, origin, scale);
    let Some(end) = clipped_ray(start, through, half) else {
        return;
    };
    paint_bounded_line(
        painter,
        state,
        label,
        ray.origin,
        geodesic_core::RealAffinePoint {
            x: f64::from(end.x.mul_add(scale, origin.x)),
            y: f64::from(end.y.mul_add(scale, origin.y)),
        },
        origin,
        scale,
    );
}

fn paint_vector(
    painter: &mut PathPainter,
    state: &GuiState,
    label: &geodesic_core::Label,
    vector: geodesic_core::AffineVector,
    origin: Vec2,
    scale: f32,
) {
    paint_bounded_line(
        painter,
        state,
        label,
        vector.start,
        vector.end,
        origin,
        scale,
    );
    let start = affine_to_screen(vector.start, origin, scale);
    let end = affine_to_screen(vector.end, origin, scale);
    let direction = end - start;
    let Some(unit) = direction.try_normalize() else {
        return;
    };
    let style = geodesic_io::effective_style(&state.project, label);
    let head = (12.0 + line_width_pixels(style.size) * 2.0).min(direction.length() / 2.0);
    let base = end - unit * head;
    let offset = unit.perp() * (head / 2.0);
    painter.triangle(end, base + offset, base - offset, canvas_color(&style));
}

fn clipped_ray(origin: Vec2, through: Vec2, half: Vec2) -> Option<Vec2> {
    let direction = through - origin;
    let mut parameters = Vec::with_capacity(4);
    if direction.x.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        parameters.extend([-half.x, half.x].into_iter().filter_map(|x| {
            let parameter = (x - origin.x) / direction.x;
            let y = origin.y + parameter * direction.y;
            (parameter >= 0.0 && (-half.y..=half.y).contains(&y)).then_some(parameter)
        }));
    }
    if direction.y.abs() >= geodesic_core::STANDARD_PRECISION_F32 {
        parameters.extend([-half.y, half.y].into_iter().filter_map(|y| {
            let parameter = (y - origin.y) / direction.y;
            let x = origin.x + parameter * direction.x;
            (parameter >= 0.0 && (-half.x..=half.x).contains(&x)).then_some(parameter)
        }));
    }
    parameters
        .into_iter()
        .reduce(f32::max)
        .map(|parameter| origin + direction * parameter)
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Convert only viewport-local positions to mesh precision."
)]
fn paint_point(
    painter: &mut PathPainter,
    state: &GuiState,
    label: &geodesic_core::Label,
    point: ProjectivePoint,
    half: Vec2,
    origin: Vec2,
    scale: f32,
) {
    let geodesic_core::Renderability::Renderable(point) = point.renderability() else {
        return;
    };
    let center = Vec2::new(
        ((point.x - f64::from(origin.x)) / f64::from(scale)) as f32,
        ((point.y - f64::from(origin.y)) / f64::from(scale)) as f32,
    );
    let radius = point_radius_pixels(&state.project, label);
    let highlight = radius.mul_add(2.0, 1.0);
    if !center.is_finite() || (center.abs() - half).max_element() > highlight + 1.0 {
        return;
    }
    let style = geodesic_io::effective_style(&state.project, label);
    let color = canvas_color(&style);
    if state.selected.as_ref() == Some(label) {
        painter.disk(center, highlight, selected_canvas_color(&style));
        painter.circle(center, highlight, 2.0, color);
    }
    painter.disk(center, radius, color);
    painter.circle(center, radius, 1.0, Color::BLACK.with_alpha(color.alpha()));
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "Transform in f64 before producing viewport-local f32 coefficients."
)]
fn paint_line(
    painter: &mut PathPainter,
    state: &GuiState,
    label: &geodesic_core::Label,
    line: ProjectiveLine,
    half: Vec2,
    origin: Vec2,
    scale: f32,
) {
    let geodesic_core::Renderability::Renderable(line) = line.renderability() else {
        return;
    };
    let constant =
        (line.a * f64::from(origin.x) + line.b * f64::from(origin.y) + line.c) / f64::from(scale);
    if !constant.is_finite() || constant.abs() > f64::from(half.length()) {
        return;
    }
    let Some((start, end)) = clipped_line(
        line.a as f32,
        line.b as f32,
        constant as f32,
        -half.x,
        half.x,
        -half.y,
        half.y,
    ) else {
        return;
    };
    let style = geodesic_io::effective_style(&state.project, label);
    let width = line_width_pixels(style.size);
    if state.selected.as_ref() == Some(label) {
        painter.line(
            start,
            end,
            2.0 * width.max(1.0) + 2.0,
            selected_canvas_color(&style),
        );
    }
    painter.line(start, end, width, canvas_color(&style));
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
