#![forbid(unsafe_code)]
use clap::{Parser, Subcommand};
use std::{fs, process::ExitCode};
#[derive(Parser)]
#[command(name = "geodesic")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Validates a native project document.
    Validate { project: String },
    /// Renders a native project document to deterministic SVG on stdout or a file.
    RenderSvg {
        project: String,
        /// Writes SVG to this file instead of stdout.
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Imports a read-only `.ggb` archive and prints native project data.
    Import { archive: String },
    /// Evaluates a project and prints a stable object summary.
    Eval {
        project: String,
        #[arg(long)]
        json: bool,
    },
    /// Applies one point-line construction command and prints the updated document.
    Apply {
        project: String,
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        operation: Operation,
    },
}

#[derive(Subcommand)]
enum Operation {
    AddPoint {
        label: String,
        x: f64,
        y: f64,
    },
    AddComplexPoint {
        label: String,
        x_re: f64,
        x_im: f64,
        y_re: f64,
        y_im: f64,
        z_re: f64,
        z_im: f64,
    },
    AddLineThrough {
        label: String,
        first: String,
        second: String,
    },
    MovePoint {
        label: String,
        x: f64,
        y: f64,
    },
    MoveLine {
        label: String,
        x: f64,
        y: f64,
    },
    AddIntersection {
        label: String,
        first: String,
        second: String,
    },
    AddParallelThrough {
        label: String,
        point: String,
        line: String,
    },
    AddPerpendicularThrough {
        label: String,
        point: String,
        line: String,
    },
    AddSegment {
        label: String,
        start: String,
        end: String,
    },
    AddRay {
        label: String,
        origin: String,
        through: String,
    },
    AddVector {
        label: String,
        start: String,
        end: String,
    },
    AddMidpoint {
        label: String,
        first: String,
        second: String,
    },
    AddPerpendicularBisector {
        label: String,
        first: String,
        second: String,
    },
    AddAngleBisector {
        label: String,
        first: String,
        vertex: String,
        second: String,
    },
    Undo,
    Redo,
}
fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Validate { project } => fs::read_to_string(project)
            .map_err(|error| error.to_string())
            .and_then(|source| {
                geodesic_io::load(&source)
                    .map(|_| "valid".to_owned())
                    .map_err(|error| error.to_string())
            }),
        Command::RenderSvg { project, output } => fs::read_to_string(project)
            .map_err(|error| error.to_string())
            .and_then(|source| geodesic_io::load(&source).map_err(|error| error.to_string()))
            .and_then(|project| geodesic_svg::render_project(&project).map_err(|error| error.to_string()))
            .and_then(|svg| match output {
                Some(path) => fs::write(&path, svg)
                    .map(|()| format!("rendered {path}"))
                    .map_err(|error| error.to_string()),
                None => Ok(svg),
            }),
        Command::Import { archive } => fs::read(archive)
            .map_err(|error| error.to_string())
            .and_then(|bytes| geodesic_io::import_ggb(&bytes).map_err(|error| error.to_string()))
            .map(|(project, report)| serde_json::json!({
                "project": geodesic_io::save(&project),
                "report": {"converted": report.converted, "ignored_ggb_script": report.ignored_ggb_script, "placeholders": report.placeholders.iter().map(|placeholder| serde_json::json!({"label": placeholder.label, "source_type": placeholder.source_type, "definition_summary": placeholder.definition_summary, "dependencies": placeholder.dependencies})).collect::<Vec<_>>()}
            }).to_string()),
        Command::Eval { project, json } => fs::read_to_string(project)
            .map_err(|error| error.to_string())
            .and_then(|source| geodesic_io::load(&source).map_err(|error| error.to_string()))
            .and_then(|project| project.construction.evaluate().map_err(|error| error.to_string()))
            .map(|snapshot| {
                let labels: Vec<_> = snapshot.iter().map(|(label, _)| label.to_string()).collect();
                if json { serde_json::json!({"objects": labels}).to_string() } else { labels.join("\n") }
            }),
        Command::Apply { project, json, operation } => fs::read_to_string(project)
            .map_err(|error| error.to_string())
            .and_then(|source| geodesic_io::load(&source).map_err(|error| error.to_string()))
            .and_then(|mut project| {
                apply_operation(&mut project, operation)?;
                let source = geodesic_io::save(&project);
                Ok(if json { serde_json::json!({"project": source}).to_string() } else { source })
            }),
    };
    match result {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("geodesic: {error}");
            ExitCode::FAILURE
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "Clap operations map directly to the shared command boundary."
)]
fn apply_operation(project: &mut geodesic_io::Project, operation: Operation) -> Result<(), String> {
    match operation {
        Operation::AddPoint { label, x, y } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddPoint {
                label: parse_label(&label)?,
                point: geodesic_core::ProjectivePoint::real_affine(x, y),
            },
        )
        .map_err(|error| error.to_string()),
        Operation::AddComplexPoint {
            label,
            x_re,
            x_im,
            y_re,
            y_im,
            z_re,
            z_im,
        } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddPoint {
                label: parse_label(&label)?,
                point: geodesic_core::ProjectivePoint::new(
                    num_complex::Complex64::new(x_re, x_im),
                    num_complex::Complex64::new(y_re, y_im),
                    num_complex::Complex64::new(z_re, z_im),
                )
                .map_err(|error| error.to_string())?,
            },
        )
        .map_err(|error| error.to_string()),
        Operation::AddLineThrough {
            label,
            first,
            second,
        } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddLineThrough {
                label: parse_label(&label)?,
                first: parse_label(&first)?,
                second: parse_label(&second)?,
            },
        )
        .map_err(|error| error.to_string()),
        Operation::MovePoint { label, x, y } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::MovePoint {
                label: parse_label(&label)?,
                point: geodesic_core::ProjectivePoint::real_affine(x, y),
            },
        )
        .map_err(|error| error.to_string()),
        Operation::MoveLine { label, x, y } => move_free_line(project, &label, x, y),
        Operation::AddIntersection {
            label,
            first,
            second,
        } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddIntersection {
                label: parse_label(&label)?,
                first: parse_label(&first)?,
                second: parse_label(&second)?,
            },
        )
        .map_err(|error| error.to_string()),
        Operation::AddParallelThrough { label, point, line } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddParallelThrough {
                label: parse_label(&label)?,
                point: parse_label(&point)?,
                line: parse_label(&line)?,
            },
        )
        .map_err(|error| error.to_string()),
        Operation::AddPerpendicularThrough { label, point, line } => geodesic_io::apply_command(
            project,
            geodesic_core::GeoCommand::AddPerpendicularThrough {
                label: parse_label(&label)?,
                point: parse_label(&point)?,
                line: parse_label(&line)?,
            },
        )
        .map_err(|error| error.to_string()),
        Operation::AddSegment { label, start, end } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddSegment {
                label: parse_label(&label)?,
                start: parse_label(&start)?,
                end: parse_label(&end)?,
            },
        ),
        Operation::AddRay {
            label,
            origin,
            through,
        } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddRay {
                label: parse_label(&label)?,
                origin: parse_label(&origin)?,
                through: parse_label(&through)?,
            },
        ),
        Operation::AddVector { label, start, end } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddVector {
                label: parse_label(&label)?,
                start: parse_label(&start)?,
                end: parse_label(&end)?,
            },
        ),
        Operation::AddMidpoint {
            label,
            first,
            second,
        } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddMidpoint {
                label: parse_label(&label)?,
                first: parse_label(&first)?,
                second: parse_label(&second)?,
            },
        ),
        Operation::AddPerpendicularBisector {
            label,
            first,
            second,
        } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddPerpendicularBisector {
                label: parse_label(&label)?,
                first: parse_label(&first)?,
                second: parse_label(&second)?,
            },
        ),
        Operation::AddAngleBisector {
            label,
            first,
            vertex,
            second,
        } => apply_relation(
            project,
            geodesic_core::GeoCommand::AddAngleBisector {
                label: parse_label(&label)?,
                first: parse_label(&first)?,
                vertex: parse_label(&vertex)?,
                second: parse_label(&second)?,
            },
        ),
        Operation::Undo => project
            .construction
            .undo()
            .map_err(|error| error.to_string()),
        Operation::Redo => project
            .construction
            .redo()
            .map_err(|error| error.to_string()),
    }
}

