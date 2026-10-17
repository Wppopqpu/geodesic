use bevy::window::{PrimaryWindow, WindowTheme};
use bevy::{
    ecs::error::Result,
    prelude::{Camera2d, Handle, Image, Projection, Query, Res, ResMut, Transform, Window, With},
};
use bevy_egui::{EguiContexts, EguiTextureHandle, egui};
use geodesic_core::Label;

use crate::{
    command::{
        command_completions, command_hint, history_next, history_previous, replace_completion,
    },
    document::{
        DEFAULT_PROJECT_FILE_NAME, export_svg, load_project, native_project_path, save_project,
    },
    presentation::{
        apply_theme, draw_axis_labels, egui_color, hex_color, submit_command, wrapped_text_height,
    },
    state::{
        AboutDialog, ActiveTool, COMMAND_BAR_HEIGHT, COMPLETION_ROW_HEIGHT, GuiState,
        MESSAGE_BAR_VERTICAL_PADDING, TOOLBAR_HEIGHT, ToolbarIcons, UiTheme,
    },
};

const TOOL_STACKS: [&[ActiveTool]; 4] = [
    &[ActiveTool::Move],
    &[
        ActiveTool::Point,
        ActiveTool::PointOnLocus,
        ActiveTool::Intersection,
        ActiveTool::Midpoint,
    ],
    &[
        ActiveTool::Line,
        ActiveTool::Segment,
        ActiveTool::Ray,
        ActiveTool::Vector,
    ],
    &[
        ActiveTool::Parallel,
        ActiveTool::Perpendicular,
        ActiveTool::PerpendicularBisector,
        ActiveTool::AngleBisector,
    ],
];
const TOOLBAR_SCROLL_STEP: f32 = 180.0;
const TOOLBAR_SCROLL_BUTTON_WIDTH: f32 = 28.0;

fn toolbar_scroll_limit(content_width: f32, viewport_width: f32) -> f32 {
    (content_width - viewport_width).max(0.0)
}

fn shifted_toolbar_offset(offset: f32, delta: f32, limit: f32) -> f32 {
    (offset + delta).clamp(0.0, limit)
}

struct ToolbarIconIds {
    move_tool: egui::TextureId,
    point: egui::TextureId,
    segment: egui::TextureId,
    ray: egui::TextureId,
    vector: egui::TextureId,
    midpoint: egui::TextureId,
    perpendicular_bisector: egui::TextureId,
    angle_bisector: egui::TextureId,
    line: egui::TextureId,
    intersection: egui::TextureId,
    parallel: egui::TextureId,
    perpendicular: egui::TextureId,
    point_on_locus: egui::TextureId,
}

impl ToolbarIconIds {
    fn from_assets(contexts: &mut EguiContexts, icons: &ToolbarIcons) -> Self {
        Self {
            move_tool: add_toolbar_image(contexts, &icons.move_tool),
            point: add_toolbar_image(contexts, &icons.point),
            segment: add_toolbar_image(contexts, &icons.segment),
            ray: add_toolbar_image(contexts, &icons.ray),
            vector: add_toolbar_image(contexts, &icons.vector),
            midpoint: add_toolbar_image(contexts, &icons.midpoint),
            perpendicular_bisector: add_toolbar_image(contexts, &icons.perpendicular_bisector),
            angle_bisector: add_toolbar_image(contexts, &icons.angle_bisector),
            line: add_toolbar_image(contexts, &icons.line),
            intersection: add_toolbar_image(contexts, &icons.intersection),
            parallel: add_toolbar_image(contexts, &icons.parallel),
            perpendicular: add_toolbar_image(contexts, &icons.perpendicular),
            point_on_locus: add_toolbar_image(contexts, &icons.point_on_locus),
        }
    }

    const fn for_tool(&self, tool: ActiveTool) -> egui::TextureId {
        match tool {
            ActiveTool::Move => self.move_tool,
            ActiveTool::Point => self.point,
            ActiveTool::PointOnLocus => self.point_on_locus,
            ActiveTool::Line => self.line,
            ActiveTool::Intersection => self.intersection,
            ActiveTool::Segment => self.segment,
            ActiveTool::Ray => self.ray,
            ActiveTool::Vector => self.vector,
            ActiveTool::Parallel => self.parallel,
            ActiveTool::Perpendicular => self.perpendicular,
            ActiveTool::Midpoint => self.midpoint,
            ActiveTool::PerpendicularBisector => self.perpendicular_bisector,
            ActiveTool::AngleBisector => self.angle_bisector,
        }
    }
}

