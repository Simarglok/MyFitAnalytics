use mfa_contracts::{CapabilityId, ModuleId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkState {
    Healthy,
    Working,
    Attention,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreEvent {
    WorkStateChanged(WorkState),
    Stage(&'static str),
    DataChanged {
        capabilities: Vec<CapabilityId>,
        dashboards: Vec<ModuleId>,
    },
    QualityChanged,
}
