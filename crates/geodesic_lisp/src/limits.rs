/// Limits applied before dispatching a project script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptLimits {
    /// Maximum top-level commands.
    pub max_commands: usize,
    /// Maximum UTF-8 source bytes accepted for one script.
    pub max_source_bytes: usize,
    /// Maximum parenthesis nesting depth accepted in one script source.
    pub max_nesting_depth: usize,
    /// Maximum nested application-event dispatches on one thread.
    pub max_event_depth: usize,
}

impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            max_commands: 1_000,
            max_source_bytes: 65_536,
            max_nesting_depth: 64,
            max_event_depth: 8,
        }
    }
}

/// Failure while translating a restricted script form.
#[derive(Debug, thiserror::Error)]
pub enum ScriptError {
    #[error("script exceeds command limit")]
    Limit,
    #[error("script exceeds source-size limit")]
    SourceLimit,
    #[error("script exceeds nesting-depth limit")]
    NestingLimit,
    #[error("script has unbalanced parentheses")]
    UnbalancedSource,
    #[error("script event recursion exceeds limit")]
    EventDepthLimit,
    #[error("invalid script form: {0}")]
    Invalid(String),
    #[error(transparent)]
    Command(#[from] geodesic_core::CommandError),
    #[error("Steel evaluation failed: {0}")]
    Steel(String),
}
