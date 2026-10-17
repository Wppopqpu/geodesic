//! Versioned S-expression persistence and read-only `GeoGebra` import reporting.
#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    io::{Cursor, Read},
};

use geodesic_core::{
    Construction, GeoCommand, Label, ObjectDefinition, ProjectiveLine, ProjectivePoint,
};

/// Current native document schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// A complete persisted project document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    /// The geometric construction.
    pub construction: Construction,
    /// Explicit application-owned script sections. They are data until dispatched.
    pub scripts: Vec<Script>,
    /// Direct visual overrides indexed by geometric-object label.
    pub object_styles: BTreeMap<Label, ObjectStyle>,
    /// Named reusable point/line appearance definitions.
    pub style_presets: BTreeMap<Label, ObjectStyle>,
    /// Optional named style preset used by each geometric object.
    pub object_presets: BTreeMap<Label, Label>,
}

/// Persisted visual properties shared by point and line renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectStyle {
    /// CSS-compatible RGB hex color.
    pub color: String,
    /// Point radius or line width in SVG units.
    pub size: f64,
    /// Opacity in the closed range zero through one.
    pub opacity: f64,
}

impl Default for ObjectStyle {
    fn default() -> Self {
        Self {
            color: "#1565c0".to_owned(),
            size: 0.15,
            opacity: 1.0,
        }
    }
}

/// A script attached to a named application event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    /// Event name, for example `on_open`.
    pub event: String,
    /// Steel source code.
    pub source: String,
}

/// A read-only representation of an unsupported `GeoGebra` object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompatibilityPlaceholder {
    /// Source object label.
    pub label: String,
    /// Source object type.
    pub source_type: String,
    /// Human-readable source definition summary.
    pub definition_summary: String,
    /// Known source dependencies.
    pub dependencies: Vec<String>,
}

/// Structured results of a lossy `.ggb` import.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompatibilityReport {
    /// Number of natively converted objects.
    pub converted: usize,
    /// Objects retained as read-only placeholders.
    pub placeholders: Vec<CompatibilityPlaceholder>,
    /// Legacy `GGBScript` is always ignored by this release.
    pub ignored_ggb_script: bool,
}

/// Persistence failure.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// The S-expression source cannot be parsed or validated.
    #[error("invalid project document: {0}")]
    Invalid(String),
    /// A validated command could not be applied.
    #[error(transparent)]
    Command(#[from] geodesic_core::CommandError),
    /// A coordinate cannot be represented as a finite number.
    #[error("invalid numeric coordinate `{0}`")]
    Number(String),
    /// The `.ggb` archive is malformed or has no construction XML.
    #[error("invalid .ggb archive: {0}")]
    Ggb(String),
}

/// Imports point-and-line data from a read-only `.ggb` archive.
///
/// Unsupported objects become [`CompatibilityPlaceholder`] values. Legacy
/// `GGBScript` is detected but never executed or retained.
///
/// # Errors
/// Returns an error for malformed ZIP/XML input or invalid supported objects.
pub fn import_ggb(bytes: &[u8]) -> Result<(Project, CompatibilityReport), ProjectError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ProjectError::Ggb(error.to_string()))?;
    let mut xml = String::new();
    archive
        .by_name("geogebra.xml")
        .map_err(|error| ProjectError::Ggb(error.to_string()))?
        .read_to_string(&mut xml)
        .map_err(|error| ProjectError::Ggb(error.to_string()))?;
    let document =
        roxmltree::Document::parse(&xml).map_err(|error| ProjectError::Ggb(error.to_string()))?;
    let mut project = Project::default();
    let mut report = CompatibilityReport {
        ignored_ggb_script: document
            .descendants()
            .any(|node| node.tag_name().name().eq_ignore_ascii_case("script")),
        ..CompatibilityReport::default()
    };
    for element in document
        .descendants()
        .filter(|node| node.has_tag_name("element"))
    {
        let source_type = element.attribute("type").unwrap_or("unknown");
        let source_label = element.attribute("label").unwrap_or("unnamed");
        let label = source_label.to_ascii_lowercase();
        let coordinates = element
            .descendants()
            .find(|node| node.has_tag_name("coords"));
        let converted = match (source_type, coordinates, Label::parse(&label)) {
            ("point", Some(coords), Ok(label)) => {
                let x = xml_number(&coords, "x")?;
                let y = xml_number(&coords, "y")?;
                let z = xml_number(&coords, "z")?;
                project.construction.apply(GeoCommand::AddPoint {
                    label,
                    point: ProjectivePoint::new(x.into(), y.into(), z.into())
                        .map_err(|error| ProjectError::Invalid(error.to_string()))?,
                })?;
                true
            }
            ("line", Some(coords), Ok(label)) => {
                let x = xml_number(&coords, "x")?;
                let y = xml_number(&coords, "y")?;
                let z = xml_number(&coords, "z")?;
                project.construction.apply(GeoCommand::AddLine {
                    label,
                    line: ProjectiveLine::new(x.into(), y.into(), z.into())
                        .map_err(|error| ProjectError::Invalid(error.to_string()))?,
                })?;
                true
            }
            _ => false,
        };
        if converted {
            report.converted += 1;
        } else {
            report.placeholders.push(CompatibilityPlaceholder {
                label: source_label.to_owned(),
                source_type: source_type.to_owned(),
                definition_summary: element
                    .attribute("definition")
                    .unwrap_or("unsupported object")
                    .to_owned(),
                dependencies: Vec::new(),
            });
        }
    }
    Ok((project, report))
}

