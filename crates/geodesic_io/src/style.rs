use std::collections::BTreeSet;

use geodesic_core::{GeoCommand, Label};

use crate::{ObjectStyle, Project, StyleBinding, StyleCategory};

/// Returns the complete resolved style for an object.
///
/// Missing, cyclic, and stale fallback references safely resolve to the relevant
/// category default; mutation APIs should reject creating such references.
#[must_use]
pub fn effective_style(project: &Project, label: &Label) -> ObjectStyle {
    let category = project
        .construction
        .definitions()
        .find_map(|(candidate, definition)| {
            (candidate == label).then_some(StyleCategory::for_definition(definition))
        })
        .unwrap_or(StyleCategory::Point);
    let fallback = project.category_presets.get(&category);
    let binding = project
        .object_styles
        .get(label)
        .or_else(|| fallback.and_then(|preset| project.style_presets.get(preset)));
    resolve_binding(project, binding, &mut BTreeSet::new()).unwrap_or_default()
}

fn resolve_binding(
    project: &Project,
    binding: Option<&StyleBinding>,
    visited: &mut BTreeSet<Label>,
) -> Option<ObjectStyle> {
    match binding? {
        StyleBinding::Concrete(style) => Some(style.clone()),
        StyleBinding::Fallback(label) if visited.insert(label.clone()) => {
            resolve_binding(project, project.style_presets.get(label), visited)
        }
        StyleBinding::Fallback(_) => None,
    }
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