fn add_toolbar_image(contexts: &mut EguiContexts, image: &Handle<Image>) -> egui::TextureId {
    contexts.add_image(EguiTextureHandle::Weak(image.id()))
}

fn toolbar_icon(texture_id: egui::TextureId) -> egui::Image<'static> {
    egui::Image::new(egui::load::SizedTexture::new(texture_id, [32.0, 32.0]))
        .fit_to_exact_size(egui::Vec2::splat(22.0))
}

fn select_tool(state: &mut GuiState, tool: ActiveTool) {
    state.active_tool = tool;
    state.tool_inputs.clear();
    state.message = match tool.input_count() {
        0 if tool == ActiveTool::Point => "click the canvas to create a point".to_owned(),
        0 => "select and drag free objects".to_owned(),
        count => format!("select {count} points for {}", tool.name().to_lowercase()),
    };
}

fn tool_stack(
    ui: &mut egui::Ui,
    state: &mut GuiState,
    icons: &ToolbarIconIds,
    tools: &[ActiveTool],
) {
    let active = tools
        .iter()
        .copied()
        .find(|tool| *tool == state.active_tool)
        .unwrap_or(tools[0]);
    let button = egui::Button::image(toolbar_icon(icons.for_tool(active)))
        .selected(state.active_tool == active);
    if ui.add(button).on_hover_text(active.name()).clicked() {
        select_tool(state, active);
    }
    if tools.len() > 1 {
        ui.menu_button("▾", |ui| {
            for &tool in tools {
                let button =
                    egui::Button::image_and_text(toolbar_icon(icons.for_tool(tool)), tool.name())
                        .selected(state.active_tool == tool);
                if ui.add(button).on_hover_text(tool.name()).clicked() {
                    select_tool(state, tool);
                    ui.close();
                }
            }
        });
    }
}

fn toolbar_contents(
    ui: &mut egui::Ui,
    state: &mut GuiState,
    icons: &ToolbarIconIds,
    context: &egui::Context,
    system_theme: Option<WindowTheme>,
) {
    file_menu(ui, state);
    ui.separator();
    for tools in TOOL_STACKS {
        tool_stack(ui, state, icons, tools);
        ui.separator();
    }
    ui.label("Theme:");
    for (theme, name) in [
        (UiTheme::System, "System"),
        (UiTheme::Light, "Light"),
        (UiTheme::Dark, "Dark"),
    ] {
        if ui.selectable_label(state.theme == theme, name).clicked() {
            state.theme = theme;
            apply_theme(context, theme, system_theme);
        }
    }
    ui.separator();
    if ui
        .button("ⓘ")
        .on_hover_text("About and asset attribution")
        .clicked()
    {
        state.about_dialog = Some(AboutDialog);
    }
}

fn file_menu(ui: &mut egui::Ui, state: &mut GuiState) {
    ui.menu_button("File", |ui| {
        if ui.button("Open…").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Geodesic project", &["lisp"])
                .add_filter("GeoGebra archive", &["ggb"])
                .pick_file()
            {
                match load_project(&path) {
                    Ok((project, message)) => {
                        let imported = path.extension().is_some_and(|extension| extension == "ggb");
                        state.replace_project(project);
                        state.document_path = (!imported).then_some(path);
                        state.message = message;
                    }
                    Err(error) => state.message = format!("error: {error}"),
                }
            }
            ui.close();
        }
        if ui
            .add_enabled(state.project_is_editable(), egui::Button::new("Save"))
            .clicked()
        {
            save_from_menu(state, false);
            ui.close();
        }
        if ui
            .add_enabled(state.project_is_editable(), egui::Button::new("Save as…"))
            .clicked()
        {
            save_from_menu(state, true);
            ui.close();
        }
        ui.separator();
        if ui
            .add_enabled(
                state.project_is_editable(),
                egui::Button::new("Export SVG…"),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Scalable Vector Graphics", &["svg"])
                .set_file_name("untitled.svg")
                .save_file()
            {
                state.message = export_svg(&path, &state.project).map_or_else(
                    |error| format!("error: {error}"),
                    |()| format!("exported {}", path.display()),
                );
            }
            ui.close();
        }
    });
}

