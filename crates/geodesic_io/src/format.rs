use std::{collections::BTreeSet, fmt::Write};

use geodesic_core::{GeoCommand, Label, ObjectDefinition, ProjectiveLine, ProjectivePoint};

use crate::{ObjectStyle, Project, ProjectError, SCHEMA_VERSION, Script};

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
    for (label, style) in &project.object_styles {
        let _ = write!(
            output,
            " (style {label} \"{}\" {} {})",
            style.color, style.size, style.opacity
        );
    }
    for (label, style) in &project.style_presets {
        let _ = write!(
            output,
            " (style_preset {label} \"{}\" {} {})",
            style.color, style.size, style.opacity
        );
    }
    for (label, preset) in &project.object_presets {
        let _ = write!(output, " (style_use {label} {preset})");
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
        "script" if form.len() == 3 => project.scripts.push(Script {
            event: string(&form[1])?.to_owned(),
            source: string(&form[2])?.to_owned(),
        }),
        "style" if form.len() == 5 => {
            project
                .object_styles
                .insert(parse_label(form, 1)?, parse_style(form)?);
        }
        "style_preset" if form.len() == 5 => {
            project
                .style_presets
                .insert(parse_label(form, 1)?, parse_style(form)?);
        }
        "style_use" if form.len() == 3 => {
            project
                .object_presets
                .insert(parse_label(form, 1)?, parse_label(form, 2)?);
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
        | ObjectDefinition::Intersection { first, second } => {
            written.contains(first) && written.contains(second)
        }
        ObjectDefinition::ParallelThrough { point, line }
        | ObjectDefinition::PerpendicularThrough { point, line } => {
            written.contains(point) && written.contains(line)
        }
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
    Ok(ObjectStyle {
        color,
        size,
        opacity,
    })
}
