pub mod asset;
pub mod capability;
pub mod dashboard;
pub mod error;
pub mod locale;
pub mod module;
pub mod observation;

pub use asset::{AssetMetadata, ReadOnlyAsset};
pub use capability::CapabilityId;
pub use dashboard::{
    AvailabilityState, CoverageRule, DashboardBlock, DashboardCard, DashboardCardPresentation,
    DashboardChart, DashboardDocument, DashboardInput, DashboardRequirement, DashboardSeries,
    DashboardStatusPanel, DashboardSummaryValue, DashboardTable,
};
pub use error::{AssetReadError, ContractError};
pub use locale::{LocalDate, LocalDateTime, UtcInstant};
pub use module::{
    ContractVersion, DASHBOARD_API_VERSION, DashboardManifest, LOCALE_API_VERSION, LocaleFile,
    LocaleManifest, ModuleId, ModuleManifest, ModuleType, PACKAGE_FORMAT_VERSION,
    SOURCE_API_VERSION, SOURCE_BATCH_CONTRACT_VERSION, SourceExtensionContract, SourceManifest,
};
pub use observation::{
    ActivityDay, ActivityEvent, BodyMeasurement, CanonicalObservation, ExerciseSet,
    ExtensionRecord, ExtensionRequirement, HeartRateObservation, LineageHook, LocaleBundle,
    MappingIssue, NutritionItem, PhaseEvent, SourceBatch, SourceDescriptor, SourceRecord,
    SourceValidation, WorkoutSession,
};
