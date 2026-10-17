use std::cell::Cell;

use geodesic_core::Construction;
use geodesic_io::Project;

use crate::{ScriptError, ScriptLimits, dispatch};

/// Dispatches scripts assigned to an application event in document order.
///
/// # Errors
///
/// Returns the first failure after preserving commands committed by prior forms.
pub fn dispatch_event(
    project: &Project,
    event: &str,
    construction: &mut Construction,
    limits: ScriptLimits,
) -> Result<usize, ScriptError> {
    let _depth = EventDepthGuard::enter(limits.max_event_depth)?;
    let mut applied = 0;
    for script in project
        .scripts
        .iter()
        .filter(|script| script.event == event)
    {
        let max_commands = limits
            .max_commands
            .checked_sub(applied)
            .ok_or(ScriptError::Limit)?;
        applied += dispatch(
            &script.source,
            construction,
            ScriptLimits {
                max_commands,
                ..limits
            },
        )?;
    }
    Ok(applied)
}

thread_local! { static EVENT_DEPTH: Cell<usize> = const { Cell::new(0) }; }

struct EventDepthGuard;

impl EventDepthGuard {
    fn enter(maximum: usize) -> Result<Self, ScriptError> {
        EVENT_DEPTH.with(|depth| {
            let current = depth.get();
            if current >= maximum {
                Err(ScriptError::EventDepthLimit)
            } else {
                depth.set(current + 1);
                Ok(Self)
            }
        })
    }
}

impl Drop for EventDepthGuard {
    fn drop(&mut self) {
        EVENT_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}
