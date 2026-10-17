//! Deterministic SVG rendering from immutable geometry snapshots.
#![forbid(unsafe_code)]
use geodesic_core::{GeometricObject, GeometrySnapshot, Renderability, STANDARD_PRECISION};
use geodesic_io::{LinePattern, ObjectStyle, PointShape, Project};
use std::fmt::Write;
/// Renders real finite points and real affine lines; non-real objects are omitted.
#[must_use]
pub fn render(snapshot: &GeometrySnapshot) -> String {
    render_with_styles(snapshot, None)
}

/// Renders an evaluated project using its direct object-style overrides.
///
/// # Errors
///
/// Returns evaluation failures from the project's construction.
pub fn render_project(project: &Project) -> Result<String, geodesic_core::EvaluationError> {
    let snapshot = project.construction.evaluate()?;
    Ok(render_with_styles(&snapshot, Some(project)))
}

fn render_with_styles(snapshot: &GeometrySnapshot, project: Option<&Project>) -> String {
    let mut svg =
        String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"-10 -10 20 20\">");
    for (label, object) in snapshot.iter() {
        if let GeometricObject::Point(point) = object
            && let Renderability::Renderable(point) = point.renderability()
        {
            let style = project
                .map(|project| geodesic_io::effective_style(project, label))
                .unwrap_or_default();
            write_point(&mut svg, label, point.x, -point.y, &style);
        }
        if let GeometricObject::Line(line) = object
            && let Renderability::Renderable(line) = line.renderability()
            && let Some(((x1, y1), (x2, y2))) = clipped_line(line.a, line.b, line.c)
        {
            let style = project.map_or(
                ObjectStyle {
                    color: "#212121".to_owned(),
                    size: 0.05,
                    opacity: 1.0,
                    ..ObjectStyle::default()
                },
                |project| geodesic_io::effective_style(project, label),
            );
            let _ = write!(
                svg,
                "<line id=\"{label}\" x1=\"{x1}\" y1=\"{}\" x2=\"{x2}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\"{} />",
                -y1,
                -y2,
                style.color,
                style.size,
                style.opacity,
                dash_attributes(&style)
            );
        }
        if let GeometricObject::Segment(segment) = object {
            write_bounded_line(&mut svg, label, segment.start, segment.end, project);
        }
        if let GeometricObject::Ray(ray) = object
            && let Some(end) = clipped_ray_endpoint(ray.origin, ray.through)
        {
            write_bounded_line(&mut svg, label, ray.origin, end, project);
        }
        if let GeometricObject::Vector(vector) = object {
            write_bounded_line(&mut svg, label, vector.start, vector.end, project);
            write_vector_head(&mut svg, label, vector.start, vector.end, project);
        }
    }
    svg.push_str("</svg>");
    svg
}

fn style_for(project: Option<&Project>, label: &geodesic_core::Label) -> ObjectStyle {
    project.map_or(
        ObjectStyle {
            color: "#212121".to_owned(),
            size: 0.05,
            opacity: 1.0,
            ..ObjectStyle::default()
        },
        |project| geodesic_io::effective_style(project, label),
    )
}

fn write_bounded_line(
    svg: &mut String,
    label: &geodesic_core::Label,
    start: geodesic_core::RealAffinePoint,
    end: geodesic_core::RealAffinePoint,
    project: Option<&Project>,
) {
    let style = style_for(project, label);
    let _ = write!(
        svg,
        "<line id=\"{label}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\"{} />",
        start.x,
        -start.y,
        end.x,
        -end.y,
        style.color,
        style.size,
        style.opacity,
        dash_attributes(&style)
    );
}

fn dash_attributes(style: &ObjectStyle) -> &'static str {
    match style.line_pattern {
        LinePattern::Solid => "",
        LinePattern::Dashed => " stroke-dasharray=\"8 5\"",
        LinePattern::Dotted => " stroke-dasharray=\"1 4\"",
    }
}

