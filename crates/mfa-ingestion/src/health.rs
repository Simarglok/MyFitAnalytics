use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Healthy,
    Working,
    Attention,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshot {
    pub state: HealthState,
    pub working_jobs: u64,
    pub waiting_assets: u64,
    pub attention_items: u64,
    pub critical_items: u64,
    pub failure_code_counts: BTreeMap<String, u64>,
}

impl HealthSnapshot {
    pub fn from_counts(
        working_jobs: u64,
        waiting_assets: u64,
        attention_items: u64,
        critical_items: u64,
    ) -> Self {
        Self::from_counts_with_failure_codes(
            working_jobs,
            waiting_assets,
            attention_items,
            critical_items,
            BTreeMap::new(),
        )
    }

    pub fn from_counts_with_failure_codes(
        working_jobs: u64,
        waiting_assets: u64,
        attention_items: u64,
        critical_items: u64,
        failure_code_counts: BTreeMap<String, u64>,
    ) -> Self {
        let state = if critical_items > 0 {
            HealthState::Blocked
        } else if working_jobs > 0 {
            HealthState::Working
        } else if waiting_assets > 0 || attention_items > 0 {
            HealthState::Attention
        } else {
            HealthState::Healthy
        };
        Self {
            state,
            working_jobs,
            waiting_assets,
            attention_items,
            critical_items,
            failure_code_counts,
        }
    }
}
