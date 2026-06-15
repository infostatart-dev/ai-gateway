/// OpenAI-style messages split for web providers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedChat {
    pub system_msg: String,
    pub history: Vec<(String, String)>,
    pub current_msg: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageBudget {
    pub max_context_tokens: u32,
    pub reserved_output_tokens: u32,
    pub protocol_overhead_tokens: u32,
}

impl Default for MessageBudget {
    fn default() -> Self {
        Self {
            max_context_tokens: 131_072,
            reserved_output_tokens: 4_096,
            protocol_overhead_tokens: 2_048,
        }
    }
}

impl MessageBudget {
    #[must_use]
    pub fn input_token_budget(self) -> u32 {
        self.max_context_tokens
            .saturating_sub(self.reserved_output_tokens)
            .saturating_sub(self.protocol_overhead_tokens)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FitReport {
    pub dropped_history_turns: usize,
    pub trimmed_system: bool,
    pub trimmed_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebTurnKind {
    ContextUpload { part: usize, total: usize },
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebTurn {
    pub kind: WebTurnKind,
    pub system_msg: String,
    pub user_msg: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkPlan {
    pub turns: Vec<WebTurn>,
}