fn write_point(
    svg: &mut String,
    label: &geodesic_core::Label,
    x: f64,
    y: f64,
    style: &ObjectStyle,
) {
    match style.point_shape {
        PointShape::Circle => {
            let _ = write!(
                svg,
                "<circle id=\"{label}\" cx=\"{x}\" cy=\"{y}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                style.size, style.color, style.opacity
            );
        }
        PointShape::Square => {
            let side = style.size * 2.0;
            let _ = write!(
                svg,
                "<rect id=\"{label}\" x=\"{}\" y=\"{}\" width=\"{side}\" height=\"{side}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                x - style.size,
                y - style.size,
                style.color,
                style.opacity
            );
        }
        PointShape::Diamond | PointShape::Triangle => {
            let points = if matches!(style.point_shape, PointShape::Diamond) {
                format!(
                    "{x},{} {},{y} {x},{} {},{y}",
                    y - style.size,
                    x + style.size,
                    y + style.size,
                    x - style.size
                )
            } else {
                format!(
                    "{x},{} {},{} {},{}",
                    y - style.size,
                    x + style.size,
                    y + style.size,
                    x - style.size,
                    y + style.size
                )
            };
            let _ = write!(
                svg,
                "<polygon id=\"{label}\" points=\"{points}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                style.color, style.opacity
            );
        }
    }
}

fn write_vector_head(
    svg: &mut String,
    label: &geodesic_core::Label,
    start: geodesic_core::RealAffinePoint,
    end: geodesic_core::RealAffinePoint,
    project: Option<&Project>,
) {
    let length = start.distance_to(end);
    if length <= STANDARD_PRECISION {
        return;
    }
    let style = style_for(project, label);
    let head = (style.size * 6.0).min(length / 2.0);
    let dx = (end.x - start.x) / length;
    let dy = (end.y - start.y) / length;
    let base_x = end.x - dx * head;
    let base_y = end.y - dy * head;
    let offset_x = -dy * head / 2.0;
    let offset_y = dx * head / 2.0;
    let _ = write!(
        svg,
        "<polygon id=\"{label}_head\" points=\"{},{} {},{} {},{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
        end.x,
        -end.y,
        base_x + offset_x,
        -(base_y + offset_y),
        base_x - offset_x,
        -(base_y - offset_y),
        style.color,
        style.opacity
    );
}

fn clipped_ray_endpoint(
    origin: geodesic_core::RealAffinePoint,
    through: geodesic_core::RealAffinePoint,
) -> Option<geodesic_core::RealAffinePoint> {
    let dx = through.x - origin.x;
    let dy = through.y - origin.y;
    let mut parameters = Vec::with_capacity(4);
    if dx.abs() >= STANDARD_PRECISION {
        parameters.extend([-10.0, 10.0].into_iter().filter_map(|x| {
            let parameter = (x - origin.x) / dx;
            let y = origin.y + parameter * dy;
            (parameter >= 0.0 && (-10.0..=10.0).contains(&y)).then_some(parameter)
        }));
    }
    if dy.abs() >= STANDARD_PRECISION {
        parameters.extend([-10.0, 10.0].into_iter().filter_map(|y| {
            let parameter = (y - origin.y) / dy;
            let x = origin.x + parameter * dx;
            (parameter >= 0.0 && (-10.0..=10.0).contains(&x)).then_some(parameter)
        }));
    }
    parameters
        .into_iter()
        .reduce(f64::max)
        .map(|parameter| geodesic_core::RealAffinePoint {
            x: origin.x + parameter * dx,
            y: origin.y + parameter * dy,
        })
}

fn clipped_line(a: f64, b: f64, c: f64) -> Option<((f64, f64), (f64, f64))> {
    let mut points = Vec::with_capacity(4);
    if b.abs() >= STANDARD_PRECISION {
        for x in [-10.0, 10.0] {
            let y = -(a * x + c) / b;
            if (-10.0..=10.0).contains(&y) {
                points.push((x, y));
            }
        }
    }
    if a.abs() >= STANDARD_PRECISION {
        for y in [-10.0, 10.0] {
            let x = -(b * y + c) / a;
            if (-10.0..=10.0).contains(&x)
                && !points.iter().any(|point| {
                    (point.0 - x).abs() < STANDARD_PRECISION
                        && (point.1 - y).abs() < STANDARD_PRECISION
                })
            {
                points.push((x, y));
            }
        }
    }
    points.first().copied().zip(points.get(1).copied())
}

