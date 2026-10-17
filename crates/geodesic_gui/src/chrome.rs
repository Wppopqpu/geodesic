use bevy::prelude::{Camera2d, Projection, Query, ResMut, Transform, Window, With};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, egui};
use geodesic_core::Label;

use crate::{
    command::{
        command_completions, command_hint, history_next, history_previous, replace_completion,
    },
    presentation::{
        apply_theme, draw_axis_labels, egui_color, hex_color, submit_command, wrapped_text_height,
    },
    state::{
        ActiveTool, COMMAND_BAR_HEIGHT, COMPLETION_ROW_HEIGHT, GuiState,
        MESSAGE_BAR_VERTICAL_PADDING, TOOLBAR_HEIGHT, UiTheme,
    },
};

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
    reason = "The application chrome keeps related egui panels in one system."
)]
pub(crate) fn application_chrome(
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
    egui::TopBottomPanel::top("toolbar")
        .exact_height(TOOLBAR_HEIGHT)
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Geodesic");
                ui.label("Real affine canvas · complex projective kernel");
                ui.separator();
                for tool in ActiveTool::ALL {
                    if ui
                        .selectable_label(state.active_tool == tool, tool.name())
                        .clicked()
                    {
                        state.active_tool = tool;
                        state.tool_inputs.clear();
                        state.message = match tool.input_count() {
                            0 if tool == ActiveTool::Point => {
                                "click the canvas to create a point".to_owned()
                            }
                            0 => "select and drag free objects".to_owned(),
                            count => {
                                format!("select {count} points for {}", tool.name().to_lowercase())
                            }
                        };
                    }
                }
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
            state.algebra_width = ui.max_rect().width();
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
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::RED, error.to_string());
                }
            }
            ui.allocate_space(ui.available_size());
        });
    egui::SidePanel::right("style_editor")
        .default_width(state.style_width)
        .min_width(160.0)
        .resizable(true)
        .show(context, |ui| {
            state.style_width = ui.max_rect().width();
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
                        let style = geodesic_io::effective_style(&state.project, &label);
                        state.project.object_presets.remove(&label);
                        state.project.object_styles.insert(label.clone(), style);
                    }
                } else {
                    let is_point =
                        state
                            .project
                            .construction
                            .definitions()
                            .any(|(candidate, definition)| {
                                candidate == &label
                                    && matches!(
                                        definition,
                                        geodesic_core::ObjectDefinition::FreePoint(_)
                                            | geodesic_core::ObjectDefinition::Intersection { .. }
                                    )
                            });
                    let mut style = geodesic_io::effective_style(&state.project, &label);
                    if is_point && !state.project.object_styles.contains_key(&label) {
                        style.size =
                            f64::from(crate::canvas::point_radius_pixels(&state.project, &label))
                                * 0.03;
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
                        .add(egui::Slider::new(&mut style.opacity, 0.0..=1.0).text("Opacity"))
                        .changed();
                    if changed {
                        state.project.object_styles.insert(label, style);
                    }
                }
            } else {
                ui.label("Select a point or line to edit its style.");
            }
            ui.allocate_space(ui.available_size());
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
            context.available_rect().width(),
            &egui::TextStyle::Small,
        );
        let command_height =
            COMMAND_BAR_HEIGHT + COMPLETION_ROW_HEIGHT * candidates.len() as f32 + hint_height;
        state.bottom_ui_height = command_height;
        egui::TopBottomPanel::bottom("command_bar")
            .exact_height(command_height)
            .show(context, |ui| {
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
                            submit_command(&mut state);
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
            context.available_rect().width(),
            &egui::TextStyle::Body,
        ) + MESSAGE_BAR_VERTICAL_PADDING;
        state.bottom_ui_height += message_height;
        egui::TopBottomPanel::bottom("message_bar")
            .exact_height(message_height)
            .show(context, |ui| {
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
        .show(context, |ui| {
            draw_axis_labels(ui.painter(), ui.max_rect(), &windows, &camera);
        });
}
