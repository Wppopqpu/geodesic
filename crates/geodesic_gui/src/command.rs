use geodesic_core::{
    Construction, GeoCommand, GeometricObject, Label, ProjectivePoint, parallel_through,
};

use crate::state::MAX_COMPLETIONS;

#[allow(
    clippy::too_many_lines,
    reason = "The command bar keeps its small public grammar in one exhaustive dispatch."
)]
pub(crate) fn execute_command(
    construction: &mut Construction,
    source: &str,
) -> Result<String, String> {
    let fields: Vec<_> = source
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .collect();
    let command = fields.first().ok_or("empty command")?;
    match *command {
        "point" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            let x = parse_coordinate(fields[2])?;
            let y = parse_coordinate(fields[3])?;
            construction
                .apply(GeoCommand::AddPoint {
                    label: label.clone(),
                    point: ProjectivePoint::real_affine(x, y),
                })
                .map_err(|error| error.to_string())?;
            Ok(format!("created point {label}"))
        }
        "line" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            construction
                .apply(GeoCommand::AddLineThrough {
                    label: label.clone(),
                    first: parse_label(fields[2])?,
                    second: parse_label(fields[3])?,
                })
                .map_err(|error| error.to_string())?;
            Ok(format!("created line {label}"))
        }
        "segment" if fields.len() == 4 => apply_relation(
            construction,
            GeoCommand::AddSegment {
                label: parse_label(fields[1])?,
                start: parse_label(fields[2])?,
                end: parse_label(fields[3])?,
            },
            "segment",
        ),
        "ray" if fields.len() == 4 => apply_relation(
            construction,
            GeoCommand::AddRay {
                label: parse_label(fields[1])?,
                origin: parse_label(fields[2])?,
                through: parse_label(fields[3])?,
            },
            "ray",
        ),
        "vector" if fields.len() == 4 => apply_relation(
            construction,
            GeoCommand::AddVector {
                label: parse_label(fields[1])?,
                start: parse_label(fields[2])?,
                end: parse_label(fields[3])?,
            },
            "vector",
        ),
        "midpoint" if fields.len() == 4 => apply_relation(
            construction,
            GeoCommand::AddMidpoint {
                label: parse_label(fields[1])?,
                first: parse_label(fields[2])?,
                second: parse_label(fields[3])?,
            },
            "midpoint",
        ),
        "perpendicular_bisector" if fields.len() == 4 => apply_relation(
            construction,
            GeoCommand::AddPerpendicularBisector {
                label: parse_label(fields[1])?,
                first: parse_label(fields[2])?,
                second: parse_label(fields[3])?,
            },
            "perpendicular bisector",
        ),
        "angle_bisector" if fields.len() == 5 => apply_relation(
            construction,
            GeoCommand::AddAngleBisector {
                label: parse_label(fields[1])?,
                first: parse_label(fields[2])?,
                vertex: parse_label(fields[3])?,
                second: parse_label(fields[4])?,
            },
            "angle bisector",
        ),
        "move" if fields.len() == 4 => {
            let label = parse_label(fields[1])?;
            let x = parse_coordinate(fields[2])?;
            let y = parse_coordinate(fields[3])?;
            let object = construction
                .evaluate()
                .map_err(|error| error.to_string())?
                .object(&label)
                .ok_or_else(|| format!("object label `{label}` does not exist"))?;
            let command = match object {
                GeometricObject::Point(_) => GeoCommand::MovePoint {
                    label: label.clone(),
                    point: ProjectivePoint::real_affine(x, y),
                },
                GeometricObject::Line(line) => GeoCommand::MoveLine {
                    label: label.clone(),
                    line: parallel_through(ProjectivePoint::real_affine(x, y), line)
                        .map_err(|error| error.to_string())?,
                },
                GeometricObject::Segment(_)
                | GeometricObject::Ray(_)
                | GeometricObject::Vector(_) => {
                    return Err("only free points and lines can be moved".to_owned());
                }
            };
            construction
                .apply(command)
                .map_err(|error| error.to_string())?;
            Ok(format!("moved object {label}"))
        }
        "undo" if fields.len() == 1 => construction
            .undo()
            .map(|()| "undid command".to_owned())
            .map_err(|error| error.to_string()),
        "redo" if fields.len() == 1 => construction
            .redo()
            .map(|()| "redid command".to_owned())
            .map_err(|error| error.to_string()),
        _ => Err(command_hint(source).to_owned()),
    }
}

