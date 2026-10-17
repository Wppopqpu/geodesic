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