fn xml_number(node: &roxmltree::Node<'_, '_>, attribute: &str) -> Result<f64, ProjectError> {
    node.attribute(attribute)
        .ok_or_else(|| ProjectError::Ggb(format!("missing coordinate `{attribute}`")))?
        .parse()
        .map_err(|_| ProjectError::Number(attribute.to_owned()))
}

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
        let form = list(form, "project form")?;
        let form = form.as_slice();
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
                let label = parse_label(form, 1)?;
                project.object_styles.insert(label, parse_style(form)?);
            }
            "style_preset" if form.len() == 5 => {
                let label = parse_label(form, 1)?;
                project.style_presets.insert(label, parse_style(form)?);
            }
            "style_use" if form.len() == 3 => {
                let object = parse_label(form, 1)?;
                let preset = parse_label(form, 2)?;
                project.object_presets.insert(object, preset);
            }
            other => {
                return Err(ProjectError::Invalid(format!(
                    "unknown or malformed form `{other}`"
                )));
            }
        }
    }
    Ok(project)
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

/// Returns the effective style; an object's direct style overrides its preset.
#[must_use]
pub fn effective_style(project: &Project, label: &Label) -> ObjectStyle {
    project
        .object_styles
        .get(label)
        .cloned()
        .or_else(|| {
            project
                .object_presets
                .get(label)
                .and_then(|preset| project.style_presets.get(preset))
                .cloned()
        })
        .unwrap_or_default()
}

/// Applies a validated core command to a complete project.
///
/// # Errors
///
/// Returns command validation failures without changing the project.
pub fn apply_command(
    project: &mut Project,
    command: GeoCommand,
) -> Result<(), geodesic_core::CommandError> {
    project.construction.apply(command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    #[test]
    fn round_trips_without_executing_scripts() {
        let source = "(geodesic 1 (point point_a 0 0) (point point_b 1 1) (line_through diagonal point_a point_b) (script \"on_open\" \"(geo::point late 4 2)\"))";
        let project = load(source).expect("valid data document");
        assert_eq!(project.scripts.len(), 1);
        assert_eq!(
            project
                .construction
                .evaluate()
                .expect("script was not executed")
                .iter()
                .count(),
            3
        );
        assert_eq!(
            load(&save(&project)).expect("saved project is valid"),
            project
        );
    }

    #[test]
    fn preserves_non_real_projective_coordinates() {
        let source = "(geodesic 1 (point_complex complex_point 1 2 3 4 1 0))";
        let project = load(source).expect("valid complex point");
        let saved = save(&project);
        assert!(saved.contains("point_complex complex_point 1 2 3 4 1 0"));
        assert_eq!(load(&saved).expect("complex project round trips"), project);
    }

    #[test]
    fn imports_supported_objects_and_reports_unsupported_ones() {
        let xml = r#"<geogebra><construction><element type="point" label="point_a"><coords x="1" y="2" z="1"/></element><element type="circle" label="circle_a" definition="Circle"/><script/></construction></geogebra>"#;
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut archive = zip::ZipWriter::new(&mut bytes);
            archive
                .start_file("geogebra.xml", zip::write::SimpleFileOptions::default())
                .expect("entry");
            archive.write_all(xml.as_bytes()).expect("xml");
            archive.finish().expect("archive");
        }
        let (project, report) = import_ggb(bytes.get_ref()).expect("import");
        assert_eq!(
            project
                .construction
                .evaluate()
                .expect("valid")
                .iter()
                .count(),
            1
        );
        assert_eq!(report.converted, 1);
        assert_eq!(report.placeholders[0].source_type, "circle");
        assert!(report.ignored_ggb_script);
    }

    #[test]
    fn persists_validated_direct_and_named_styles() {
        let project = load("(geodesic 1 (point point_a 0 0) (style point_a \"#ff0000\" 0.3 0.5) (style_preset emphasis \"#00ff00\" 0.2 1))").expect("project");
        assert_eq!(
            project.object_styles[&Label::parse("point_a").expect("label")].color,
            "#ff0000"
        );
        assert_eq!(load(&save(&project)).expect("round trip"), project);
    }

    #[test]
    fn resolves_presets_and_prefers_direct_overrides() {
        let project = load("(geodesic 1 (point point_a 0 0) (style_preset emphasis \"#00ff00\" 0.2 1) (style_use point_a emphasis) (style point_a \"#ff0000\" 0.3 0.5))").expect("project");
        assert_eq!(
            effective_style(&project, &Label::parse("point_a").expect("label")).color,
            "#ff0000"
        );
        assert_eq!(load(&save(&project)).expect("round trip"), project);
    }
}
