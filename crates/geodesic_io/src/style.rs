use std::collections::BTreeSet;

use geodesic_core::{GeoCommand, Label};

use crate::{ObjectStyle, Project, ProjectError, StyleBinding, StyleCategory};

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

/// Validates that every fallback target exists and named fallback chains are acyclic.
///
/// Object styles may reference only named styles. Category defaults likewise
/// reference named styles, so every resolved appearance remains complete.
pub(crate) fn validate_style_references(project: &Project) -> Result<(), ProjectError> {
    for binding in project.object_styles.values() {
        validate_binding_target(project, binding)?;
    }
    for preset in project.category_presets.values() {
        if !project.style_presets.contains_key(preset) {
            return Err(ProjectError::Invalid(format!(
                "unknown style preset `{preset}`"
            )));
        }
    }
    for label in project.style_presets.keys() {
        let mut visiting = BTreeSet::new();
        validate_preset_chain(project, label, &mut visiting)?;
    }
    Ok(())
}

fn validate_binding_target(project: &Project, binding: &StyleBinding) -> Result<(), ProjectError> {
    if let StyleBinding::Fallback(label) = binding
        && !project.style_presets.contains_key(label)
    {
        return Err(ProjectError::Invalid(format!(
            "unknown style preset `{label}`"
        )));
    }
    Ok(())
}

fn validate_preset_chain(
    project: &Project,
    label: &Label,
    visiting: &mut BTreeSet<Label>,
) -> Result<(), ProjectError> {
    if !visiting.insert(label.clone()) {
        return Err(ProjectError::Invalid(format!(
            "style fallback cycle at `{label}`"
        )));
    }
    let binding = project
        .style_presets
        .get(label)
        .ok_or_else(|| ProjectError::Invalid(format!("unknown style preset `{label}`")))?;
    if let StyleBinding::Fallback(target) = binding {
        validate_preset_chain(project, target, visiting)?;
    }
    visiting.remove(label);
    Ok(())
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
