use std::{collections::BTreeSet, fmt::Write};

use geodesic_core::{GeoCommand, Label, ObjectDefinition, ProjectiveLine, ProjectivePoint};

use crate::{
    LinePattern, ObjectStyle, PointShape, Project, ProjectError, SCHEMA_VERSION, Script,
    StyleBinding, StyleCategory,
};

/// Serializes a project into canonical deterministic S-expression text.
#[must_use]
pub fn save(project: &Project) -> String {
    let mut output = format!("(geodesic {SCHEMA_VERSION}");
    let definitions: Vec<_> = project.construction.definitions().collect();
    let mut written = BTreeSet::new();
    while written.len() < definitions.len() {
        let mut progressed = false;
        for (label, definition) in &definitions {
            if !written.contains(*label) && dependencies_written(definition, &written) {
                write_definition(&mut output, label, definition);
                written.insert((*label).clone());
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    for script in &project.scripts {
        let escaped = script.source.replace('\\', "\\\\").replace('"', "\\\"");
        let event = script.event.replace('\\', "\\\\").replace('"', "\\\"");
        let _ = write!(output, " (script \"{event}\" \"{escaped}\")");
    }
    for (label, binding) in &project.object_styles {
        write_style_binding(&mut output, "style", label, binding);
    }
    for (label, binding) in &project.style_presets {
        write_style_binding(&mut output, "style_preset", label, binding);
    }
    for (category, preset) in &project.category_presets {
        let category = match category {
            StyleCategory::Point => "point",
            StyleCategory::Line => "line",
        };
        let _ = write!(output, " (style_category {category} {preset})");
    }
    output.push(')');
    output
}

/// Parses data only; it never evaluates project scripts.
///
/// # Errors
/// Returns a detailed schema or construction validation error.
pub fn load(source: &str) -> Result<Project, ProjectError> {
    let value =
        lexpr::from_str(source).map_err(|error| ProjectError::Invalid(error.to_string()))?;
    let root = list(&value, "document")?;
    if root.len() < 2 || symbol(&root[0])? != "geodesic" {
        return Err(ProjectError::Invalid("expected `(geodesic 1 ...)`".into()));
    }
    if integer(&root[1])? != i64::from(SCHEMA_VERSION) {
        return Err(ProjectError::Invalid("unsupported schema version".into()));
    }
    let mut project = Project::default();
    for form in &root[2..] {
        load_form(&mut project, list(form, "project form")?.as_slice())?;
    }
    Ok(project)
}

#[allow(
    clippy::too_many_lines,
    reason = "The versioned data grammar is deliberately kept in one exhaustive dispatch."
)]
fn load_form(project: &mut Project, form: &[lexpr::Value]) -> Result<(), ProjectError> {
    match symbol(&form[0])? {
        "point" => project.construction.apply(GeoCommand::AddPoint {
            label: parse_label(form, 1)?,
            point: ProjectivePoint::real_affine(number(form, 2)?, number(form, 3)?),
        })?,
        "point_complex" => project.construction.apply(GeoCommand::AddPoint {
            label: parse_label(form, 1)?,
            point: ProjectivePoint::new(
                num_complex::Complex64::new(number(form, 2)?, number(form, 3)?),
                num_complex::Complex64::new(number(form, 4)?, number(form, 5)?),
                num_complex::Complex64::new(number(form, 6)?, number(form, 7)?),
            )
            .map_err(|error| ProjectError::Invalid(error.to_string()))?,
        })?,
        "line" => project.construction.apply(GeoCommand::AddLine {
            label: parse_label(form, 1)?,
            line: ProjectiveLine::new(
                number(form, 2)?.into(),
                number(form, 3)?.into(),
                number(form, 4)?.into(),
            )
            .map_err(|error| ProjectError::Invalid(error.to_string()))?,
        })?,
        "line_complex" => project.construction.apply(GeoCommand::AddLine {
            label: parse_label(form, 1)?,
            line: ProjectiveLine::new(
                num_complex::Complex64::new(number(form, 2)?, number(form, 3)?),
                num_complex::Complex64::new(number(form, 4)?, number(form, 5)?),
                num_complex::Complex64::new(number(form, 6)?, number(form, 7)?),
            )
            .map_err(|error| ProjectError::Invalid(error.to_string()))?,
        })?,
        "line_through" => project.construction.apply(GeoCommand::AddLineThrough {
            label: parse_label(form, 1)?,
            first: parse_label(form, 2)?,
            second: parse_label(form, 3)?,
        })?,
        "intersection" => project.construction.apply(GeoCommand::AddIntersection {
            label: parse_label(form, 1)?,
            first: parse_label(form, 2)?,
            second: parse_label(form, 3)?,
        })?,
        "parallel_through" => project.construction.apply(GeoCommand::AddParallelThrough {
            label: parse_label(form, 1)?,
            point: parse_label(form, 2)?,
            line: parse_label(form, 3)?,
        })?,
        "perpendicular_through" => {
            project
                .construction
                .apply(GeoCommand::AddPerpendicularThrough {
                    label: parse_label(form, 1)?,
                    point: parse_label(form, 2)?,
                    line: parse_label(form, 3)?,
                })?;
        }
        "segment" => project.construction.apply(GeoCommand::AddSegment {
            label: parse_label(form, 1)?,
            start: parse_label(form, 2)?,
            end: parse_label(form, 3)?,
        })?,
        "ray" => project.construction.apply(GeoCommand::AddRay {
            label: parse_label(form, 1)?,
            origin: parse_label(form, 2)?,
            through: parse_label(form, 3)?,
        })?,
        "vector" => project.construction.apply(GeoCommand::AddVector {
            label: parse_label(form, 1)?,
            start: parse_label(form, 2)?,
            end: parse_label(form, 3)?,
        })?,
        "midpoint" => project.construction.apply(GeoCommand::AddMidpoint {
            label: parse_label(form, 1)?,
            first: parse_label(form, 2)?,
            second: parse_label(form, 3)?,
        })?,
        "perpendicular_bisector" => {
            project
                .construction
                .apply(GeoCommand::AddPerpendicularBisector {
                    label: parse_label(form, 1)?,
                    first: parse_label(form, 2)?,
                    second: parse_label(form, 3)?,
                })?;
        }
        "angle_bisector" => project.construction.apply(GeoCommand::AddAngleBisector {
            label: parse_label(form, 1)?,
            first: parse_label(form, 2)?,
            vertex: parse_label(form, 3)?,
            second: parse_label(form, 4)?,
        })?,
        "line_angle_bisectors" => {
            project
                .construction
                .apply(GeoCommand::AddLineAngleBisectors {
                    first_label: parse_label(form, 1)?,
                    second_label: parse_label(form, 2)?,
                    first: parse_label(form, 3)?,
                    second: parse_label(form, 4)?,
                })?;
        }
        "point_on_locus" => project.construction.apply(GeoCommand::AddPointOnLocus {
            label: parse_label(form, 1)?,
            locus: parse_label(form, 2)?,
            parameter: number(form, 3)?,
        })?,
        "script" if form.len() == 3 => project.scripts.push(Script {
            event: string(&form[1])?.to_owned(),
            source: string(&form[2])?.to_owned(),
        }),
        "style" if matches!(form.len(), 5 | 7) => {
            project.object_styles.insert(
                parse_label(form, 1)?,
                StyleBinding::Concrete(parse_style(form)?),
            );
        }
        "style_preset" if matches!(form.len(), 5 | 7) => {
            project.style_presets.insert(
                parse_label(form, 1)?,
                StyleBinding::Concrete(parse_style(form)?),
            );
        }
        "style_use" if form.len() == 3 => {
            project.object_styles.insert(
                parse_label(form, 1)?,
                StyleBinding::Fallback(parse_label(form, 2)?),
            );
        }
        "style_preset_use" if form.len() == 3 => {
            project.style_presets.insert(
                parse_label(form, 1)?,
                StyleBinding::Fallback(parse_label(form, 2)?),
            );
        }
        "style_category" if form.len() == 3 => {
            let category = match symbol(&form[1])? {
                "point" => StyleCategory::Point,
                "line" => StyleCategory::Line,
                other => {
                    return Err(ProjectError::Invalid(format!(
                        "unknown style category `{other}`"
                    )));
                }
            };
            project
                .category_presets
                .insert(category, parse_label(form, 2)?);
        }
        other => {
            return Err(ProjectError::Invalid(format!(
                "unknown or malformed form `{other}`"
            )));
        }
    }
    Ok(())
}

fn dependencies_written(definition: &ObjectDefinition, written: &BTreeSet<Label>) -> bool {
    match definition {
        ObjectDefinition::FreePoint(_) | ObjectDefinition::FreeLine(_) => true,
        ObjectDefinition::LineThrough { first, second }
        | ObjectDefinition::Intersection { first, second }
        | ObjectDefinition::LineAngleBisector { first, second, .. } => {
            written.contains(first) && written.contains(second)
        }
        ObjectDefinition::PointOnLocus { locus, .. } => written.contains(locus),
        ObjectDefinition::ParallelThrough { point, line }
        | ObjectDefinition::PerpendicularThrough { point, line } => {
            written.contains(point) && written.contains(line)
        }
        ObjectDefinition::Segment { start, end }
        | ObjectDefinition::Vector { start, end }
        | ObjectDefinition::Midpoint {
            first: start,
            second: end,
        }
        | ObjectDefinition::PerpendicularBisector {
            first: start,
            second: end,
        } => written.contains(start) && written.contains(end),
        ObjectDefinition::Ray { origin, through } => {
            written.contains(origin) && written.contains(through)
        }
        ObjectDefinition::AngleBisector {
            first,
            vertex,
            second,
        } => written.contains(first) && written.contains(vertex) && written.contains(second),
    }
}

fn write_definition(output: &mut String, label: &Label, definition: &ObjectDefinition) {
    match definition {
        ObjectDefinition::FreePoint(point) => {
            let [x, y, z] = point.coordinates();
            if z.im.abs() < geodesic_core::STANDARD_PRECISION
                && (z.re - 1.0).abs() < geodesic_core::STANDARD_PRECISION
                && x.im.abs() < geodesic_core::STANDARD_PRECISION
                && y.im.abs() < geodesic_core::STANDARD_PRECISION
            {
                let _ = write!(output, " (point {label} {} {})", x.re, y.re);
            } else {
                let _ = write!(
                    output,
                    " (point_complex {label} {} {} {} {} {} {})",
                    x.re, x.im, y.re, y.im, z.re, z.im
                );
            }
        }
        ObjectDefinition::FreeLine(line) => {
            let [a, b, c] = line.coefficients();
            if a.im.abs() < geodesic_core::STANDARD_PRECISION
                && b.im.abs() < geodesic_core::STANDARD_PRECISION
                && c.im.abs() < geodesic_core::STANDARD_PRECISION
            {
                let _ = write!(output, " (line {label} {} {} {})", a.re, b.re, c.re);
            } else {
                let _ = write!(
                    output,
                    " (line_complex {label} {} {} {} {} {} {})",
                    a.re, a.im, b.re, b.im, c.re, c.im
                );
            }
        }
        ObjectDefinition::LineThrough { first, second } => {
            let _ = write!(output, " (line_through {label} {first} {second})");
        }
        ObjectDefinition::Intersection { first, second } => {
            let _ = write!(output, " (intersection {label} {first} {second})");
        }
        ObjectDefinition::ParallelThrough { point, line } => {
            let _ = write!(output, " (parallel_through {label} {point} {line})");
        }
        ObjectDefinition::PerpendicularThrough { point, line } => {
            let _ = write!(output, " (perpendicular_through {label} {point} {line})");
        }
        ObjectDefinition::Segment { start, end } => {
            let _ = write!(output, " (segment {label} {start} {end})");
        }
        ObjectDefinition::Ray { origin, through } => {
            let _ = write!(output, " (ray {label} {origin} {through})");
        }
        ObjectDefinition::Vector { start, end } => {
            let _ = write!(output, " (vector {label} {start} {end})");
        }
        ObjectDefinition::Midpoint { first, second } => {
            let _ = write!(output, " (midpoint {label} {first} {second})");
        }
        ObjectDefinition::PerpendicularBisector { first, second } => {
            let _ = write!(output, " (perpendicular_bisector {label} {first} {second})");
        }
        ObjectDefinition::AngleBisector {
            first,
            vertex,
            second,
        } => {
            let _ = write!(
                output,
                " (angle_bisector {label} {first} {vertex} {second})"
            );
        }
        ObjectDefinition::LineAngleBisector {
            group,
            peer,
            first,
            second,
            branch,
        } if group == label && matches!(branch, geodesic_core::LineAngleBisectorBranch::Sum) => {
            let _ = write!(
                output,
                " (line_angle_bisectors {label} {peer} {first} {second})"
            );
        }
        ObjectDefinition::LineAngleBisector { .. } => {}
        ObjectDefinition::PointOnLocus { locus, parameter } => {
            let _ = write!(output, " (point_on_locus {label} {locus} {parameter})");
        }
    }
}

fn list(value: &lexpr::Value, context: &str) -> Result<Vec<lexpr::Value>, ProjectError> {
    value
        .to_vec()
        .ok_or_else(|| ProjectError::Invalid(format!("expected list for {context}")))
}
fn symbol(value: &lexpr::Value) -> Result<&str, ProjectError> {
    value
        .as_symbol()
        .ok_or_else(|| ProjectError::Invalid("expected symbol".into()))
}
fn string(value: &lexpr::Value) -> Result<&str, ProjectError> {
    value
        .as_str()
        .ok_or_else(|| ProjectError::Invalid("expected string".into()))
}
fn integer(value: &lexpr::Value) -> Result<i64, ProjectError> {
    value
        .as_i64()
        .ok_or_else(|| ProjectError::Invalid("expected integer".into()))
}
fn number(form: &[lexpr::Value], index: usize) -> Result<f64, ProjectError> {
    form.get(index)
        .and_then(lexpr::Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| ProjectError::Number(index.to_string()))
}
fn parse_label(form: &[lexpr::Value], index: usize) -> Result<Label, ProjectError> {
    let value = form
        .get(index)
        .ok_or_else(|| ProjectError::Invalid("missing label".into()))?;
    Label::parse(symbol(value)?).map_err(|error| ProjectError::Invalid(error.to_string()))
}

fn parse_style(form: &[lexpr::Value]) -> Result<ObjectStyle, ProjectError> {
    let color = string(&form[2])?.to_owned();
    if !color.starts_with('#')
        || color.len() != 7
        || !color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ProjectError::Invalid(
            "style color must be a #RRGGBB value".to_owned(),
        ));
    }
    let size = number(form, 3)?;
    let opacity = number(form, 4)?;
    if !(size.is_sign_positive() && (0.0..=1.0).contains(&opacity)) {
        return Err(ProjectError::Invalid(
            "style size must be positive and opacity must be within 0..=1".to_owned(),
        ));
    }
    let mut style = ObjectStyle {
        color,
        size,
        opacity,
        ..ObjectStyle::default()
    };
    if form.len() == 7 {
        style.point_shape = match symbol(&form[5])? {
            "circle" => PointShape::Circle,
            "square" => PointShape::Square,
            "diamond" => PointShape::Diamond,
            "triangle" => PointShape::Triangle,
            other => {
                return Err(ProjectError::Invalid(format!(
                    "unknown point shape `{other}`"
                )));
            }
        };
        style.line_pattern = match symbol(&form[6])? {
            "solid" => LinePattern::Solid,
            "dashed" => LinePattern::Dashed,
            "dotted" => LinePattern::Dotted,
            other => {
                return Err(ProjectError::Invalid(format!(
                    "unknown line pattern `{other}`"
                )));
            }
        };
    }
    Ok(style)
}

fn write_style_binding(output: &mut String, form: &str, label: &Label, binding: &StyleBinding) {
    match binding {
        StyleBinding::Concrete(style) => {
            let point_shape = match style.point_shape {
                PointShape::Circle => "circle",
                PointShape::Square => "square",
                PointShape::Diamond => "diamond",
                PointShape::Triangle => "triangle",
            };
            let line_pattern = match style.line_pattern {
                LinePattern::Solid => "solid",
                LinePattern::Dashed => "dashed",
                LinePattern::Dotted => "dotted",
            };
            let _ = write!(
                output,
                " ({form} {label} \"{}\" {} {} {point_shape} {line_pattern})",
                style.color, style.size, style.opacity,
            );
        }
        StyleBinding::Fallback(target) => {
            let form = if form == "style" {
                "style_use"
            } else {
                "style_preset_use"
            };
            let _ = write!(output, " ({form} {label} {target})");
        }
    }
}
