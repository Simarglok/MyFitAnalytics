use mfa_contracts::CanonicalObservation;
use mfa_source_hevy::{
    CsvInput, ExerciseMapping, MappingContext, assign_exercise_blocks, group_sessions,
    parse_workout_rows, parse_workouts,
};
use std::path::Path;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn sessions_group_by_title_and_local_timestamps_and_preserve_source_order() {
    let rows =
        parse_workout_rows(CsvInput::new(fixture("workout_data.csv"), "workout-asset")).unwrap();
    let groups = group_sessions(rows).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].rows.len(), 4);
    assert_eq!(groups[0].rows[0].source_row_number, 2);
    assert_eq!(groups[0].rows[3].source_row_number, 5);
    assert_eq!(groups[0].duration_seconds(), Some(2520));
    assert_eq!(assign_exercise_blocks(&groups[0].rows), vec![1, 1, 2, 3]);
}

#[test]
fn reverse_chronological_sessions_are_normalized_without_reordering_rows_inside_a_session() {
    let input = b"title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps,rpe,duration_seconds,notes
Earlier,2026-02-01 10:00:00,2026-02-01 10:30:00,Plank,1,normal,,, ,30,
Later,2026-02-02 10:00:00,2026-02-02 10:30:00,Plank,1,normal,,, ,30,
Later,2026-02-02 10:00:00,2026-02-02 10:30:00,Push-Up,1,normal,,10,,,
Earlier,2026-02-01 10:00:00,2026-02-01 10:30:00,Plank,2,normal,,, ,30,
"
    .to_vec();
    let rows = parse_workout_rows(CsvInput::new(input, "reverse-asset")).unwrap();
    let groups = group_sessions(rows).unwrap();
    assert_eq!(groups[0].title, "Earlier");
    assert_eq!(groups[1].title, "Later");
    assert_eq!(groups[0].rows[0].source_row_number, 2);
    assert_eq!(groups[0].rows[1].source_row_number, 5);
}

#[test]
fn workouts_emit_sessions_sets_and_governed_load_types() {
    let batch = parse_workouts(
        CsvInput::new(fixture("workout_data.csv"), "workout-asset"),
        &ExerciseMapping::default(),
        &MappingContext::for_workouts("workout-asset", 2026, "sha256:test".to_owned()),
    )
    .unwrap();
    let sessions = batch
        .records
        .iter()
        .filter_map(|record| match record {
            CanonicalObservation::WorkoutSession(session) => Some(session),
            _ => None,
        })
        .collect::<Vec<_>>();
    let sets = batch
        .records
        .iter()
        .filter_map(|record| match record {
            CanonicalObservation::ExerciseSet(set) => Some(set),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].duration_seconds, Some(2520));
    assert_eq!(sets.len(), 4);
    assert_eq!(
        sets.iter()
            .map(|set| set.exercise_block_ordinal)
            .collect::<Vec<_>>(),
        vec![1, 1, 2, 3]
    );
    assert_eq!(sets[0].load_type, "external");
    assert_eq!(sets[1].set_type, "warmup");
    assert_eq!(sets[2].load_type, "duration");
    assert_eq!(sets[3].load_type, "external");
    assert_eq!(batch.source_records.len(), 4);
    assert_eq!(batch.lineage.len(), 5);
}

#[test]
fn unknown_exercise_and_set_type_are_visible_and_do_not_infer_load_type() {
    let input = b"title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps,rpe,duration_seconds,notes
Unknown,2026-02-03 10:00:00,2026-02-03 10:10:00,New Exercise,1,mystery,30,5,,,
"
    .to_vec();
    let batch = parse_workouts(
        CsvInput::new(input, "unknown-asset"),
        &ExerciseMapping::default(),
        &MappingContext::for_workouts("unknown-asset", 2026, "sha256:test".to_owned()),
    )
    .unwrap();
    let set = batch
        .records
        .iter()
        .find_map(|record| match record {
            CanonicalObservation::ExerciseSet(set) => Some(set),
            _ => None,
        })
        .unwrap();
    assert_eq!(set.load_type, "unknown");
    assert_eq!(set.weight_kg, Some(30.0));
    assert_eq!(set.set_type, "mystery");
    assert!(
        batch
            .issues
            .iter()
            .any(|issue| issue.code == "hevy.unknown_exercise")
    );
    assert!(
        batch
            .issues
            .iter()
            .any(|issue| issue.code == "hevy.unknown_set_type")
    );
}

#[test]
fn session_end_before_start_is_rejected() {
    let input = b"title,start_time,end_time,exercise_title,set_index,set_type,weight_kg,reps,rpe,duration_seconds,notes
Broken,2026-02-03 10:10:00,2026-02-03 10:00:00,Plank,1,normal,,,30,,
"
    .to_vec();
    let error = parse_workout_rows(CsvInput::new(input, "broken-asset")).unwrap_err();
    assert_eq!(error.code(), "hevy.invalid_session_time");
}
