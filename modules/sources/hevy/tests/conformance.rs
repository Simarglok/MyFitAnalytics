use mfa_source_hevy::{CsvInput, ExerciseMapping, parse_measurements, parse_workouts};
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
fn measurement_and_workout_batches_are_deterministic_and_have_complete_lineage() {
    let measurements = CsvInput::new(fixture("measurement_data.csv"), "measurements");
    let measurement_context = mfa_source_hevy::context_for_measurements(&measurements).unwrap();
    let first_measurements =
        parse_measurements(measurements.clone(), &measurement_context).unwrap();
    let second_measurements = parse_measurements(measurements, &measurement_context).unwrap();
    assert_eq!(first_measurements, second_measurements);
    assert_eq!(first_measurements.source_module_id, "hevy");
    assert!(first_measurements.lineage.iter().all(|hook| {
        first_measurements
            .source_records
            .iter()
            .any(|record| record.source_record_key == hook.source_record_key)
    }));

    let workouts = CsvInput::new(fixture("workout_data.csv"), "workouts");
    let workout_context = mfa_source_hevy::context_for_workouts(&workouts).unwrap();
    let first_workouts = parse_workouts(
        workouts.clone(),
        &ExerciseMapping::default(),
        &workout_context,
    )
    .unwrap();
    let second_workouts =
        parse_workouts(workouts, &ExerciseMapping::default(), &workout_context).unwrap();
    assert_eq!(first_workouts, second_workouts);
    assert_eq!(first_workouts.records.len(), 5);
    assert_eq!(first_workouts.source_records.len(), 4);
    assert_eq!(first_workouts.lineage.len(), 5);
    assert!(first_workouts.issues.is_empty());
}
