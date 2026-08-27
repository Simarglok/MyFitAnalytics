use crate::error::IngestionError;
pub use mfa_archive::{ScanReason, ScanRequest};
use mfa_contracts::UtcInstant;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{self, MissedTickBehavior};
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
    Periodic(ScanRequest),
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
        Self::start_with_periodic(executor, capacity, None)
    }

    pub fn start_periodic<E>(executor: E, capacity: usize, interval: Duration) -> Self
    where
        E: ScanExecutor,
    {
        assert!(
            !interval.is_zero(),
            "periodic scan interval must be positive"
        );
        Self::start_with_periodic(executor, capacity, Some(interval))
    }

    fn start_with_periodic<E>(
        executor: E,
        capacity: usize,
        periodic_interval: Option<Duration>,
    ) -> Self
    where
        E: ScanExecutor,
    {
        assert!(capacity > 0, "scan queue capacity must be positive");
        let (sender, receiver) = mpsc::channel(capacity);
        tokio::spawn(run_queue(receiver, Arc::new(executor), periodic_interval));
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

async fn run_queue<E>(
    mut receiver: mpsc::Receiver<QueueCommand>,
    executor: Arc<E>,
    periodic_interval: Option<Duration>,
) where
    E: ScanExecutor,
{
    let mut ticker = periodic_interval.map(|interval| {
        let mut ticker = time::interval_at(time::Instant::now() + interval, interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        ticker
    });
    loop {
        let command = match ticker.as_mut() {
            Some(ticker) => tokio::select! {
                command = receiver.recv() => command,
                _ = ticker.tick() => Some(QueueCommand::Periodic(now_request(ScanReason::Periodic))),
            },
            None => receiver.recv().await,
        };
        let Some(command) = command else {
            break;
        };
        let pending = match command {
            QueueCommand::Scan { request, response } => PendingScan {
                request,
                responses: vec![response],
            },
            QueueCommand::Periodic(request) => PendingScan {
                request,
                responses: Vec::new(),
            },
            QueueCommand::Shutdown(response) => {
                let _ = response.send(Ok(()));
                break;
            }
        };
        if run_pending(pending, &mut receiver, Arc::clone(&executor)).await {
            break;
        }
    }
}

async fn run_pending<E>(
    mut pending: PendingScan,
    receiver: &mut mpsc::Receiver<QueueCommand>,
    executor: Arc<E>,
) -> bool
where
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
            QueueCommand::Periodic(request) => {
                if pending.request.reason != ScanReason::Manual {
                    pending.request = request;
                }
            }
            QueueCommand::Shutdown(response) => {
                let _ = response.send(Ok(()));
                for waiter in pending.responses {
                    let _ = waiter.send(Err(IngestionError::QueueClosed));
                }
                return true;
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
    false
}

pub fn now_request(reason: ScanReason) -> ScanRequest {
    ScanRequest::new(reason, UtcInstant::from(chrono::Utc::now()))
}
