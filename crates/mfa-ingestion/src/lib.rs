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
    IngestionCoordinator, IngestionDependencies, RetryResult, RuntimeArchiveImporter, SourceInvoker,
};
pub use queue::{
    BoxFuture, ScanExecutor, ScanQueue, ScanReason, ScanReport, ScanRequest, ScanTicket,
    now_request,
};
pub use rebuild::{
    ArchiveAssetImporter, ArchiveRebuildConfig, ArchiveRebuildConfirmation, ArchiveRebuildResult,
    ArchiveRebuildService, RebuildConfig, RebuildConfirmation, RebuildError, RebuildPlan,
    RebuildResult, RebuildService,
};
pub use recovery::{
    FailurePoint, FaultInjector, InjectedFailure, NoFaultInjector, RecoveryError, RecoveryGate,
    RecoveryMode, RecoveryReport, RecoveryService, TestFaultInjector, recover_sources,
};
pub use retry::{FailureClass, RetryClock, RetryPolicy, TokioRetryClock, retry_with_policy};
