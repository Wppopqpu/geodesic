#[cfg(test)]
mod tests {
    use crate::{
        GuiState, UiTheme, command_completions, command_hint, execute_command, hex_color,
        history_next, history_previous, replace_completion, rgb,
    };
    use bevy_egui::egui;
    use geodesic_core::{Construction, GeoCommand, ProjectiveLine, ProjectivePoint};

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
    fn ex_move_translates_a_free_line_without_changing_its_direction() {
        let mut construction = Construction::new();
        construction
            .apply(GeoCommand::AddLine {
                label: geodesic_core::Label::parse("vertical").expect("label"),
                line: ProjectiveLine::new(1.0.into(), 0.0.into(), (-1.0).into()).expect("line"),
            })
            .expect("add line");

        execute_command(&mut construction, ":move vertical 3 2").expect("move line");
        let Some(geodesic_core::GeometricObject::Line(line)) = construction
            .evaluate()
            .expect("evaluation")
            .object(&geodesic_core::Label::parse("vertical").expect("label"))
        else {
            panic!("line");
        };
        assert!(line.is_incident_with(ProjectivePoint::real_affine(3.0, 2.0)));
        assert!(line.is_incident_with(ProjectivePoint::real_affine(3.0, -2.0)));
    }

    #[test]
    fn completes_command_names_from_the_current_token() {
        let construction = Construction::new();
        assert_eq!(
            command_completions(&construction, ":p"),
            vec!["point", "perpendicular_bisector"]
        );
        assert_eq!(command_completions(&construction, ":un"), vec!["undo"]);
        assert_eq!(
            command_completions(&construction, ":"),
            vec![
                "point",
                "line",
                "segment",
                "ray",
                "vector",
                "midpoint",
                "perpendicular_bisector",
                "angle_bisector",
                "move",
                "undo",
                "redo"
            ]
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
        for index in 0..(crate::MAX_COMPLETIONS + 2) {
            execute_command(&mut construction, &format!(":point point_{index:02} 0 0"))
                .expect("point");
        }

        let completions = command_completions(&construction, ":move point_");
        assert_eq!(completions.len(), crate::MAX_COMPLETIONS);
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
            "commands: :point | :line | :segment | :ray | :vector | :midpoint | :perpendicular_bisector | :angle_bisector | :move | :undo | :redo"
        );
        assert_eq!(
            command_hint(":line diagonal"),
            "usage: :line label point_a point_b"
        );
        assert_eq!(
            command_hint(":unknown"),
            "unknown command; use a geometry command or :move, :undo, :redo"
        );
    }

