pub mod error;
pub mod events;
pub mod health;
pub mod pipeline;
pub mod queue;
pub mod rebuild;
pub mod recovery;
pub mod retry;

pub use error::IngestionError;
pub use events::{CoreEvent, WorkState};
pub use health::{HealthSnapshot, HealthState};
pub use pipeline::{
    IngestionCoordinator, IngestionDependencies, RuntimeArchiveImporter, SourceInvoker,
};
pub use queue::{
    BoxFuture, ScanExecutor, ScanQueue, ScanReason, ScanReport, ScanRequest, ScanTicket,
};
pub use rebuild::{
    ArchiveAssetImporter, ArchiveRebuildConfig, ArchiveRebuildConfirmation, ArchiveRebuildResult,
    ArchiveRebuildService, RebuildConfig, RebuildConfirmation, RebuildError, RebuildPlan,
    RebuildResult, RebuildService,
};
pub use recovery::{
    FailurePoint, FaultInjector, InjectedFailure, NoFaultInjector, RecoveryError, RecoveryGate,
    RecoveryMode, RecoveryReport, RecoveryService, TestFaultInjector,
};
pub use retry::{FailureClass, RetryClock, RetryPolicy, TokioRetryClock, retry_with_policy};
