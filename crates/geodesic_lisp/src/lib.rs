//! Restricted script command adapter.
#![forbid(unsafe_code)]

use std::cell::Cell;

use geodesic_core::{Construction, GeoCommand, Label, ProjectivePoint};
use geodesic_io::Project;
use steel::{
    SteelErr,
    steel_vm::{engine::Engine, register_fn::RegisterFn},
};

/// Limits applied before dispatching a project script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptLimits {
    /// Maximum top-level commands.
    pub max_commands: usize,
    /// Maximum UTF-8 source bytes accepted for one script.
    pub max_source_bytes: usize,
    /// Maximum parenthesis nesting depth accepted in one script source.
    pub max_nesting_depth: usize,
    /// Maximum nested application-event dispatches on one thread.
    pub max_event_depth: usize,
}

impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            max_commands: 1_000,
            max_source_bytes: 65_536,
            max_nesting_depth: 64,
            max_event_depth: 8,
        }
    }
}

/// Failure while translating a restricted script form.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("script exceeds command limit")]
    Limit,
    #[error("script exceeds source-size limit")]
    SourceLimit,
    #[error("script exceeds nesting-depth limit")]
    NestingLimit,
    #[error("script has unbalanced parentheses")]
    UnbalancedSource,
    #[error("script event recursion exceeds limit")]
    EventDepthLimit,
    #[error("invalid script form: {0}")]
    Invalid(String),
    #[error(transparent)]
    Command(#[from] geodesic_core::CommandError),
    #[error("Steel evaluation failed: {0}")]
    Steel(String),
}

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
        let command = parse_command(&fields, form.trim())?;
        validate_with_steel(&steel_form(&fields)?)?;
        construction.apply(command)?;
        applied += 1;
    }
    Ok(applied)
}

fn parse_command(fields: &[&str], source: &str) -> Result<GeoCommand, ScriptError> {
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
        _ => Err(ScriptError::Invalid(fields.join(" "))),
    }
}

/// Dispatches scripts assigned to an application event in document order.
///
/// # Errors
///
/// Returns the first failure after preserving commands committed by prior forms.
pub fn dispatch_event(
    project: &Project,
    event: &str,
    construction: &mut Construction,
    limits: ScriptLimits,
) -> Result<usize, ScriptError> {
    let _depth = EventDepthGuard::enter(limits.max_event_depth)?;
    let mut applied = 0;
    for script in project
        .scripts
        .iter()
        .filter(|script| script.event == event)
    {
        let max_commands = limits
            .max_commands
            .checked_sub(applied)
            .ok_or(ScriptError::Limit)?;
        applied += dispatch(
            &script.source,
            construction,
            ScriptLimits {
                max_commands,
                ..limits
            },
        )?;
    }
    Ok(applied)
}

fn validate_nesting(source: &str, maximum: usize) -> Result<(), ScriptError> {
    let mut depth = 0_usize;
    for character in source.chars() {
        match character {
            '(' => {
                depth += 1;
                if depth > maximum {
                    return Err(ScriptError::NestingLimit);
                }
            }
            ')' => {
                depth = depth.checked_sub(1).ok_or(ScriptError::UnbalancedSource)?;
            }
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err(ScriptError::UnbalancedSource)
    }
}

thread_local! { static EVENT_DEPTH: Cell<usize> = const { Cell::new(0) }; }

struct EventDepthGuard;

impl EventDepthGuard {
    fn enter(maximum: usize) -> Result<Self, ScriptError> {
        EVENT_DEPTH.with(|depth| {
            let current = depth.get();
            if current >= maximum {
                Err(ScriptError::EventDepthLimit)
            } else {
                depth.set(current + 1);
                Ok(Self)
            }
        })
    }
}

impl Drop for EventDepthGuard {
    fn drop(&mut self) {
        EVENT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

fn validate_with_steel(source: &str) -> Result<(), ScriptError> {
    fn geo_point(_: String, _: f64, _: f64) {}
    fn geo_point_complex(_: String, _: f64, _: f64, _: f64, _: f64, _: f64, _: f64) {}
    fn geo_relation(_: String, _: String, _: String) {}
    let mut engine = Engine::new();
    engine.register_fn("geo::point", geo_point);
    engine.register_fn("geo::point_complex", geo_point_complex);
    engine.register_fn("geo::line_through", geo_relation);
    engine.register_fn("geo::intersection", geo_relation);
    engine.register_fn("geo::parallel_through", geo_relation);
    engine.register_fn("geo::perpendicular_through", geo_relation);
    engine
        .compile_and_run_raw_program(source.to_owned())
        .map(|_| ())
        .map_err(|error| steel_error(&error))
}

fn steel_error(error: &SteelErr) -> ScriptError {
    ScriptError::Steel(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_prior_commands_when_later_command_fails() {
        let mut construction = Construction::new();
        let result = dispatch(
            "(geo::point point_a 0 0) (unsupported)",
            &mut construction,
            ScriptLimits::default(),
        );
        assert!(result.is_err());
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
}
