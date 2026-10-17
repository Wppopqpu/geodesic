use geodesic_core::{Construction, GeoCommand, Label, ProjectivePoint};

use crate::{ScriptError, ScriptLimits, steel::validate_with_steel};

/// Executes registered `geo::point` commands incrementally; prior commands persist on failure.
///
/// This adapter deliberately exposes no host I/O primitives.
///
/// # Errors
///
/// Returns an error for malformed forms, rejected commands, or exhausted limits.
pub fn dispatch(
    source: &str,
    construction: &mut Construction,
    limits: ScriptLimits,
) -> Result<usize, ScriptError> {
    if source.len() > limits.max_source_bytes {
        return Err(ScriptError::SourceLimit);
    }
    validate_nesting(source, limits.max_nesting_depth)?;
    let mut applied = 0;
    for form in source.split(')').filter(|form| !form.trim().is_empty()) {
        if applied == limits.max_commands {
            return Err(ScriptError::Limit);
        }
        let fields: Vec<_> = form
            .trim()
            .trim_start_matches('(')
            .split_whitespace()
            .collect();
        let command = parse_command(&fields, form.trim(), construction)?;
        validate_with_steel(&steel_form(&fields)?)?;
        construction.apply(command)?;
        applied += 1;
    }
    Ok(applied)
}

fn parse_command(
    fields: &[&str],
    source: &str,
    construction: &Construction,
) -> Result<GeoCommand, ScriptError> {
    let label = |value: &&str| {
        Label::parse(*value).map_err(|error| ScriptError::Invalid(error.to_string()))
    };
    match fields {
        ["geo::point", object, x, y] => Ok(GeoCommand::AddPoint {
            label: label(object)?,
            point: ProjectivePoint::real_affine(parse_number(x, source)?, parse_number(y, source)?),
        }),
        [
            "geo::point_complex",
            object,
            x_re,
            x_im,
            y_re,
            y_im,
            z_re,
            z_im,
        ] => Ok(GeoCommand::AddPoint {
            label: label(object)?,
            point: ProjectivePoint::new(
                geodesic_core::ComplexCoordinate::new(
                    parse_number(x_re, source)?,
                    parse_number(x_im, source)?,
                ),
                geodesic_core::ComplexCoordinate::new(
                    parse_number(y_re, source)?,
                    parse_number(y_im, source)?,
                ),
                geodesic_core::ComplexCoordinate::new(
                    parse_number(z_re, source)?,
                    parse_number(z_im, source)?,
                ),
            )
            .map_err(|error| ScriptError::Invalid(error.to_string()))?,
        }),
        ["geo::line_through", object, first, second] => Ok(GeoCommand::AddLineThrough {
            label: label(object)?,
            first: label(first)?,
            second: label(second)?,
        }),
        ["geo::move_line", object, x, y] => {
            let label = label(object)?;
            let line = match construction
                .evaluate()
                .map_err(|error| ScriptError::Invalid(error.to_string()))?
                .object(&label)
            {
                Some(geodesic_core::GeometricObject::Line(line)) => line,
                Some(geodesic_core::GeometricObject::Point(_)) => {
                    return Err(ScriptError::Invalid(format!(
                        "object `{label}` is not a line"
                    )));
                }
                None => {
                    return Err(ScriptError::Invalid(format!(
                        "object label `{label}` does not exist"
                    )));
                }
            };
            Ok(GeoCommand::MoveLine {
                label,
                line: geodesic_core::parallel_through(
                    ProjectivePoint::real_affine(
                        parse_number(x, source)?,
                        parse_number(y, source)?,
                    ),
                    line,
                )
                .map_err(|error| ScriptError::Invalid(error.to_string()))?,
            })
        }
        ["geo::intersection", object, first, second] => Ok(GeoCommand::AddIntersection {
            label: label(object)?,
            first: label(first)?,
            second: label(second)?,
        }),
        ["geo::parallel_through", object, point, line] => Ok(GeoCommand::AddParallelThrough {
            label: label(object)?,
            point: label(point)?,
            line: label(line)?,
        }),
        ["geo::perpendicular_through", object, point, line] => {
            Ok(GeoCommand::AddPerpendicularThrough {
                label: label(object)?,
                point: label(point)?,
                line: label(line)?,
            })
        }
        _ => Err(ScriptError::Invalid(source.to_owned())),
    }
}

fn parse_number(value: &str, source: &str) -> Result<f64, ScriptError> {
    value
        .parse()
        .map_err(|_| ScriptError::Invalid(source.to_owned()))
}

fn steel_form(fields: &[&str]) -> Result<String, ScriptError> {
    match fields {
        ["geo::point", label, x, y] => {
            let x = parse_number(x, "number")?;
            let y = parse_number(y, "number")?;
            Ok(format!("(geo::point \"{label}\" {x:?} {y:?})"))
        }
        [
            "geo::point_complex",
            label,
            x_re,
            x_im,
            y_re,
            y_im,
            z_re,
            z_im,
        ] => {
            let x_re = parse_number(x_re, "number")?;
            let x_im = parse_number(x_im, "number")?;
            let y_re = parse_number(y_re, "number")?;
            let y_im = parse_number(y_im, "number")?;
            let z_re = parse_number(z_re, "number")?;
            let z_im = parse_number(z_im, "number")?;
            Ok(format!(
                "(geo::point_complex \"{label}\" {x_re:?} {x_im:?} {y_re:?} {y_im:?} {z_re:?} {z_im:?})"
            ))
        }
        [
            name @ ("geo::line_through"
            | "geo::intersection"
            | "geo::parallel_through"
            | "geo::perpendicular_through"),
            object,
            first,
            second,
        ] => Ok(format!("({name} \"{object}\" \"{first}\" \"{second}\")")),
        ["geo::move_line", label, x, y] => {
            let x = parse_number(x, "number")?;
            let y = parse_number(y, "number")?;
            Ok(format!("(geo::move_line \"{label}\" {x:?} {y:?})"))
        }
        _ => Err(ScriptError::Invalid(fields.join(" "))),
    }
}

pub(super) fn validate_nesting(source: &str, maximum: usize) -> Result<(), ScriptError> {
    let mut depth = 0_usize;
    for character in source.chars() {
        match character {
            '(' => {
                depth += 1;
                if depth > maximum {
                    return Err(ScriptError::NestingLimit);
                }
            }
            ')' => depth = depth.checked_sub(1).ok_or(ScriptError::UnbalancedSource)?,
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err(ScriptError::UnbalancedSource)
    }
}