#[cfg(test)]
mod tests {
    use super::render;
    use geodesic_core::{Construction, GeoCommand, Label, ProjectiveLine, ProjectivePoint};
    #[test]
    fn renders_real_geometry_and_hides_non_real_points() {
        let mut construction = Construction::new();
        construction
            .apply(GeoCommand::AddPoint {
                label: Label::parse("point_a").expect("valid"),
                point: ProjectivePoint::real_affine(1.0, 2.0),
            })
            .expect("command");
        construction
            .apply(GeoCommand::AddPoint {
                label: Label::parse("complex_point").expect("valid"),
                point: ProjectivePoint::new(
                    num_complex::Complex64::new(1.0, 1.0),
                    0.0.into(),
                    1.0.into(),
                )
                .expect("valid"),
            })
            .expect("command");
        construction
            .apply(GeoCommand::AddLine {
                label: Label::parse("axis").expect("valid"),
                line: ProjectiveLine::new(0.0.into(), 1.0.into(), 0.0.into()).expect("valid"),
            })
            .expect("command");
        let svg = render(&construction.evaluate().expect("evaluation"));
        assert!(svg.contains("id=\"point_a\""));
        assert!(svg.contains("id=\"axis\""));
        assert!(!svg.contains("complex_point"));
    }

    #[test]
    fn line_visibility_uses_normalized_core_classification() {
        use geodesic_core::ComplexCoordinate as C;
        let mut construction = Construction::new();
        for (name, values) in [
            (
                "phase_real",
                [C::new(0.0, 1e-200), C::new(0.0, 2e-200), C::new(0.0, 0.0)],
            ),
            ("complex_line", [1.0.into(), C::new(0.0, 1.0), 0.0.into()]),
            (
                "uncertain_line",
                [1.0.into(), C::new(0.5, 1e-7), 0.0.into()],
            ),
            ("infinite_line", [0.0.into(), 0.0.into(), 1.0.into()]),
        ] {
            let [a, b, c] = values;
            construction
                .apply(GeoCommand::AddLine {
                    label: Label::parse(name).expect("label"),
                    line: ProjectiveLine::new(a, b, c).expect("line"),
                })
                .expect("command");
        }
        let snapshot = construction.evaluate().expect("snapshot");
        assert_eq!(
            snapshot.iter().count(),
            4,
            "hidden objects remain in the model"
        );
        let svg = render(&snapshot);
        assert!(svg.contains("id=\"phase_real\""));
        for hidden in ["complex_line", "uncertain_line", "infinite_line"] {
            assert!(!svg.contains(hidden));
        }
    }

    #[test]
    fn renders_bounded_lines_and_a_vector_head() {
        let project = geodesic_io::load(
            "(geodesic 1 (point point_a 0 0) (point point_b 2 0) (segment segment_ab point_a point_b) (ray ray_ab point_a point_b) (vector vector_ab point_a point_b))",
        )
        .expect("project");
        let svg = super::render_project(&project).expect("svg");
        for id in ["segment_ab", "ray_ab", "vector_ab", "vector_ab_head"] {
            assert!(svg.contains(&format!("id=\"{id}\"")), "missing {id}");
        }
    }

    #[test]
    fn applies_persisted_direct_style_to_svg() {
        let project = geodesic_io::load(
            "(geodesic 1 (point point_a 0 0) (style point_a \"#ff0000\" 0.3 0.5))",
        )
        .expect("project");
        let svg = super::render_project(&project).expect("render");
        assert!(svg.contains("r=\"0.3\" fill=\"#ff0000\" fill-opacity=\"0.5\""));
    }

    #[test]
    fn applies_style_preset_when_no_direct_override_exists() {
        let project = geodesic_io::load("(geodesic 1 (point point_a 0 0) (style_preset emphasis \"#00ff00\" 0.2 1) (style_use point_a emphasis))").expect("project");
        let svg = super::render_project(&project).expect("render");
        assert!(svg.contains("r=\"0.2\" fill=\"#00ff00\""));
    }
}
