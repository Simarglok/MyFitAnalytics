use crate::error::IngestionError;
pub use mfa_archive::{ScanReason, ScanRequest};
use mfa_contracts::UtcInstant;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub completed_assets: u64,
    pub duplicate_assets: u64,
    pub failed_assets: u64,
}

pub trait ScanExecutor: Send + Sync + 'static {
    fn execute(&self, request: ScanRequest) -> BoxFuture<'_, Result<ScanReport, IngestionError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTicket {
    pub scan_id: Uuid,
    pub coalesced_requests: u32,
}

impl ScanTicket {
    pub fn new(scan_id: Uuid, coalesced_requests: u32) -> Self {
        Self {
            scan_id,
            coalesced_requests,
        }
    }
}

enum QueueCommand {
    Scan {
        request: ScanRequest,
        response: oneshot::Sender<Result<ScanTicket, IngestionError>>,
    },
    Shutdown(oneshot::Sender<Result<(), IngestionError>>),
}

struct PendingScan {
    request: ScanRequest,
    responses: Vec<oneshot::Sender<Result<ScanTicket, IngestionError>>>,
}

#[derive(Clone)]
pub struct ScanQueue {
    sender: mpsc::Sender<QueueCommand>,
}

impl ScanQueue {
    pub fn start<E>(executor: E, capacity: usize) -> Self
    where
        E: ScanExecutor,
    {
        assert!(capacity > 0, "scan queue capacity must be positive");
        let (sender, receiver) = mpsc::channel(capacity);
        tokio::spawn(run_queue(receiver, Arc::new(executor)));
        Self { sender }
    }

    pub async fn request_scan(&self, request: ScanRequest) -> Result<ScanTicket, IngestionError> {
        let (response, result) = oneshot::channel();
        self.sender
            .send(QueueCommand::Scan { request, response })
            .await
            .map_err(|_| IngestionError::QueueClosed)?;
        result.await.map_err(|_| IngestionError::QueueClosed)?
    }

    pub async fn shutdown(self) -> Result<(), IngestionError> {
        let (response, result) = oneshot::channel();
        self.sender
            .send(QueueCommand::Shutdown(response))
            .await
            .map_err(|_| IngestionError::QueueClosed)?;
        result.await.map_err(|_| IngestionError::QueueClosed)?
    }
}

async fn run_queue<E>(mut receiver: mpsc::Receiver<QueueCommand>, executor: Arc<E>)
where
    E: ScanExecutor,
{
    while let Some(command) = receiver.recv().await {
        let pending = match command {
            QueueCommand::Scan { request, response } => PendingScan {
                request,
                responses: vec![response],
            },
            QueueCommand::Shutdown(response) => {
                let _ = response.send(Ok(()));
                break;
            }
        };
        run_pending(pending, &mut receiver, Arc::clone(&executor)).await;
    }
}

async fn run_pending<E>(
    mut pending: PendingScan,
    receiver: &mut mpsc::Receiver<QueueCommand>,
    executor: Arc<E>,
) where
    E: ScanExecutor,
{
    // Requests that arrived while the previous job was active are drained here,
    // producing one follow-up scan instead of one job per trigger.
    while let Ok(command) = receiver.try_recv() {
        match command {
            QueueCommand::Scan { request, response } => {
                pending.responses.push(response);
                if request.reason == ScanReason::Manual {
                    pending.request = request;
                }
            }
            QueueCommand::Shutdown(response) => {
                let _ = response.send(Ok(()));
                break;
            }
        }
    }
    let scan_id = Uuid::new_v4();
    let response = executor
        .execute(pending.request)
        .await
        .map(|_| ScanTicket::new(scan_id, pending.responses.len().saturating_sub(1) as u32));
    for waiter in pending.responses {
        let _ = waiter.send(response.clone());
    }
}

pub fn now_request(reason: ScanReason) -> ScanRequest {
    ScanRequest::new(reason, UtcInstant::from(chrono::Utc::now()))
}
