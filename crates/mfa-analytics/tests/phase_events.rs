use mfa_analytics::{DateRange, excluded_dates};
use mfa_contracts::{LocalDate, PhaseEvent};
use std::str::FromStr;
use uuid::Uuid;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn event(id: u128, start: &str, end: &str, exclude_from_tdee: bool) -> PhaseEvent {
    PhaseEvent {
        phase_event_id: Uuid::from_u128(id),
        event_type: format!("phase-{id}"),
        start_date: date(start),
        end_date: date(end),
        description: Some(format!("description-{id}")),
        exclude_from_tdee,
    }
}

fn window() -> DateRange {
    DateRange::inclusive(date("2026-01-01"), date("2026-01-20"))
}

#[test]
fn exclusion_bounds_are_inclusive() {
    assert_eq!(
        excluded_dates(window(), &[event(1, "2026-01-10", "2026-01-12", true)])
            .into_iter()
            .collect::<Vec<_>>(),
        [date("2026-01-10"), date("2026-01-11"), date("2026-01-12"),]
    );
}

#[test]
fn overlapping_exclusions_are_a_deterministic_union() {
    assert_eq!(
        excluded_dates(
            window(),
            &[
                event(1, "2026-01-10", "2026-01-12", true),
                event(2, "2026-01-12", "2026-01-15", true),
                event(3, "2026-01-11", "2026-01-14", false),
            ],
        )
        .into_iter()
        .collect::<Vec<_>>(),
        (10..=15)
            .map(|day| date(&format!("2026-01-{day:02}")))
            .collect::<Vec<_>>()
    );
}

#[test]
fn non_excluding_and_out_of_range_events_do_not_exclude_dates() {
    assert!(
        excluded_dates(
            window(),
            &[
                event(1, "2025-12-01", "2025-12-31", true),
                event(2, "2026-01-05", "2026-01-07", false),
            ],
        )
        .is_empty()
    );
}

#[test]
fn exclusion_is_clipped_to_the_requested_window() {
    assert_eq!(
        excluded_dates(window(), &[event(1, "2025-12-25", "2026-01-03", true)])
            .into_iter()
            .collect::<Vec<_>>(),
        [date("2026-01-01"), date("2026-01-02"), date("2026-01-03"),]
    );
}
