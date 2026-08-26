use mfa_ingestion::{HealthSnapshot, HealthState};

#[test]
fn aggregate_health_prioritizes_blocked_then_working_then_attention() {
    assert_eq!(
        HealthSnapshot::from_counts(0, 0, 0, 0).state,
        HealthState::Healthy
    );
    assert_eq!(
        HealthSnapshot::from_counts(0, 1, 0, 0).state,
        HealthState::Attention
    );
    assert_eq!(
        HealthSnapshot::from_counts(1, 0, 0, 0).state,
        HealthState::Working
    );
    assert_eq!(
        HealthSnapshot::from_counts(1, 1, 1, 0).state,
        HealthState::Working
    );
    assert_eq!(
        HealthSnapshot::from_counts(0, 0, 1, 1).state,
        HealthState::Blocked
    );
}
