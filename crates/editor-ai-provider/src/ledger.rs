//! Aggregate token usage for a session (M23 can display totals).

use crate::types::Usage;

#[derive(Debug, Default, Clone)]
pub struct TokenLedger {
    total: Usage,
}

impl TokenLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, u: Usage) {
        self.total = self.total.saturating_add(u);
    }

    pub fn total(&self) -> Usage {
        self.total
    }

    pub fn reset(&mut self) {
        self.total = Usage::ZERO;
    }
}