    #[test]
    fn ex_commands_create_bounded_objects_and_affine_constructions() {
        let mut construction = Construction::new();
        for command in [
            ":point point_a 0 0",
            ":point point_b 4 0",
            ":point point_c 2 2",
        ] {
            execute_command(&mut construction, command).expect("point");
        }
        for command in [
            ":segment segment_ab point_a point_b",
            ":ray ray_ab point_a point_b",
            ":vector vector_ac point_a point_c",
            ":midpoint midpoint_ab point_a point_b",
            ":perpendicular_bisector perp_ab point_a point_b",
            ":angle_bisector angle_acb point_a point_c point_b",
        ] {
            execute_command(&mut construction, command).expect("construction");
        }
        assert_eq!(
            construction.evaluate().expect("evaluation").iter().count(),
            9
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
        crate::submit_command(&mut state);
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
        assert_eq!(crate::next_zoom_scale(1.0, 0.0), None);
    }

    #[test]
    fn zoom_accepts_large_finite_scales_but_rejects_invalid_results() {
        assert_eq!(crate::next_zoom_scale(1_000.0, 2.0), Some(2_000.0));
        assert_eq!(crate::next_zoom_scale(1.0, 0.0), None);
        assert_eq!(crate::next_zoom_scale(f32::INFINITY, 2.0), None);
    }

    #[test]
    fn point_radius_is_constant_in_screen_pixels_across_zoom_levels() {
        let style_size = 0.15;
        let zoomed_out = crate::point_radius_world(style_size, 0.01);
        let zoomed_in = crate::point_radius_world(style_size, 0.04);

        assert!((zoomed_out / 0.01 - 5.0).abs() < 1e-5);
        assert!((zoomed_in / 0.04 - 5.0).abs() < 1e-5);
    }

    #[test]
    fn point_sizes_match_geogebra_defaults_and_editor_range() {
        for (actual, expected) in [
            (crate::default_point_radius_pixels(true), 5.0),
            (crate::default_point_radius_pixels(false), 4.0),
            (crate::style_size_pixels(0.03), 1.0),
            (crate::style_size_pixels(0.15), 5.0),
            (crate::style_size_pixels(0.27), 9.0),
            (crate::style_size_pixels(0.01), 1.0),
            (crate::style_size_pixels(1.0), 9.0),
        ] {
            assert!((actual - expected).abs() < 1e-5);
        }
    }

    #[test]
    fn point_hit_target_has_a_zoom_independent_minimum_radius() {
        let point = bevy::prelude::Vec2::ZERO;
        for scale in [0.01, 0.04] {
            assert!(crate::point_is_hit(
                point,
                bevy::prelude::Vec2::new(15.0 * scale, 0.0),
                0.15,
                scale,
            ));
            assert!(!crate::point_is_hit(
                point,
                bevy::prelude::Vec2::new(15.1 * scale, 0.0),
                0.15,
                scale,
            ));
            assert!((crate::point_selection_radius_world(0.15, scale) / scale - 15.0).abs() < 1e-5);
        }
    }

    #[test]
    fn line_hit_target_is_constant_in_screen_pixels() {
        let line = ProjectiveLine::new(0.0.into(), 1.0.into(), 0.0.into()).expect("line");
        for scale in [0.01, 0.04] {
            assert!(crate::line_is_hit(
                line,
                bevy::prelude::Vec2::new(0.0, 3.0 * scale),
                scale,
            ));
            assert!(!crate::line_is_hit(
                line,
                bevy::prelude::Vec2::new(0.0, 3.1 * scale),
                scale,
            ));
        }
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
            crate::clipped_line(0.0, 1.0, 0.0, -10.0, 10.0, -10.0, 10.0).expect("horizontal line");
        assert_eq!(start, bevy::prelude::Vec2::new(-10.0, 0.0));
        assert_eq!(end, bevy::prelude::Vec2::new(10.0, 0.0));
    }

    #[test]
    fn selects_125_grid_steps_from_camera_scale() {
        for (scale, expected) in [(0.02, 1.0), (0.2, 10.0), (0.002, 0.1)] {
            assert!((crate::grid_step(scale) - expected).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn formats_tick_values_for_the_visible_grid_scale() {
        assert_eq!(crate::format_coordinate(12.0, 1.0), "12");
        assert_eq!(crate::format_coordinate(0.5, 0.5), "0.5");
        assert_eq!(crate::format_coordinate(0.25, 0.05), "0.25");
        assert_eq!(crate::format_coordinate(-0.000_001, 0.1), "0");
    }

    #[test]
    fn uses_compact_scientific_notation_for_extreme_ticks() {
        assert_eq!(crate::format_coordinate(1_000_000.0, 1.0), "1e6");
        assert_eq!(crate::format_coordinate(2.5e7, 1.0), "2.5e7");
        assert_eq!(crate::format_coordinate(0.00001, 0.00001), "1e-5");
    }

    #[test]
    fn projects_horizontal_axis_using_camera_translation_direction() {
        assert!((crate::world_to_screen_y(0.0, 1.0, 100.0, 0.5) - 52.0).abs() < f32::EPSILON);
    }

    #[test]
    fn clamps_label_rect_inside_each_canvas_edge() {
        let bounds = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(90.0, 80.0));
        let size = egui::vec2(20.0, 10.0);
        for position in [
            egui::pos2(0.0, 30.0),
            egui::pos2(100.0, 30.0),
            egui::pos2(30.0, 0.0),
            egui::pos2(30.0, 100.0),
        ] {
            let clamped = crate::clamp_label_position(position, size, bounds);
            let label_rect = egui::Rect::from_min_size(clamped, size);
            assert!(bounds.contains(label_rect.min));
            assert!(bounds.contains(label_rect.max));
        }
    }

    #[test]
    fn canvas_input_excludes_sidebars_and_command_bar() {
        let state = GuiState {
            algebra_width: 220.0,
            style_width: 220.0,
            command_bar_visible: true,
            bottom_ui_height: 100.0,
            ..GuiState::default()
        };
        let window = bevy::window::Window::default();
        assert!(!crate::canvas_contains(
            bevy::prelude::Vec2::new(100.0, 200.0),
            &window,
            &state
        ));
        assert!(!crate::canvas_contains(
            bevy::prelude::Vec2::new(500.0, 20.0),
            &window,
            &state
        ));
        assert!(!crate::canvas_contains(
            bevy::prelude::Vec2::new(500.0, window.height() - 20.0),
            &window,
            &state
        ));
    }
}
