use crate::window::DateRange;
use mfa_contracts::{LocalDate, PhaseEvent};
use std::collections::BTreeSet;

pub fn excluded_dates(window: DateRange, phases: &[PhaseEvent]) -> BTreeSet<LocalDate> {
    phases
        .iter()
        .filter(|phase| phase.exclude_from_tdee)
        .flat_map(|phase| {
            let start = phase.start_date.max(window.start);
            let end = phase.end_date.min(window.end);
            if start <= end {
                DateRange::inclusive(start, end).dates().collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        })
        .collect()
}