fn apply_relation(
    project: &mut geodesic_io::Project,
    command: geodesic_core::GeoCommand,
) -> Result<(), String> {
    geodesic_io::apply_command(project, command).map_err(|error| error.to_string())
}

fn move_free_line(
    project: &mut geodesic_io::Project,
    label: &str,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let label = parse_label(label)?;
    let line = match project
        .construction
        .evaluate()
        .map_err(|error| error.to_string())?
        .object(&label)
    {
        Some(geodesic_core::GeometricObject::Line(line)) => line,
        Some(
            geodesic_core::GeometricObject::Point(_)
            | geodesic_core::GeometricObject::Segment(_)
            | geodesic_core::GeometricObject::Ray(_)
            | geodesic_core::GeometricObject::Vector(_),
        ) => {
            return Err(format!("object `{label}` is not a line"));
        }
        None => return Err(format!("object label `{label}` does not exist")),
    };
    let line =
        geodesic_core::parallel_through(geodesic_core::ProjectivePoint::real_affine(x, y), line)
            .map_err(|error| error.to_string())?;
    geodesic_io::apply_command(project, geodesic_core::GeoCommand::MoveLine { label, line })
        .map_err(|error| error.to_string())
}

fn parse_label(value: &str) -> Result<geodesic_core::Label, String> {
    geodesic_core::Label::parse(value).map_err(|error| error.to_string())
}
