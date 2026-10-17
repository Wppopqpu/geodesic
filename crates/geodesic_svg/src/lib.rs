//! Deterministic SVG rendering from immutable geometry snapshots.
#![forbid(unsafe_code)]
use geodesic_core::{GeometricObject, GeometrySnapshot, Renderability, STANDARD_PRECISION};
use geodesic_io::{ObjectStyle, Project};
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
            let _ = write!(
                svg,
                "<circle id=\"{label}\" cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                point.x, -point.y, style.size, style.color, style.opacity
            );
        }
        if let GeometricObject::Line(line) = object {
            let [a, b, c] = line.coefficients();
            if a.im.abs() < STANDARD_PRECISION
                && b.im.abs() < STANDARD_PRECISION
                && c.im.abs() < STANDARD_PRECISION
                && let Some(((x1, y1), (x2, y2))) = clipped_line(a.re, b.re, c.re)
            {
                let style = project.map_or(
                    ObjectStyle {
                        color: "#212121".to_owned(),
                        size: 0.05,
                        opacity: 1.0,
                    },
                    |project| geodesic_io::effective_style(project, label),
                );
                let _ = write!(
                    svg,
                    "<line id=\"{label}\" x1=\"{x1}\" y1=\"{}\" x2=\"{x2}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-opacity=\"{}\"/>",
                    -y1, -y2, style.color, style.size, style.opacity
                );
            }
        }
    }
    svg.push_str("</svg>");
    svg
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
