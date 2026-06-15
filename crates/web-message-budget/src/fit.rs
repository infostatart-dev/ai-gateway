use crate::{
    token::{estimate_tokens, trim_tail_tokens},
    types::{FitReport, MessageBudget, ParsedChat},
};

const HISTORY_TURN_OVERHEAD_TOKENS: usize = 24;

/// Fit messages into provider input budget: keep system + current, drop oldest history.
#[must_use]
pub fn fit_parsed(parsed: &mut ParsedChat, budget: MessageBudget) -> FitReport {
    let mut report = FitReport::default();
    let input_budget = budget.input_token_budget() as usize;
    if input_budget == 0 {
        parsed.history.clear();
        parsed.system_msg.clear();
        parsed.current_msg = trim_tail_tokens(&parsed.current_msg, 1);
        report.trimmed_current = true;
        return report;
    }

    let max_current = input_budget * 65 / 100;
    if estimate_tokens(&parsed.current_msg) > max_current {
        parsed.current_msg = trim_tail_tokens(&parsed.current_msg, max_current);
        report.trimmed_current = true;
    }

    let mut remaining = input_budget
        .saturating_sub(estimate_tokens(&parsed.current_msg))
        .saturating_sub(estimate_tokens(&parsed.system_msg));

    while !parsed.history.is_empty()
        && history_tokens(&parsed.history) > remaining
    {
        parsed.history.remove(0);
        report.dropped_history_turns += 1;
    }

    if history_tokens(&parsed.history) > remaining {
        parsed.history.clear();
    }
    remaining = remaining.saturating_sub(history_tokens(&parsed.history));

    if remaining == 0 && !parsed.system_msg.is_empty() {
        let cap = (input_budget / 10).max(256);
        if estimate_tokens(&parsed.system_msg) > cap {
            parsed.system_msg = trim_tail_tokens(&parsed.system_msg, cap);
            report.trimmed_system = true;
        }
    }

    shrink_until_within(parsed, input_budget, &mut report);
    report
}

fn history_tokens(history: &[(String, String)]) -> usize {
    history
        .iter()
        .map(|(role, content)| {
            estimate_tokens(content)
                + estimate_tokens(role)
                + HISTORY_TURN_OVERHEAD_TOKENS
        })
        .sum()
}

fn total_tokens(parsed: &ParsedChat) -> usize {
    estimate_tokens(&parsed.system_msg)
        + estimate_tokens(&parsed.current_msg)
        + history_tokens(&parsed.history)
}

fn shrink_until_within(
    parsed: &mut ParsedChat,
    budget: usize,
    report: &mut FitReport,
) {
    while total_tokens(parsed) > budget {
        if !parsed.history.is_empty() {
            parsed.history.remove(0);
            report.dropped_history_turns += 1;
            continue;
        }
        if estimate_tokens(&parsed.current_msg) > budget / 2 {
            parsed.current_msg = trim_tail_tokens(
                &parsed.current_msg,
                budget.saturating_sub(256).max(1),
            );
            report.trimmed_current = true;
            break;
        }
        if !parsed.system_msg.is_empty() {
            parsed.system_msg = trim_tail_tokens(&parsed.system_msg, 256);
            report.trimmed_system = true;
        }
        break;
    }
}
