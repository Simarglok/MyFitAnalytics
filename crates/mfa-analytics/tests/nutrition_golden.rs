use mfa_analytics::{
    AlgorithmVersion, DateRange, MetricContext, NutritionQuality, SnapshotRef, nutrition_analytics,
};
use mfa_contracts::{LocalDate, NutritionItem};
use std::collections::BTreeSet;
use std::str::FromStr;
use uuid::Uuid;

fn date(value: &str) -> LocalDate {
    LocalDate::from_str(value).unwrap()
}

fn context() -> MetricContext {
    MetricContext {
        requested: DateRange::inclusive(date("2026-01-01"), date("2026-01-04")),
        as_of: date("2026-01-04"),
        snapshot_refs: vec![SnapshotRef {
            logical_snapshot_key: "mynetdiary:2026".to_owned(),
            snapshot_id: "snapshot-nutrition".to_owned(),
        }],
        algorithm_version: AlgorithmVersion::new("nutrition.daily@1"),
    }
}

fn item(
    id: u128,
    local_date: &str,
    calories_kcal: Option<f64>,
    protein_g: Option<f64>,
    fiber_g: Option<f64>,
) -> NutritionItem {
    NutritionItem {
        nutrition_item_id: Uuid::from_u128(id),
        occurred_local_at: None,
        local_date: date(local_date),
        meal: "Lunch".to_owned(),
        food_source_id: "food-1".to_owned(),
        name: "Synthetic food".to_owned(),
        amount_raw: "1 serving".to_owned(),
        calories_kcal,
        protein_g,
        fat_g: Some(10.0),
        carbs_g: Some(20.0),
        fiber_g,
        sugars_g: None,
        sodium_mg: None,
        source_record_id: Some(format!("record-{id}")),
    }
}

#[test]
fn nutrition_analytics_preserves_quality_gaps_nulls_and_row_multiplicity() {
    let mut excluded = BTreeSet::new();
    excluded.insert(date("2026-01-04"));
    let result = nutrition_analytics(
        &context(),
        &[
            item(1, "2026-01-01", Some(500.0), Some(20.0), None),
            item(2, "2026-01-01", Some(500.0), Some(20.0), Some(4.0)),
            item(3, "2026-01-02", None, Some(10.0), Some(2.0)),
            item(4, "2026-01-04", Some(300.0), Some(15.0), Some(3.0)),
            item(5, "2026-01-08", Some(999.0), Some(99.0), Some(9.0)),
        ],
        &excluded,
    );

    assert_eq!(result.days.len(), 4);
    assert_eq!(result.days[0].logged_item_count, 2);
    assert_eq!(result.days[0].quality, NutritionQuality::Complete);
    assert_eq!(result.days[0].calories_kcal, Some(1000.0));
    assert_eq!(result.days[0].protein_g, Some(40.0));
    assert_eq!(result.days[0].fiber_g, None);

    assert_eq!(result.days[1].quality, NutritionQuality::PartialFields);
    assert_eq!(result.days[1].calories_kcal, None);
    assert_eq!(result.days[1].protein_g, Some(10.0));

    assert_eq!(result.days[2].quality, NutritionQuality::Missing);
    assert_eq!(result.days[2].calories_kcal, None);
    assert_eq!(result.days[2].logged_item_count, 0);

    assert_eq!(result.days[3].quality, NutritionQuality::ExcludedByUser);
    assert_eq!(result.days[3].calories_kcal, Some(300.0));
    assert_eq!(result.days[3].logged_item_count, 1);

    assert_eq!(result.trailing_7d_mean_calories.len(), 4);
    assert_eq!(result.trailing_7d_mean_calories[0].value, Some(1000.0));
    assert_eq!(result.trailing_7d_mean_calories[1].value, Some(1000.0));
    assert_eq!(result.trailing_7d_mean_calories[2].value, Some(1000.0));
    assert_eq!(result.trailing_7d_mean_calories[3].value, Some(1000.0));
    assert_eq!(
        result.provenance.snapshot_refs[0].logical_snapshot_key,
        "mynetdiary:2026"
    );
}

#[test]
fn nutrition_analytics_returns_null_rolling_mean_when_no_complete_day_is_available() {
    let mut excluded = BTreeSet::new();
    excluded.insert(date("2026-01-01"));
    let result = nutrition_analytics(
        &context(),
        &[item(6, "2026-01-02", None, Some(10.0), Some(2.0))],
        &excluded,
    );

    assert_eq!(result.days[0].quality, NutritionQuality::ExcludedByUser);
    assert_eq!(result.days[1].quality, NutritionQuality::PartialFields);
    assert_eq!(result.trailing_7d_mean_calories[0].value, None);
    assert_eq!(result.trailing_7d_mean_calories[3].value, None);
}
