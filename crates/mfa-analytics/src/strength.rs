use crate::provenance::{DerivedProvenance, MetricContext};
use chrono::Datelike;
use mfa_contracts::{ExerciseSet, LocalDate, WorkoutSession};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

const GOVERNED_EXTERNAL_EXERCISE_KEYS: &[&str] = &["bench press"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowCounts {
    pub seven_day: u32,
    pub fourteen_day: u32,
    pub twenty_eight_day: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDuration {
    pub session_id: Uuid,
    pub local_date: LocalDate,
    pub duration_seconds: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingSet {
    pub exercise_key: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct E1rmPoint {
    pub week_start: LocalDate,
    pub exercise_key: String,
    pub value_kg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrengthAnalytics {
    pub session_counts: WindowCounts,
    pub session_durations: Vec<SessionDuration>,
    pub working_sets: Vec<WorkingSet>,
    pub weekly_best_e1rm: Vec<E1rmPoint>,
    pub provenance: DerivedProvenance,
}

pub fn strength_analytics(
    context: &MetricContext,
    sessions: &[WorkoutSession],
    sets: &[ExerciseSet],
) -> StrengthAnalytics {
    let mut session_dates = BTreeMap::new();
    let mut session_durations = Vec::new();
    for session in sessions.iter().filter(|session| {
        context
            .requested
            .contains(session.started_local_at.0.date().into())
    }) {
        let local_date: LocalDate = session.started_local_at.0.date().into();
        session_dates.insert(session.workout_session_id, local_date);
        session_durations.push(SessionDuration {
            session_id: session.workout_session_id,
            local_date,
            duration_seconds: session.duration_seconds,
        });
    }
    session_durations.sort_by_key(|duration| (duration.local_date, duration.session_id));

    let session_counts = WindowCounts {
        seven_day: count_sessions_in_window(context.as_of, &session_dates, 7),
        fourteen_day: count_sessions_in_window(context.as_of, &session_dates, 14),
        twenty_eight_day: count_sessions_in_window(context.as_of, &session_dates, 28),
    };

    let mut working_sets = BTreeMap::<String, u32>::new();
    let mut e1rm_by_week = BTreeMap::<(LocalDate, String), f64>::new();
    for set in sets {
        let Some(&local_date) = session_dates.get(&set.workout_session_id) else {
            continue;
        };
        let set_type = set.set_type.trim().to_ascii_lowercase();
        if !matches!(set_type.as_str(), "normal" | "failure") || set.load_type != "external" {
            continue;
        }
        let exercise_key = set.exercise_key.trim().to_ascii_lowercase();
        let (Some(weight_kg), Some(reps)) = (set.weight_kg, set.reps) else {
            continue;
        };
        if exercise_key.is_empty()
            || !GOVERNED_EXTERNAL_EXERCISE_KEYS.contains(&exercise_key.as_str())
            || !weight_kg.is_finite()
            || weight_kg <= 0.0
            || !(1..=12).contains(&reps)
        {
            continue;
        }
        *working_sets.entry(exercise_key.clone()).or_default() += 1;
        let week_start = week_start(local_date);
        let e1rm = weight_kg * (1.0 + reps as f64 / 30.0);
        e1rm_by_week
            .entry((week_start, exercise_key))
            .and_modify(|best| *best = best.max(e1rm))
            .or_insert(e1rm);
    }

    let weekly_best_e1rm = e1rm_by_week
        .into_iter()
        .map(|((week_start, exercise_key), value_kg)| E1rmPoint {
            week_start,
            exercise_key,
            value_kg,
        })
        .collect();
    let working_sets = working_sets
        .into_iter()
        .map(|(exercise_key, count)| WorkingSet {
            exercise_key,
            count,
        })
        .collect();

    StrengthAnalytics {
        session_counts,
        session_durations,
        working_sets,
        weekly_best_e1rm,
        provenance: context.provenance(session_dates.len()),
    }
}

fn count_sessions_in_window(
    as_of: LocalDate,
    session_dates: &BTreeMap<Uuid, LocalDate>,
    days: i64,
) -> u32 {
    let start = as_of
        .0
        .checked_sub_days(chrono::Days::new((days - 1) as u64))
        .expect("session window starts at a valid local date");
    session_dates
        .values()
        .filter(|date| date.0 >= start && **date <= as_of)
        .count() as u32
}

fn week_start(local_date: LocalDate) -> LocalDate {
    LocalDate::from(
        local_date
            .0
            .checked_sub_days(chrono::Days::new(
                local_date.0.weekday().num_days_from_monday() as u64,
            ))
            .expect("week starts at a valid local date"),
    )
}