fn save_from_menu(state: &mut GuiState, force_choose_path: bool) {
    let path = if force_choose_path {
        None
    } else {
        state.document_path.clone()
    }
    .or_else(|| {
        rfd::FileDialog::new()
            .add_filter("Geodesic project", &["lisp"])
            .set_file_name(DEFAULT_PROJECT_FILE_NAME)
            .save_file()
            .map(|path| native_project_path(&path))
    });
    let Some(path) = path else {
        return;
    };
    state.message = save_project(&path, &state.project).map_or_else(
        |error| format!("error: {error}"),
        |()| {
            state.document_path = Some(path.clone());
            format!("saved {}", path.display())
        },
    );
}

fn show_attribution_window(context: &egui::Context, about_dialog: &mut Option<AboutDialog>) {
    if about_dialog.is_none() {
        return;
    }
    let mut open = true;
    egui::Window::new("About Geodesic")
        .open(&mut open)
        .resizable(false)
        .show(context, |ui| {
            ui.heading("Geodesic");
            ui.label("Real affine canvas · complex projective kernel");
            ui.separator();
            ui.strong("Made with GeoGebra®");
            ui.label("This non-commercial application incorporates GeoGebra toolbar icons.");
            ui.label("© International GeoGebra Institute · CC BY-NC-SA 4.0 or later.");
            ui.hyperlink_to("GeoGebra website", "https://www.geogebra.org/");
            ui.hyperlink_to("GeoGebra license", "https://www.geogebra.org/license");
            ui.small("GeoGebra does not endorse Geodesic.");
            ui.separator();
            ui.small("Full asset provenance and checksums: assets/ASSET_MANIFEST.md");
        });
    if !open {
        *about_dialog = None;
    }
}

fn renderability_hint<T: Copy>(
    classification: geodesic_core::Renderability<T>,
) -> Option<&'static str> {
    use geodesic_core::Renderability;
    match classification {
        Renderability::Renderable(_) => None,
        Renderability::NonReal => Some("Hidden: no supported real affine representation"),
        Renderability::AtInfinity => Some("Hidden: at infinity"),
        Renderability::NumericallyUncertain => {
            Some("Hidden: numerically uncertain real affine projection")
        }
    }
}

