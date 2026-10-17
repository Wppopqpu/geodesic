use geodesic_core::{
    Construction, GeoCommand, GeometricObject, Label, ProjectivePoint, parallel_through,
};

use crate::state::MAX_COMPLETIONS;

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

const COMMAND_NAMES: [&str; 5] = ["point", "line", "move", "undo", "redo"];

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
            "point" | "line" if position == 1 => true,
            "move" if position == 1 => {
                matches!(object, GeometricObject::Point(_) | GeometricObject::Line(_))
            }
            "line" if position >= 2 => matches!(object, GeometricObject::Point(_)),
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
            "commands: :point label x y | :line label point_a point_b | :move label x y | :undo | :redo"
        }
        Some("point") => "usage: :point label x y",
        Some("line") => "usage: :line label point_a point_b",
        Some("move") => "usage: :move free_point_or_line x y",
        Some("undo") => "usage: :undo",
        Some("redo") => "usage: :redo",
        Some(_) => "unknown command; use :point, :line, :move, :undo, or :redo",
    }
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
