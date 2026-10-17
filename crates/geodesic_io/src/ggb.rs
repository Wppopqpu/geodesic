use std::io::{Cursor, Read};

use geodesic_core::{GeoCommand, Label, ProjectiveLine, ProjectivePoint};

use crate::{CompatibilityPlaceholder, CompatibilityReport, Project, ProjectError};

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
        let label = source_label;
        let coordinates = element
            .descendants()
            .find(|node| node.has_tag_name("coords"));
        let converted = match (source_type, coordinates, Label::parse(label)) {
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
    for command in document
        .descendants()
        .filter(|node| node.has_tag_name("command"))
    {
        if let Some((label, command)) = bounded_command(&command)
            && project.construction.apply(command).is_ok()
        {
            report.converted += 1;
            report
                .placeholders
                .retain(|placeholder| placeholder.label != label.to_string());
        }
    }
    Ok((project, report))
}

fn bounded_command(node: &roxmltree::Node<'_, '_>) -> Option<(Label, GeoCommand)> {
    let input = node.children().find(|child| child.has_tag_name("input"))?;
    let output = node.children().find(|child| child.has_tag_name("output"))?;
    let label = xml_label(output.attribute("a0")?)?;
    let first = xml_label(input.attribute("a0")?)?;
    let second = xml_label(input.attribute("a1")?)?;
    let command = match node.attribute("name")? {
        "Segment" => GeoCommand::AddSegment {
            label: label.clone(),
            start: first,
            end: second,
        },
        "Ray" => GeoCommand::AddRay {
            label: label.clone(),
            origin: first,
            through: second,
        },
        "Vector" => GeoCommand::AddVector {
            label: label.clone(),
            start: first,
            end: second,
        },
        "Midpoint" => GeoCommand::AddMidpoint {
            label: label.clone(),
            first,
            second,
        },
        "LineBisector" => GeoCommand::AddPerpendicularBisector {
            label: label.clone(),
            first,
            second,
        },
        _ => return None,
    };
    Some((label, command))
}

fn xml_label(value: &str) -> Option<Label> {
    Label::parse(value).ok()
}

fn xml_number(node: &roxmltree::Node<'_, '_>, attribute: &str) -> Result<f64, ProjectError> {
    node.attribute(attribute)
        .ok_or_else(|| ProjectError::Ggb(format!("missing coordinate `{attribute}`")))?
        .parse()
        .map_err(|_| ProjectError::Number(attribute.to_owned()))
}
