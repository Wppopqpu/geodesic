use geodesic_core::{Construction, Label};

use crate::{ScriptError, ScriptLimits, dispatch, dispatch_event};

#[test]
fn keeps_prior_commands_when_later_command_fails() {
    let mut construction = Construction::new();
    assert!(
        dispatch(
            "(geo::point point_a 0 0) (unsupported)",
            &mut construction,
            ScriptLimits::default()
        )
        .is_err()
    );
    assert_eq!(
        construction
            .evaluate()
            .expect("first command persists")
            .iter()
            .count(),
        1
    );
}

#[test]
fn dispatches_only_matching_project_event_scripts() {
    let project = geodesic_io::load("(geodesic 1 (script \"on_open\" \"(geo::point point_a 0 0)\") (script \"on_save\" \"(geo::point point_b 1 1)\"))").expect("project");
    let mut construction = Construction::new();
    assert_eq!(
        dispatch_event(
            &project,
            "on_open",
            &mut construction,
            ScriptLimits::default()
        )
        .expect("dispatch"),
        1
    );
    assert_eq!(
        construction.evaluate().expect("evaluation").iter().count(),
        1
    );
}

#[test]
fn rejects_source_over_the_explicit_limit() {
    let mut construction = Construction::new();
    let limits = ScriptLimits {
        max_commands: 1,
        max_source_bytes: 3,
        ..ScriptLimits::default()
    };
    assert!(matches!(
        dispatch("(geo::point point_a 0 0)", &mut construction, limits),
        Err(ScriptError::SourceLimit)
    ));
}

#[test]
fn translates_point_line_primitives_into_one_construction() {
    let mut construction = Construction::new();
    let source = "(geo::point point_a 0 0) (geo::point point_b 1 1) (geo::line_through diagonal point_a point_b)";
    assert_eq!(
        dispatch(source, &mut construction, ScriptLimits::default()).expect("dispatch"),
        3
    );
    assert_eq!(
        construction.evaluate().expect("evaluation").iter().count(),
        3
    );
}

#[test]
fn rejects_unbalanced_and_deep_script_sources() {
    let mut construction = Construction::new();
    assert!(matches!(
        dispatch(
            "(geo::point point_a 0 0",
            &mut construction,
            ScriptLimits::default()
        ),
        Err(ScriptError::UnbalancedSource)
    ));
    let limits = ScriptLimits {
        max_nesting_depth: 1,
        ..ScriptLimits::default()
    };
    assert!(matches!(
        dispatch("((geo::point point_a 0 0))", &mut construction, limits),
        Err(ScriptError::NestingLimit)
    ));
}

#[test]
fn retains_complex_points_outside_the_canvas_chart() {
    let mut construction = Construction::new();
    dispatch(
        "(geo::point_complex complex_point 1 1 0 0 1 0)",
        &mut construction,
        ScriptLimits::default(),
    )
    .expect("dispatch");
    let point = construction
        .evaluate()
        .expect("evaluation")
        .object(&Label::parse("complex_point").expect("label"))
        .expect("point");
    assert!(
        matches!(point, geodesic_core::GeometricObject::Point(point) if matches!(point.renderability(), geodesic_core::Renderability::NonReal))
    );
}
