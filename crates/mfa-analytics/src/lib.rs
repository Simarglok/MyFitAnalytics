pub mod activity;
pub mod coverage;
pub mod nutrition;
pub mod phase;
pub mod provenance;
pub mod strength;
pub mod tdee;
pub mod weight;
pub mod window;

pub use coverage::TdeeCoverage;
pub use phase::excluded_dates;

pub use activity::{ActivityAnalytics, ActivitySummary, activity_analytics};
pub use nutrition::{NutritionAnalytics, NutritionDay, NutritionQuality, nutrition_analytics};
pub use provenance::{
    AlgorithmVersion, CoverageEvidence, DerivedProvenance, MetricContext, SnapshotRef,
};
pub use strength::{
    E1rmPoint, SessionDuration, StrengthAnalytics, WindowCounts, WorkingSet, strength_analytics,
};
pub use tdee::{TdeeEstimate, TdeeResult, rolling_tdee, rolling_tdee_with_context};
pub use weight::{
    NullablePoint, TheilSenEstimate, WeightAnalytics, WeightObservation, WeightPoint,
    weight_analytics,
};
pub use window::DateRange;
