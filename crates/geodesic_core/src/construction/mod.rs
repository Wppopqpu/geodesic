mod apply;
mod command;
mod evaluate;
mod object;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

pub use command::GeoCommand;
pub use object::{GeometricObject, ObjectDefinition};

use crate::Label;

/// The ordered, labeled graph of construction definitions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Construction {
    pub(super) definitions: BTreeMap<Label, ObjectDefinition>,
    pub(super) undo_stack: Vec<BTreeMap<Label, ObjectDefinition>>,
    pub(super) redo_stack: Vec<BTreeMap<Label, ObjectDefinition>>,
}

/// An immutable evaluated view used by renderers and adapters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeometrySnapshot {
    pub(super) objects: BTreeMap<Label, GeometricObject>,
}

impl Construction {
    /// Creates an empty construction.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether a successful command is available to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns whether a reverted command is available to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Iterates over definitions in deterministic label order.
    pub fn definitions(&self) -> impl Iterator<Item = (&Label, &ObjectDefinition)> {
        self.definitions.iter()
    }
}