const COMMAND_NAMES: [&str; 11] = [
    "point",
    "line",
    "segment",
    "ray",
    "vector",
    "midpoint",
    "perpendicular_bisector",
    "angle_bisector",
    "move",
    "undo",
    "redo",
];

pub(crate) fn command_completions(construction: &Construction, source: &str) -> Vec<String> {
    let trimmed = source.trim_start();
    let without_colon = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let fields: Vec<_> = without_colon.split_whitespace().collect();
    let trailing_space = without_colon.ends_with(char::is_whitespace);
    let position = if trailing_space {
        fields.len()
    } else {
        fields.len().saturating_sub(1)
    };
    let partial = if trailing_space {
        ""
    } else {
        fields.last().copied().unwrap_or_default()
    };

    if fields.is_empty() || position == 0 {
        return COMMAND_NAMES
            .into_iter()
            .filter(|name| name.starts_with(partial))
            .map(str::to_owned)
            .collect();
    }

    let command = fields[0];
    let labels = labels_for_position(construction, command, position);
    labels
        .into_iter()
        .filter(|label| label.starts_with(partial))
        .take(MAX_COMPLETIONS)
        .collect()
}

fn labels_for_position(construction: &Construction, command: &str, position: usize) -> Vec<String> {
    let Ok(snapshot) = construction.evaluate() else {
        return Vec::new();
    };
    snapshot
        .iter()
        .filter(|(_, object)| match command {
            "point"
            | "line"
            | "segment"
            | "ray"
            | "vector"
            | "midpoint"
            | "perpendicular_bisector"
            | "angle_bisector"
                if position == 1 =>
            {
                true
            }
            "move" if position == 1 => {
                matches!(object, GeometricObject::Point(_) | GeometricObject::Line(_))
            }
            "line"
            | "segment"
            | "ray"
            | "vector"
            | "midpoint"
            | "perpendicular_bisector"
            | "angle_bisector"
                if position >= 2 =>
            {
                matches!(object, GeometricObject::Point(_))
            }
            _ => false,
        })
        .map(|(label, _)| label.to_string())
        .collect()
}

pub(crate) fn replace_completion(source: &str, completion: &str) -> String {
    let token_start = source
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or_else(
            || usize::from(source.starts_with(':')),
            |(index, character)| index + character.len_utf8(),
        );
    format!("{}{}", &source[..token_start], completion)
}

pub(crate) fn command_hint(source: &str) -> &'static str {
    let fields: Vec<_> = source
        .trim()
        .trim_start_matches(':')
        .split_whitespace()
        .collect();
    match fields.first().copied() {
        None => {
            "commands: :point | :line | :segment | :ray | :vector | :midpoint | :perpendicular_bisector | :angle_bisector | :move | :undo | :redo"
        }
        Some("point") => "usage: :point label x y",
        Some("line") => "usage: :line label point_a point_b",
        Some("segment") => "usage: :segment label start end",
        Some("ray") => "usage: :ray label origin through",
        Some("vector") => "usage: :vector label start end",
        Some("midpoint") => "usage: :midpoint label first second",
        Some("perpendicular_bisector") => "usage: :perpendicular_bisector label first second",
        Some("angle_bisector") => "usage: :angle_bisector label first vertex second",
        Some("move") => "usage: :move free_point_or_line x y",
        Some("undo") => "usage: :undo",
        Some("redo") => "usage: :redo",
        Some(_) => "unknown command; use a geometry command or :move, :undo, :redo",
    }
}

fn apply_relation(
    construction: &mut Construction,
    command: GeoCommand,
    description: &str,
) -> Result<String, String> {
    construction
        .apply(command)
        .map(|()| format!("created {description}"))
        .map_err(|error| error.to_string())
}

pub(crate) fn history_previous(
    command: &mut String,
    cursor: &mut Option<usize>,
    history: &[String],
) {
    if history.is_empty() {
        return;
    }
    let index = cursor.map_or(history.len() - 1, |index| index.saturating_sub(1));
    *cursor = Some(index);
    command.clone_from(&history[index]);
}

pub(crate) fn history_next(command: &mut String, cursor: &mut Option<usize>, history: &[String]) {
    let Some(index) = *cursor else {
        return;
    };
    if index + 1 < history.len() {
        let next = index + 1;
        *cursor = Some(next);
        command.clone_from(&history[next]);
    } else {
        *cursor = None;
        command.clear();
    }
}

fn parse_label(source: &str) -> Result<Label, String> {
    Label::parse(source).map_err(|error| error.to_string())
}

fn parse_coordinate(source: &str) -> Result<f64, String> {
    source
        .parse()
        .map_err(|_| format!("invalid coordinate `{source}`"))
}
