use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudget {
    pub total_tokens: usize,
    pub task_tokens: usize,
    pub action_tokens: usize,
    pub evidence_tokens: usize,
    pub state_tokens: usize,
    pub response_tokens: usize,
}

impl ContextBudget {
    pub fn validate(self) -> Result<Self, ContextError> {
        let reserved = self.task_tokens
            + self.action_tokens
            + self.evidence_tokens
            + self.state_tokens
            + self.response_tokens;
        if reserved > self.total_tokens {
            return Err(ContextError::ReservedBudgetExceedsTotal);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub reference: String,
    pub summary: String,
    pub source: String,
    pub importance: i32,
    pub stale: bool,
}

impl EvidenceRecord {
    pub fn new(
        reference: impl Into<String>,
        summary: impl Into<String>,
        source: impl Into<String>,
        importance: i32,
    ) -> Self {
        Self {
            reference: reference.into(),
            summary: summary.into(),
            source: source.into(),
            importance,
            stale: false,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct EvidenceLedger {
    records: BTreeMap<String, EvidenceRecord>,
}

impl EvidenceLedger {
    pub fn record(&mut self, record: EvidenceRecord) {
        let replace = self
            .records
            .get(&record.reference)
            .is_none_or(|current| record.importance >= current.importance);
        if replace {
            self.records.insert(record.reference.clone(), record);
        }
    }

    pub fn mark_stale(&mut self, reference: &str) {
        if let Some(record) = self.records.get_mut(reference) {
            record.stale = true;
        }
    }

    pub fn active(&self) -> Vec<EvidenceRecord> {
        let mut records: Vec<_> = self
            .records
            .values()
            .filter(|record| !record.stale)
            .cloned()
            .collect();
        records.sort_by(|left, right| {
            right
                .importance
                .cmp(&left.importance)
                .then_with(|| left.reference.cmp(&right.reference))
        });
        records
    }

    pub fn snapshot(&self) -> Vec<EvidenceRecord> {
        self.records.values().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextPacket {
    pub task: String,
    pub action: String,
    pub evidence: Vec<EvidenceRecord>,
    pub state: String,
    pub estimated_tokens: usize,
    pub dropped_evidence: Vec<String>,
    pub slot_usage: BTreeMap<String, usize>,
}

impl ContextPacket {
    pub fn as_text(&self) -> String {
        let evidence = self
            .evidence
            .iter()
            .map(|item| {
                format!(
                    "[{}] {} (source: {})",
                    item.reference, item.summary, item.source
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "TASK:\n{}\n\nACTION:\n{}\n\nEVIDENCE:\n{}\n\nOMITTED_EVIDENCE:\n{:?}\n\nSTATE:\n{}",
            self.task, self.action, evidence, self.dropped_evidence, self.state
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextError {
    ReservedBudgetExceedsTotal,
    TaskExceedsBudget,
    ActionExceedsBudget,
    StateExceedsBudget,
    ResponseBudgetUnavailable,
}

#[derive(Debug)]
pub struct ContextAssembler {
    pub budget: ContextBudget,
}

impl ContextAssembler {
    pub fn new(budget: ContextBudget) -> Result<Self, ContextError> {
        Ok(Self {
            budget: budget.validate()?,
        })
    }

    pub fn estimate_tokens(value: &str) -> usize {
        value.len().saturating_add(3) / 4
    }

    pub fn assemble(
        &self,
        task: impl Into<String>,
        action: impl Into<String>,
        state: impl Into<String>,
        evidence: &[EvidenceRecord],
    ) -> Result<ContextPacket, ContextError> {
        let task = task.into();
        let action = action.into();
        let state = state.into();
        let task_tokens = Self::estimate_tokens(&task);
        let action_tokens = Self::estimate_tokens(&action);
        let state_tokens = Self::estimate_tokens(&state);
        if task_tokens > self.budget.task_tokens {
            return Err(ContextError::TaskExceedsBudget);
        }
        if action_tokens > self.budget.action_tokens {
            return Err(ContextError::ActionExceedsBudget);
        }
        if state_tokens > self.budget.state_tokens {
            return Err(ContextError::StateExceedsBudget);
        }

        let mut selected = Vec::new();
        let mut dropped = Vec::new();
        let mut used_evidence = 0;
        let mut ordered: Vec<_> = evidence.iter().filter(|item| !item.stale).collect();
        ordered.sort_by(|left, right| {
            right
                .importance
                .cmp(&left.importance)
                .then_with(|| left.reference.cmp(&right.reference))
        });
        for item in evidence.iter().filter(|item| item.stale) {
            dropped.push(item.reference.clone());
        }
        for item in ordered {
            let item_tokens = Self::estimate_tokens(&item.summary);
            if used_evidence + item_tokens > self.budget.evidence_tokens {
                dropped.push(item.reference.clone());
            } else {
                selected.push(item.clone());
                used_evidence += item_tokens;
            }
        }
        let estimated_tokens = task_tokens + action_tokens + state_tokens + used_evidence;
        if estimated_tokens
            > self
                .budget
                .total_tokens
                .saturating_sub(self.budget.response_tokens)
        {
            return Err(ContextError::ResponseBudgetUnavailable);
        }
        Ok(ContextPacket {
            task,
            action,
            evidence: selected,
            state,
            estimated_tokens,
            dropped_evidence: dropped,
            slot_usage: BTreeMap::from([
                ("task".into(), task_tokens),
                ("action".into(), action_tokens),
                ("evidence".into(), used_evidence),
                ("state".into(), state_tokens),
                ("response_reserved".into(), self.budget.response_tokens),
            ]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assembler() -> ContextAssembler {
        ContextAssembler::new(ContextBudget {
            total_tokens: 100,
            task_tokens: 10,
            action_tokens: 10,
            evidence_tokens: 40,
            state_tokens: 10,
            response_tokens: 20,
        })
        .unwrap()
    }

    #[test]
    fn ledger_replaces_weaker_evidence_and_drops_stale_records() {
        let mut ledger = EvidenceLedger::default();
        ledger.record(EvidenceRecord::new("e1", "weak", "a.py", 1));
        ledger.record(EvidenceRecord::new("e1", "strong", "a.py:4", 3));
        ledger.record(EvidenceRecord::new("old", "stale", "stdout", 5));
        ledger.mark_stale("old");
        assert_eq!(ledger.active()[0].summary, "strong");
        assert!(
            ledger
                .snapshot()
                .iter()
                .any(|item| item.reference == "old" && item.stale)
        );
    }

    #[test]
    fn assembler_preserves_sources_and_reports_omissions() {
        let mut stale = EvidenceRecord::new("stale", "old output", "stdout", 5);
        stale.stale = true;
        let packet = assembler()
            .assemble(
                "fix pagination",
                "return a bounded plan",
                "phase=PLAN",
                &[
                    EvidenceRecord::new("high", "relevant source", "src/pagination.py:4", 3),
                    stale,
                ],
            )
            .unwrap();
        assert_eq!(packet.evidence[0].source, "src/pagination.py:4");
        assert_eq!(packet.dropped_evidence, vec!["stale"]);
        assert!(packet.as_text().contains("OMITTED_EVIDENCE"));
    }

    #[test]
    fn budget_rejects_missing_response_reserve() {
        let error = ContextAssembler::new(ContextBudget {
            total_tokens: 10,
            task_tokens: 8,
            action_tokens: 2,
            evidence_tokens: 1,
            state_tokens: 1,
            response_tokens: 1,
        })
        .unwrap_err();
        assert_eq!(error, ContextError::ReservedBudgetExceedsTotal);
    }
}