#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::needless_pass_by_value,
    reason = "The application chrome keeps related egui panels in one system."
)]
pub(crate) fn application_chrome(
    mut contexts: EguiContexts,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Transform, &Projection), With<Camera2d>>,
    toolbar_icons: Res<ToolbarIcons>,
    mut state: ResMut<GuiState>,
) -> Result {
    let toolbar_icon_ids = ToolbarIconIds::from_assets(&mut contexts, toolbar_icons.as_ref());
    let context = contexts.ctx_mut()?;
    let system_theme = windows.single().ok().and_then(|window| window.window_theme);
    apply_theme(context, state.theme, system_theme);
    let mut viewport_ui = egui::Ui::new(
        context.clone(),
        "viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(context.viewport_rect()),
    );
    if !state.command_bar_visible
        && !context.egui_wants_keyboard_input()
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
    state.reconcile_lisp_draft(context.input(|input| input.time));
    egui::Panel::top("toolbar")
        .exact_size(TOOLBAR_HEIGHT)
        .show(&mut viewport_ui, |ui| {
            ui.horizontal(|ui| {
                if state.toolbar_overflows
                    && ui
                        .add_enabled(state.toolbar_scroll_offset > 0.0, egui::Button::new("◀"))
                        .clicked()
                {
                    state.toolbar_scroll_offset = shifted_toolbar_offset(
                        state.toolbar_scroll_offset,
                        -TOOLBAR_SCROLL_STEP,
                        f32::INFINITY,
                    );
                }

                let right_button_width = if state.toolbar_overflows {
                    TOOLBAR_SCROLL_BUTTON_WIDTH
                } else {
                    0.0
                };
                let scroll_width = (ui.available_width() - right_button_width).max(0.0);
                let output = egui::ScrollArea::horizontal()
                    .id_salt("toolbar_scroll")
                    .auto_shrink([false, false])
                    .vscroll(false)
                    .max_width(scroll_width)
                    .max_height(TOOLBAR_HEIGHT)
                    .horizontal_scroll_offset(state.toolbar_scroll_offset)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .scroll_source(egui::scroll_area::ScrollSource {
                        mouse_wheel: false,
                        ..Default::default()
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            toolbar_contents(
                                ui,
                                state.as_mut(),
                                &toolbar_icon_ids,
                                context,
                                system_theme,
                            );
                        });
                    });
                let scroll_limit =
                    toolbar_scroll_limit(output.content_size.x, output.inner_rect.width());
                state.toolbar_overflows = scroll_limit > 0.0;
                state.toolbar_scroll_offset = state.toolbar_scroll_offset.min(scroll_limit);

                if state.toolbar_overflows
                    && ui
                        .add_enabled(
                            state.toolbar_scroll_offset < scroll_limit,
                            egui::Button::new("▶"),
                        )
                        .clicked()
                {
                    state.toolbar_scroll_offset = shifted_toolbar_offset(
                        state.toolbar_scroll_offset,
                        TOOLBAR_SCROLL_STEP,
                        scroll_limit,
                    );
                }

                if context
                    .pointer_latest_pos()
                    .is_some_and(|position| output.inner_rect.contains(position))
                {
                    let wheel_delta = context.input(|input| input.smooth_scroll_delta);
                    if wheel_delta != egui::Vec2::ZERO && scroll_limit > 0.0 {
                        state.toolbar_scroll_offset = shifted_toolbar_offset(
                            state.toolbar_scroll_offset,
                            -(wheel_delta.x + wheel_delta.y),
                            scroll_limit,
                        );
                        context.input_mut(|input| input.smooth_scroll_delta = egui::Vec2::ZERO);
                    }
                }
            });
        });
    show_attribution_window(context, &mut state.about_dialog);
    egui::Panel::left("algebra")
        .default_size(state.algebra_width)
        .min_size(160.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            state.algebra_width = ui.max_rect().width();
            ui.heading("Algebra");
            ui.separator();
            let content_height = ui.available_height();
            egui::ScrollArea::vertical()
                .id_salt("algebra_scroll")
                .auto_shrink([false, false])
                .hscroll(false)
                .max_height(content_height)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.label("Project data (Lisp)");
                    let response = ui.add(
                        egui::TextEdit::multiline(&mut state.lisp_draft.source)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(12)
                            .desired_width(f32::INFINITY),
                    );
                    if response.changed() {
                        state.edit_lisp_draft(ui.input(|input| input.time));
                    }
                    match &state.lisp_draft.state {
                        crate::state::LispDraftState::Clean => {
                            ui.small("Validated and synchronized with the canvas.");
                        }
                        crate::state::LispDraftState::Pending { .. } => {
                            ui.small("Validating Lisp draft…");
                        }
                        crate::state::LispDraftState::Invalid(error) => {
                            ui.colored_label(egui::Color32::RED, error);
                            if ui.button("Discard draft changes").clicked() {
                                state.discard_lisp_draft();
                            }
                        }
                    }
                    ui.separator();
                    ui.heading("Algebra");
                    match state.project.construction.evaluate() {
                        Ok(snapshot) => {
                            for (label, object) in snapshot.iter() {
                                ui.label(format!("{label}: {object:?}"));
                                let hint = match object {
                                    geodesic_core::GeometricObject::Point(point) => {
                                        renderability_hint(point.renderability())
                                    }
                                    geodesic_core::GeometricObject::Line(line) => {
                                        renderability_hint(line.renderability())
                                    }
                                    geodesic_core::GeometricObject::Segment(_)
                                    | geodesic_core::GeometricObject::Ray(_)
                                    | geodesic_core::GeometricObject::Vector(_) => None,
                                };
                                if let Some(hint) = hint {
                                    ui.small(hint);
                                }
                            }
                            for (label, error) in snapshot.undefined() {
                                ui.label(format!("{label}: undefined"));
                                ui.colored_label(
                                    egui::Color32::from_rgb(190, 55, 55),
                                    error.to_string(),
                                );
                            }
                        }
                        Err(error) => {
                            ui.colored_label(egui::Color32::RED, error.to_string());
                        }
                    }
                });
        });
    egui::Panel::right("style_editor")
        .default_size(state.style_width)
        .min_size(160.0)
        .resizable(true)
        .show(&mut viewport_ui, |ui| {
            state.style_width = ui.max_rect().width();
            ui.heading("Style");
            ui.separator();
            let content_height = ui.available_height();
            egui::ScrollArea::vertical()
                .id_salt("style_scroll")
                .auto_shrink([false, false])
                .hscroll(false)
                .max_height(content_height)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    if !state.project_is_editable() {
                        ui.colored_label(
                            egui::Color32::RED,
                            "Style changes are disabled while the Lisp draft is unvalidated.",
                        );
                        if ui.button("Discard draft changes").clicked() {
                            state.discard_lisp_draft();
                        }
                    } else if let Some(label) = state.selected.clone() {
                        ui.label(label.to_string());
                        let presets: Vec<_> = state.project.style_presets.keys().cloned().collect();
                        if ui.button("Create emphasis preset").clicked()
                            && let Ok(preset) = Label::parse("emphasis")
                        {
                            state.project.style_presets.entry(preset).or_insert(
                                geodesic_io::ObjectStyle {
                                    color: "#d32f2f".to_owned(),
                                    size: 0.25,
                                    opacity: 1.0,
                                },
                            );
                            state.sync_lisp_draft();
                        }
                        for preset in presets {
                            if ui.button(format!("Use preset {preset}")).clicked() {
                                state.project.object_presets.insert(label.clone(), preset);
                                state.project.object_styles.remove(&label);
                                state.sync_lisp_draft();
                            }
                        }
                        if state.project.object_presets.contains_key(&label) {
                            let preset = &state.project.object_presets[&label];
                            ui.label(format!("Using preset {preset}"));
                            if ui.button("Use direct override").clicked() {
                                let style = geodesic_io::effective_style(&state.project, &label);
                                state.project.object_presets.remove(&label);
                                state.project.object_styles.insert(label.clone(), style);
                                state.sync_lisp_draft();
                            }
                        } else {
                            let is_point =
                                state.project.construction.definitions().any(
                                    |(candidate, definition)| {
                                        candidate == &label
                                            && matches!(
                                        definition,
                                        geodesic_core::ObjectDefinition::FreePoint(_)
                                            | geodesic_core::ObjectDefinition::Intersection { .. }
                                    )
                                    },
                                );
                            let mut style = geodesic_io::effective_style(&state.project, &label);
                            if is_point && !state.project.object_styles.contains_key(&label) {
                                style.size = f64::from(crate::canvas::point_radius_pixels(
                                    &state.project,
                                    &label,
                                )) * 0.03;
                            }
                            let mut color = egui_color(&style.color);
                            let mut changed = ui.color_edit_button_srgba(&mut color).changed();
                            if changed {
                                style.color = hex_color(color);
                            }
                            let mut size = style.size / 0.03;
                            let (maximum, caption) = if is_point {
                                (9.0, "Radius (px)")
                            } else {
                                (13.0, "Thickness (½ px)")
                            };
                            if ui
                                .add(
                                    egui::Slider::new(&mut size, 1.0..=maximum)
                                        .step_by(1.0)
                                        .text(caption),
                                )
                                .changed()
                            {
                                style.size = size * 0.03;
                                changed = true;
                            }
                            changed |= ui
                                .add(
                                    egui::Slider::new(&mut style.opacity, 0.0..=1.0)
                                        .text("Opacity"),
                                )
                                .changed();
                            if changed {
                                state.project.object_styles.insert(label, style);
                                state.sync_lisp_draft();
                            }
                        }
                    } else {
                        ui.label("Select a point or line to edit its style.");
                    }
                });
        });
    if state.command_bar_visible {
        let candidates = command_completions(&state.project.construction, &state.command);
        if state.completion_query != state.command {
            let command = state.command.clone();
            state.completion_query.clone_from(&command);
            state.completion_index = 0;
        }
        if !candidates.is_empty() {
            state.completion_index %= candidates.len();
        }
        let hint = command_hint(&state.command);
        let hint_height = wrapped_text_height(
            context,
            hint,
            context.viewport_rect().width(),
            &egui::TextStyle::Small,
        );
        let command_height =
            COMMAND_BAR_HEIGHT + COMPLETION_ROW_HEIGHT * candidates.len() as f32 + hint_height;
        state.bottom_ui_height = command_height;
        egui::Panel::bottom("command_bar")
            .exact_size(command_height)
            .show(&mut viewport_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(":");
                    ui.vertical(|ui| {
                        let mut text_edit = egui::TextEdit::singleline(&mut state.command)
                            .lock_focus(true)
                            .show(ui);
                        let response = &text_edit.response;
                        let mut completion_cursor = None;
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
                            state.command = replace_completion(
                                &state.command,
                                &candidates[state.completion_index],
                            );
                            completion_cursor = Some(state.command.chars().count());
                        } else if input.2 {
                            submit_command(state.as_mut());
                        } else if input.3 {
                            state.command_bar_visible = false;
                        }

                        if !candidates.is_empty() {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_min_width(response.rect.width());
                                for (index, candidate) in candidates.iter().enumerate() {
                                    let button = egui::Button::new(candidate)
                                        .selected(index == state.completion_index)
                                        .sense(egui::Sense::CLICK);
                                    if ui.add(button).clicked() {
                                        state.command =
                                            replace_completion(&state.command, candidate);
                                        completion_cursor = Some(state.command.chars().count());
                                    }
                                }
                            });
                        }
                        if command_bar_tab_pressed {
                            response.request_focus();
                        }
                        if let Some(cursor) = completion_cursor {
                            text_edit.state.cursor.set_char_range(Some(
                                egui::text::CCursorRange::one(egui::text::CCursor::new(cursor)),
                            ));
                            text_edit.state.store(ui.ctx(), response.id);
                        }
                    });
                });
                ui.small(hint);
            });
    } else {
        state.bottom_ui_height = 0.0;
    }
    if !state.message.is_empty() {
        let message_height = wrapped_text_height(
            context,
            &state.message,
            context.viewport_rect().width(),
            &egui::TextStyle::Body,
        ) + MESSAGE_BAR_VERTICAL_PADDING;
        state.bottom_ui_height += message_height;
        egui::Panel::bottom("message_bar")
            .exact_size(message_height)
            .show(&mut viewport_ui, |ui| {
                let text = if state.message.starts_with("error:") {
                    egui::RichText::new(&state.message).color(egui::Color32::from_rgb(190, 55, 55))
                } else {
                    egui::RichText::new(&state.message)
                };
                ui.label(text);
            });
    }
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(&mut viewport_ui, |ui| {
            draw_axis_labels(ui.painter(), ui.max_rect(), &windows, &camera);
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TOOL_STACKS, shifted_toolbar_offset, toolbar_scroll_limit};
    use crate::state::ActiveTool;

    #[test]
    fn tool_stacks_cover_each_available_tool_once() {
        let mut tools: Vec<_> = TOOL_STACKS.into_iter().flatten().copied().collect();
        tools.sort_by_key(|tool| *tool as u8);
        assert_eq!(tools, ActiveTool::ALL);
    }

    #[test]
    fn toolbar_scroll_stays_inside_the_overflow_range() {
        assert_eq!(toolbar_scroll_limit(200.0, 240.0), 0.0);
        let limit = toolbar_scroll_limit(640.0, 240.0);
        assert_eq!(limit, 400.0);
        assert_eq!(shifted_toolbar_offset(0.0, -180.0, limit), 0.0);
        assert_eq!(shifted_toolbar_offset(320.0, 180.0, limit), limit);
        assert_eq!(shifted_toolbar_offset(100.0, 180.0, limit), 280.0);
    }
}
