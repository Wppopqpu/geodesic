use crate::{Construction, GeoCommand, GeometricObject, Label, ProjectiveLine, ProjectivePoint};

fn label(value: &str) -> Label {
    Label::parse(value).expect("test labels are valid")
}

#[test]
fn evaluates_a_line_and_intersection_from_dependencies() {
    let mut construction = Construction::new();
    construction
        .apply(GeoCommand::AddPoint {
            label: label("point_a"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        })
        .expect("add point a");
    construction
        .apply(GeoCommand::AddPoint {
            label: label("point_b"),
            point: ProjectivePoint::real_affine(2.0, 2.0),
        })
        .expect("add point b");
    construction
        .apply(GeoCommand::AddLineThrough {
            label: label("diagonal"),
            first: label("point_a"),
            second: label("point_b"),
        })
        .expect("add diagonal");
    construction
        .apply(GeoCommand::AddLine {
            label: label("horizontal"),
            line: ProjectiveLine::new(0.0.into(), 1.0.into(), (-1.0).into())
                .expect("valid horizontal line"),
        })
        .expect("add horizontal line");
    construction
        .apply(GeoCommand::AddIntersection {
            label: label("crossing"),
            first: label("diagonal"),
            second: label("horizontal"),
        })
        .expect("add intersection");
    let Some(GeometricObject::Point(crossing)) = construction
        .evaluate()
        .expect("construction is valid")
        .object(&label("crossing"))
    else {
        panic!("intersection should evaluate to a point");
    };
    assert_eq!(
        crossing.renderability(),
        crate::Renderability::Renderable(crate::RealAffinePoint { x: 1.0, y: 1.0 })
    );
}

#[test]
fn temporarily_degenerate_objects_are_undefined_and_recover() {
    let mut construction = Construction::new();
    for (name, x, y) in [("point_a", 0.0, 0.0), ("point_b", 1.0, 0.0)] {
        construction
            .apply(GeoCommand::AddPoint {
                label: label(name),
                point: ProjectivePoint::real_affine(x, y),
            })
            .expect("point");
    }
    construction
        .apply(GeoCommand::AddLineThrough {
            label: label("moving_line"),
            first: label("point_a"),
            second: label("point_b"),
        })
        .expect("line");
    construction
        .apply(GeoCommand::AddLine {
            label: label("vertical"),
            line: ProjectiveLine::new(1.0.into(), 0.0.into(), (-0.5).into()).expect("line"),
        })
        .expect("line");
    construction
        .apply(GeoCommand::AddIntersection {
            label: label("crossing"),
            first: label("moving_line"),
            second: label("vertical"),
        })
        .expect("intersection");

    construction
        .apply(GeoCommand::MovePoint {
            label: label("point_b"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        })
        .expect("move");
    let snapshot = construction
        .evaluate()
        .expect("structurally valid snapshot");
    assert!(snapshot.object(&label("moving_line")).is_none());
    assert!(snapshot.object(&label("crossing")).is_none());
    assert_eq!(snapshot.undefined().count(), 2);

    construction
        .apply(GeoCommand::MovePoint {
            label: label("point_b"),
            point: ProjectivePoint::real_affine(1.0, 0.0),
        })
        .expect("move");
    let snapshot = construction.evaluate().expect("recovered snapshot");
    assert!(matches!(
        snapshot.object(&label("moving_line")),
        Some(GeometricObject::Line(_))
    ));
    assert!(matches!(
        snapshot.object(&label("crossing")),
        Some(GeometricObject::Point(_))
    ));
    assert_eq!(snapshot.undefined().count(), 0);
}

#[test]
fn rejects_duplicate_public_labels() {
    let mut construction = Construction::new();
    let command = GeoCommand::AddPoint {
        label: label("point_a"),
        point: ProjectivePoint::real_affine(0.0, 0.0),
    };
    construction
        .apply(command.clone())
        .expect("first label is free");
    assert!(matches!(
        construction.apply(command),
        Err(crate::CommandError::DuplicateLabel { .. })
    ));
}

#[test]
fn restores_commands_through_undo_and_redo() {
    let mut construction = Construction::new();
    construction
        .apply(GeoCommand::AddPoint {
            label: label("point_a"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        })
        .expect("command");
    assert!(construction.can_undo());
    construction.undo().expect("undo");
    assert_eq!(
        construction
            .evaluate()
            .expect("empty construction")
            .iter()
            .count(),
        0
    );
    assert!(construction.can_redo());
    construction.redo().expect("redo");
    assert_eq!(
        construction
            .evaluate()
            .expect("restored point")
            .iter()
            .count(),
        1
    );
}

#[test]
fn moves_a_free_point_and_records_the_change() {
    let mut construction = Construction::new();
    construction
        .apply(GeoCommand::AddPoint {
            label: label("point_a"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        })
        .expect("create");
    construction
        .apply(GeoCommand::MovePoint {
            label: label("point_a"),
            point: ProjectivePoint::real_affine(2.0, 3.0),
        })
        .expect("move");
    let point = point_at(&construction, "point_a");
    assert_eq!(
        point.renderability(),
        crate::Renderability::Renderable(crate::RealAffinePoint { x: 2.0, y: 3.0 })
    );
    construction.undo().expect("undo move");
    assert_eq!(
        point_at(&construction, "point_a").renderability(),
        crate::Renderability::Renderable(crate::RealAffinePoint { x: 0.0, y: 0.0 })
    );
}

#[test]
fn moves_a_free_line_and_records_the_change() {
    let mut construction = Construction::new();
    construction
        .apply(GeoCommand::AddLine {
            label: label("vertical"),
            line: ProjectiveLine::new(1.0.into(), 0.0.into(), (-1.0).into())
                .expect("valid vertical line"),
        })
        .expect("create");
    construction
        .apply(GeoCommand::MoveLine {
            label: label("vertical"),
            line: ProjectiveLine::new(1.0.into(), 0.0.into(), (-3.0).into())
                .expect("valid translated line"),
        })
        .expect("move");

    assert!(
        line_at(&construction, "vertical").is_incident_with(ProjectivePoint::real_affine(3.0, 0.0))
    );
    construction.undo().expect("undo move");
    assert!(
        line_at(&construction, "vertical").is_incident_with(ProjectivePoint::real_affine(1.0, 0.0))
    );
}

#[test]
fn evaluates_bounded_objects_and_affine_constructions() {
    let mut construction = Construction::new();
    for (name, x, y) in [
        ("point_a", 0.0, 0.0),
        ("point_b", 4.0, 0.0),
        ("point_c", 2.0, 2.0),
    ] {
        construction
            .apply(GeoCommand::AddPoint {
                label: label(name),
                point: ProjectivePoint::real_affine(x, y),
            })
            .expect("point");
    }
    for command in [
        GeoCommand::AddSegment {
            label: label("segment_ab"),
            start: label("point_a"),
            end: label("point_b"),
        },
        GeoCommand::AddRay {
            label: label("ray_ab"),
            origin: label("point_a"),
            through: label("point_b"),
        },
        GeoCommand::AddVector {
            label: label("vector_ac"),
            start: label("point_a"),
            end: label("point_c"),
        },
        GeoCommand::AddMidpoint {
            label: label("mid_ab"),
            first: label("point_a"),
            second: label("point_b"),
        },
        GeoCommand::AddPerpendicularBisector {
            label: label("perp_ab"),
            first: label("point_a"),
            second: label("point_b"),
        },
        GeoCommand::AddAngleBisector {
            label: label("angle"),
            first: label("point_a"),
            vertex: label("point_c"),
            second: label("point_b"),
        },
    ] {
        construction.apply(command).expect("construction");
    }
    let snapshot = construction.evaluate().expect("evaluation");
    assert!(
        matches!(snapshot.object(&label("segment_ab")), Some(GeometricObject::Segment(segment)) if segment.start == crate::RealAffinePoint { x: 0.0, y: 0.0 } && segment.end == crate::RealAffinePoint { x: 4.0, y: 0.0 })
    );
    assert!(matches!(
        snapshot.object(&label("ray_ab")),
        Some(GeometricObject::Ray(_))
    ));
    assert!(matches!(
        snapshot.object(&label("vector_ac")),
        Some(GeometricObject::Vector(_))
    ));
    assert_eq!(
        point_at(&construction, "mid_ab").renderability(),
        crate::Renderability::Renderable(crate::RealAffinePoint { x: 2.0, y: 0.0 })
    );
    assert!(
        line_at(&construction, "perp_ab").is_incident_with(ProjectivePoint::real_affine(2.0, 10.0))
    );
    assert!(
        line_at(&construction, "angle").is_incident_with(ProjectivePoint::real_affine(2.0, 1.0))
    );
    construction.undo().expect("undo");
    assert!(
        construction
            .evaluate()
            .expect("evaluation")
            .object(&label("angle"))
            .is_none()
    );
}

#[test]
fn rejects_non_real_points_for_bounded_constructions() {
    let mut construction = Construction::new();
    construction
        .apply(GeoCommand::AddPoint {
            label: label("complex_point"),
            point: ProjectivePoint::new(
                num_complex::Complex64::new(1.0, 1.0),
                0.0.into(),
                1.0.into(),
            )
            .expect("point"),
        })
        .expect("point");
    construction
        .apply(GeoCommand::AddPoint {
            label: label("real_point"),
            point: ProjectivePoint::real_affine(0.0, 0.0),
        })
        .expect("point");
    assert!(matches!(
        construction.apply(GeoCommand::AddSegment {
            label: label("invalid_segment"),
            start: label("complex_point"),
            end: label("real_point"),
        }),
        Err(crate::CommandError::NotRealAffinePoint { .. })
    ));
}

fn point_at(construction: &Construction, name: &str) -> ProjectivePoint {
    let Some(GeometricObject::Point(point)) = construction
        .evaluate()
        .expect("evaluation")
        .object(&label(name))
    else {
        panic!("point");
    };
    point
}

fn line_at(construction: &Construction, name: &str) -> ProjectiveLine {
    let Some(GeometricObject::Line(line)) = construction
        .evaluate()
        .expect("evaluation")
        .object(&label(name))
    else {
        panic!("line");
    };
    line
}
