use geodesic_core::{GeoCommand, Label};

use crate::{ObjectStyle, Project};

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
