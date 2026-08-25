pub mod error;
pub mod events;
pub mod pipeline;
pub mod queue;

pub use error::IngestionError;
pub use events::{CoreEvent, WorkState};
pub use pipeline::{IngestionCoordinator, IngestionDependencies, SourceInvoker};
pub use queue::{
    BoxFuture, ScanExecutor, ScanQueue, ScanReason, ScanReport, ScanRequest, ScanTicket,
};
