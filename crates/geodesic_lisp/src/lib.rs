//! Restricted script command adapter.
#![forbid(unsafe_code)]

mod dispatch;
mod event;
mod limits;
mod steel;

#[cfg(test)]
mod tests;

pub use dispatch::dispatch;
pub use event::dispatch_event;
pub use limits::{ScriptError, ScriptLimits};
