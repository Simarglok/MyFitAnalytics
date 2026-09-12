use mfa_contracts::CanonicalObservation;
use mfa_source_mynetdiary::{
    MappingContext, SchemaProfile, SheetKind, map_activity, map_food, map_measurements,
    map_trackers, map_water, validate_workbook,
};
use std::path::Path;

fn fixture() -> Vec<u8> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/current-schema.xls"))
        .unwrap()
}

fn context() -> MappingContext {
    MappingContext::synthetic("current-schema-asset")
}

#[test]
fn current_profile_normalizes_known_headers_without_inventing_measurements() {
    let schema = validate_workbook(&fixture()).unwrap();
    assert_eq!(schema.profile, SchemaProfile::Current);
    assert_eq!(
        schema.sheets[&SheetKind::Food].profile,
        SchemaProfile::Current
    );

    let food = map_food(&schema.sheets[&SheetKind::Food], &context()).unwrap();
    let CanonicalObservation::NutritionItem(nutrition) = &food.records[0] else {
        panic!()
    };
    assert_eq!(nutrition.name, "Synthetic Meal");
    assert_eq!(nutrition.calories_kcal, Some(320.0));
    assert_eq!(
        nutrition.occurred_local_at.unwrap().to_string(),
        "2026-01-04T08:15:00"
    );

    let activity = map_activity(&schema.sheets[&SheetKind::Exercise], &context()).unwrap();
    let CanonicalObservation::ActivityEvent(activity) = &activity.records[0] else {
        panic!()
    };
    assert_eq!(
        activity.occurred_local_at.to_string(),
        "2026-01-04T18:30:00"
    );
    assert_eq!(activity.duration_seconds, None);
    assert_eq!(activity.distance_km, None);

    let measurements =
        map_measurements(&schema.sheets[&SheetKind::Measurements], &context()).unwrap();
    assert!(matches!(
        measurements.records[0],
        CanonicalObservation::ActivityDay(_)
    ));

    let trackers = map_trackers(schema.sheets.get(&SheetKind::Trackers), &context()).unwrap();
    let CanonicalObservation::HeartRate(heart_rate) = &trackers.records[0] else {
        panic!()
    };
    assert_eq!(
        heart_rate.observed_local_at.to_string(),
        "2026-01-04T07:30:00"
    );
    assert_eq!(heart_rate.heart_rate_bpm, 128.0);

    let water = map_water(schema.sheets.get(&SheetKind::WaterGlasses), &context()).unwrap();
    assert_eq!(water.extensions[0].payload["glasses"], 2.0);
}
