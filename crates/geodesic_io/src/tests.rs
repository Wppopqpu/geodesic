use std::io::{Cursor, Write as _};

use geodesic_core::Label;

use crate::{effective_style, import_ggb, load, save};

#[test]
fn round_trips_without_executing_scripts() {
    let source = "(geodesic 1 (point point_a 0 0) (point point_b 1 1) (line_through diagonal point_a point_b) (script \"on_open\" \"(geo::point late 4 2)\"))";
    let project = load(source).expect("valid data document");
    assert_eq!(project.scripts.len(), 1);
    assert_eq!(
        project
            .construction
            .evaluate()
            .expect("script was not executed")
            .iter()
            .count(),
        3
    );
    assert_eq!(
        load(&save(&project)).expect("saved project is valid"),
        project
    );
}

#[test]
fn preserves_non_real_projective_coordinates() {
    let project = load("(geodesic 1 (point_complex complex_point 1 2 3 4 1 0))")
        .expect("valid complex point");
    let saved = save(&project);
    assert!(saved.contains("point_complex complex_point 1 2 3 4 1 0"));
    assert_eq!(load(&saved).expect("complex project round trips"), project);
}

#[test]
fn round_trips_bounded_objects_and_affine_constructions() {
    let source = "(geodesic 1 (point point_a 0 0) (point point_b 4 0) (point point_c 2 2) (segment segment_ab point_a point_b) (ray ray_ab point_a point_b) (vector vector_ac point_a point_c) (midpoint midpoint_ab point_a point_b) (perpendicular_bisector perp_ab point_a point_b) (angle_bisector angle_acb point_a point_c point_b))";
    let project = load(source).expect("project");
    assert_eq!(
        project
            .construction
            .evaluate()
            .expect("evaluation")
            .iter()
            .count(),
        9
    );
    let saved = save(&project);
    let restored = load(&saved).expect("round trip");
    assert_eq!(save(&restored), saved, "serialization stays deterministic");
    assert_eq!(
        restored.construction.evaluate().expect("evaluation"),
        project.construction.evaluate().expect("evaluation")
    );
}

#[test]
fn imports_supported_objects_and_reports_unsupported_ones() {
    let xml = r#"<geogebra><construction><element type="point" label="point_a"><coords x="1" y="2" z="1"/></element><element type="circle" label="circle_a" definition="Circle"/><script/></construction></geogebra>"#;
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut bytes);
        archive
            .start_file("geogebra.xml", zip::write::SimpleFileOptions::default())
            .expect("entry");
        archive.write_all(xml.as_bytes()).expect("xml");
        archive.finish().expect("archive");
    }
    let (project, report) = import_ggb(bytes.get_ref()).expect("import");
    assert_eq!(
        project
            .construction
            .evaluate()
            .expect("valid")
            .iter()
            .count(),
        1
    );
    assert_eq!(report.converted, 1);
    assert_eq!(report.placeholders[0].source_type, "circle");
    assert!(report.ignored_ggb_script);
}

#[test]
fn imports_bounded_constructions_from_geogebra_commands() {
    let xml = r#"<geogebra><construction><element type="point" label="A"><coords x="0" y="0" z="1"/></element><element type="point" label="B"><coords x="2" y="0" z="1"/></element><element type="segment" label="s"/><command name="Segment"><input a0="A" a1="B"/><output a0="s"/></command></construction></geogebra>"#;
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut archive = zip::ZipWriter::new(&mut bytes);
        archive
            .start_file("geogebra.xml", zip::write::SimpleFileOptions::default())
            .expect("entry");
        archive.write_all(xml.as_bytes()).expect("xml");
        archive.finish().expect("archive");
    }
    let (project, report) = import_ggb(bytes.get_ref()).expect("import");
    assert_eq!(report.converted, 3);
    assert!(report.placeholders.is_empty());
    assert!(matches!(
        project
            .construction
            .evaluate()
            .expect("evaluation")
            .object(&Label::parse("s").expect("label")),
        Some(geodesic_core::GeometricObject::Segment(_))
    ));
}

#[test]
fn persists_validated_direct_and_named_styles() {
    let project = load("(geodesic 1 (point point_a 0 0) (style point_a \"#ff0000\" 0.3 0.5) (style_preset emphasis \"#00ff00\" 0.2 1))").expect("project");
    assert_eq!(
        project.object_styles[&Label::parse("point_a").expect("label")].color,
        "#ff0000"
    );
    assert_eq!(load(&save(&project)).expect("round trip"), project);
}

#[test]
fn resolves_presets_and_prefers_direct_overrides() {
    let project = load("(geodesic 1 (point point_a 0 0) (style_preset emphasis \"#00ff00\" 0.2 1) (style_use point_a emphasis) (style point_a \"#ff0000\" 0.3 0.5))").expect("project");
    assert_eq!(
        effective_style(&project, &Label::parse("point_a").expect("label")).color,
        "#ff0000"
    );
    assert_eq!(load(&save(&project)).expect("round trip"), project);
}
