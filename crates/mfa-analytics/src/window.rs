use mfa_contracts::LocalDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateRange {
    pub start: LocalDate,
    pub end: LocalDate,
}

impl DateRange {
    pub fn inclusive(start: LocalDate, end: LocalDate) -> Self {
        assert!(start <= end, "date range start must not be after end");
        Self { start, end }
    }

    pub fn contains(&self, date: LocalDate) -> bool {
        self.start <= date && date <= self.end
    }

    pub fn len_days(&self) -> u64 {
        (self.end.0 - self.start.0).num_days() as u64 + 1
    }

    pub fn dates(&self) -> impl Iterator<Item = LocalDate> {
        let mut current = self.start.0;
        let end = self.end.0;
        std::iter::from_fn(move || {
            if current > end {
                return None;
            }
            let result = LocalDate::from(current);
            current += chrono::Duration::days(1);
            Some(result)
        })
    }

    pub fn trailing_ending(&self, end: LocalDate, days: i64) -> Option<Self> {
        if days <= 0 {
            return None;
        }
        let start = LocalDate::from(
            end.0
                .checked_sub_days(chrono::Days::new((days as u64).saturating_sub(1)))?,
        );
        Some(Self::inclusive(start, end))
    }
}
