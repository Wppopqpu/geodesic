//! Line-delimited JSON-RPC adapter over the core document command surface.
#![forbid(unsafe_code)]
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines().map_while(Result::ok) {
        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                println!(
                    "{}",
                    error_response(serde_json::Value::Null, -32700, error.to_string())
                );
                continue;
            }
        };
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let response = request
            .get("method")
            .and_then(serde_json::Value::as_str)
            .map_or_else(
                || error_response(id.clone(), -32600, "missing method"),
                |method| dispatch(method, request.get("params"), id.clone()),
            );
        println!("{response}");
    }
}

fn dispatch(
    method: &str,
    params: Option<&serde_json::Value>,
    id: serde_json::Value,
) -> serde_json::Value {
    let source = params
        .and_then(|params| params.get("project"))
        .and_then(serde_json::Value::as_str);
    let Some(source) = source else {
        return error_response(id, -32602, "params.project must be a project string");
    };
    let mut project = match geodesic_io::load(source) {
        Ok(project) => project,
        Err(error) => return error_response(id, -32001, error.to_string()),
    };
    match method {
        "validate" => success_response(id, serde_json::json!({"valid": true})),
        "render_svg" => match geodesic_svg::render_project(&project) {
            Ok(svg) => success_response(id, serde_json::json!({"svg": svg})),
            Err(error) => error_response(id, -32002, error.to_string()),
        },
        "eval" => match project.construction.evaluate() {
            Ok(snapshot) => success_response(
                id,
                serde_json::json!({"objects": snapshot.iter().map(|(label, _)| label.to_string()).collect::<Vec<_>>() }),
            ),
            Err(error) => error_response(id, -32002, error.to_string()),
        },
        "apply" => match apply_request(params, &mut project) {
            Ok(()) => success_response(
                id,
                serde_json::json!({"project": geodesic_io::save(&project)}),
            ),
            Err(error) => error_response(id, -32602, error),
        },
        _ => error_response(id, -32601, "unknown method"),
    }
}

fn apply_request(
    params: Option<&serde_json::Value>,
    project: &mut geodesic_io::Project,
) -> Result<(), String> {
    let command = params
        .and_then(|params| params.get("command"))
        .and_then(serde_json::Value::as_object)
        .ok_or("params.command must be an object")?;
    let operation = command
        .get("op")
        .and_then(serde_json::Value::as_str)
        .ok_or("command.op must be a string")?;
    let command = match operation {
        "add_point" => geodesic_core::GeoCommand::AddPoint {
            label: json_label(command, "label")?,
            point: geodesic_core::ProjectivePoint::real_affine(
                json_number(command, "x")?,
                json_number(command, "y")?,
            ),
        },
        "add_complex_point" => geodesic_core::GeoCommand::AddPoint {
            label: json_label(command, "label")?,
            point: geodesic_core::ProjectivePoint::new(
                num_complex::Complex64::new(
                    json_number(command, "x_re")?,
                    json_number(command, "x_im")?,
                ),
                num_complex::Complex64::new(
                    json_number(command, "y_re")?,
                    json_number(command, "y_im")?,
                ),
                num_complex::Complex64::new(
                    json_number(command, "z_re")?,
                    json_number(command, "z_im")?,
                ),
            )
            .map_err(|error| error.to_string())?,
        },
        "add_line_through" => geodesic_core::GeoCommand::AddLineThrough {
            label: json_label(command, "label")?,
            first: json_label(command, "first")?,
            second: json_label(command, "second")?,
        },
        "move_point" => geodesic_core::GeoCommand::MovePoint {
            label: json_label(command, "label")?,
            point: geodesic_core::ProjectivePoint::real_affine(
                json_number(command, "x")?,
                json_number(command, "y")?,
            ),
        },
        "add_intersection" => geodesic_core::GeoCommand::AddIntersection {
            label: json_label(command, "label")?,
            first: json_label(command, "first")?,
            second: json_label(command, "second")?,
        },
        "add_parallel_through" => geodesic_core::GeoCommand::AddParallelThrough {
            label: json_label(command, "label")?,
            point: json_label(command, "point")?,
            line: json_label(command, "line")?,
        },
        "add_perpendicular_through" => geodesic_core::GeoCommand::AddPerpendicularThrough {
            label: json_label(command, "label")?,
            point: json_label(command, "point")?,
            line: json_label(command, "line")?,
        },
        _ => return Err("unsupported command.op".to_owned()),
    };
    geodesic_io::apply_command(project, command).map_err(|error| error.to_string())
}

fn json_label(
    command: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<geodesic_core::Label, String> {
    let value = command
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("command.{field} must be a label string"))?;
    geodesic_core::Label::parse(value).map_err(|error| error.to_string())
}

fn json_number(
    command: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Result<f64, String> {
    command
        .get(field)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("command.{field} must be a finite number"))
}

fn success_response(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    let mut response = serde_json::Map::new();
    response.insert("jsonrpc".into(), serde_json::Value::String("2.0".into()));
    response.insert("id".into(), id);
    response.insert("result".into(), result);
    serde_json::Value::Object(response)
}
fn error_response(
    id: serde_json::Value,
    code: i64,
    message: impl Into<String>,
) -> serde_json::Value {
    let mut error = serde_json::Map::new();
    error.insert("code".into(), serde_json::Value::Number(code.into()));
    error.insert("message".into(), serde_json::Value::String(message.into()));
    let mut response = serde_json::Map::new();
    response.insert("jsonrpc".into(), serde_json::Value::String("2.0".into()));
    response.insert("id".into(), id);
    response.insert("error".into(), serde_json::Value::Object(error));
    serde_json::Value::Object(response)
}
