use mfa_contracts::UtcInstant;
use mfa_ingestion::queue::{
    BoxFuture, ScanExecutor, ScanQueue, ScanReason, ScanReport, ScanRequest,
};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone)]
struct BlockingExecutor {
    runs: Arc<AtomicUsize>,
    started: Arc<Notify>,
    release: Arc<Notify>,
}

impl ScanExecutor for BlockingExecutor {
    fn execute(
        &self,
        _request: ScanRequest,
    ) -> BoxFuture<'_, Result<ScanReport, mfa_ingestion::IngestionError>> {
        Box::pin(async move {
            let run = self.runs.fetch_add(1, Ordering::SeqCst);
            self.started.notify_waiters();
            if run == 0 {
                self.release.notified().await;
            }
            Ok(ScanReport::default())
        })
    }
}

#[derive(Clone)]
struct FailOnceExecutor {
    runs: Arc<AtomicUsize>,
}

impl ScanExecutor for FailOnceExecutor {
    fn execute(
        &self,
        _request: ScanRequest,
    ) -> BoxFuture<'_, Result<ScanReport, mfa_ingestion::IngestionError>> {
        Box::pin(async move {
            if self.runs.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(mfa_ingestion::IngestionError::AssetFailure {
                    code: "synthetic_failure".to_owned(),
                    detail: "first scan failed".to_owned(),
                })
            } else {
                Ok(ScanReport::default())
            }
        })
    }
}

#[derive(Clone)]
struct CountingExecutor {
    runs: Arc<AtomicUsize>,
}

impl ScanExecutor for CountingExecutor {
    fn execute(
        &self,
        _request: ScanRequest,
    ) -> BoxFuture<'_, Result<ScanReport, mfa_ingestion::IngestionError>> {
        Box::pin(async move {
            self.runs.fetch_add(1, Ordering::SeqCst);
            Ok(ScanReport::default())
        })
    }
}

#[derive(Clone)]
struct RecordingExecutor {
    reasons: Arc<Mutex<Vec<ScanReason>>>,
    started: Arc<Notify>,
    release: Arc<Notify>,
}

impl ScanExecutor for RecordingExecutor {
    fn execute(
        &self,
        request: ScanRequest,
    ) -> BoxFuture<'_, Result<ScanReport, mfa_ingestion::IngestionError>> {
        Box::pin(async move {
            let run = {
                let mut reasons = self.reasons.lock().unwrap();
                let run = reasons.len();
                reasons.push(request.reason);
                run
            };
            self.started.notify_one();
            if run == 0 {
                self.release.notified().await;
            }
            Ok(ScanReport::default())
        })
    }
}

fn request() -> ScanRequest {
    ScanRequest::new(
        ScanReason::Manual,
        "2026-08-25T00:00:00Z".parse::<UtcInstant>().unwrap(),
    )
}

async fn yield_to_queue() {
    for _ in 0..4 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pending_scan_requests_coalesce_after_the_active_scan_and_queue_is_bounded() {
    let executor = BlockingExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
        started: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
    };
    let queue = ScanQueue::start(executor.clone(), 2);

    let first_queue = queue.clone();
    let first = tokio::spawn(async move { first_queue.request_scan(request()).await });
    executor.started.notified().await;

    let second_queue = queue.clone();
    let second = tokio::spawn(async move { second_queue.request_scan(request()).await });
    let third_queue = queue.clone();
    let third = tokio::spawn(async move { third_queue.request_scan(request()).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!second.is_finished() || !third.is_finished());

    executor.release.notify_one();
    let first_ticket = first.await.unwrap().unwrap();
    let second_ticket = second.await.unwrap().unwrap();
    let third_ticket = third.await.unwrap().unwrap();
    assert_ne!(first_ticket.scan_id, second_ticket.scan_id);
    assert_eq!(second_ticket.scan_id, third_ticket.scan_id);
    assert_eq!(executor.runs.load(Ordering::SeqCst), 2);
    queue.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_scan_failure_does_not_stop_later_scan_jobs() {
    let executor = FailOnceExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
    };
    let queue = ScanQueue::start(executor.clone(), 1);
    let first = queue.request_scan(request()).await;
    assert!(matches!(
        first,
        Err(mfa_ingestion::IngestionError::AssetFailure { .. })
    ));
    assert!(queue.request_scan(request()).await.is_ok());
    assert_eq!(executor.runs.load(Ordering::SeqCst), 2);
    queue.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_finishes_a_queued_scan_before_stopping() {
    let executor = BlockingExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
        started: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
    };
    let queue = ScanQueue::start(executor.clone(), 2);

    let first_queue = queue.clone();
    let first = tokio::spawn(async move { first_queue.request_scan(request()).await });
    executor.started.notified().await;

    let second_queue = queue.clone();
    let second = tokio::spawn(async move { second_queue.request_scan(request()).await });
    tokio::time::sleep(Duration::from_millis(10)).await;
    let shutdown_queue = queue.clone();
    let shutdown = tokio::spawn(async move { shutdown_queue.shutdown().await });
    tokio::time::sleep(Duration::from_millis(10)).await;

    executor.release.notify_one();
    assert!(first.await.unwrap().is_ok());
    assert!(second.await.unwrap().is_ok());
    assert!(shutdown.await.unwrap().is_ok());
    assert_eq!(executor.runs.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn periodic_scans_stop_after_queue_shutdown() {
    let executor = CountingExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
    };
    let queue = ScanQueue::start_periodic(executor.clone(), 2, Duration::from_millis(10));

    tokio::time::timeout(Duration::from_secs(1), async {
        while executor.runs.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();

    queue.shutdown().await.unwrap();
    let runs_after_shutdown = executor.runs.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(executor.runs.load(Ordering::SeqCst), runs_after_shutdown);
}

#[tokio::test(start_paused = true, flavor = "current_thread")]
async fn first_periodic_scan_waits_for_a_full_interval() {
    let interval = Duration::from_secs(5);
    let executor = CountingExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
    };
    let queue = ScanQueue::start_periodic(executor.clone(), 2, interval);

    yield_to_queue().await;
    assert_eq!(executor.runs.load(Ordering::SeqCst), 0);
    tokio::time::advance(interval - Duration::from_millis(1)).await;
    yield_to_queue().await;
    assert_eq!(executor.runs.load(Ordering::SeqCst), 0);

    tokio::time::advance(Duration::from_millis(1)).await;
    yield_to_queue().await;
    assert_eq!(executor.runs.load(Ordering::SeqCst), 1);
    queue.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true, flavor = "current_thread")]
async fn manual_follow_ups_coalesce_after_periodic_and_observe_manual_reason() {
    let interval = Duration::from_secs(5);
    let executor = RecordingExecutor {
        reasons: Arc::new(Mutex::new(Vec::new())),
        started: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
    };
    let queue = ScanQueue::start_periodic(executor.clone(), 2, interval);
    yield_to_queue().await;

    tokio::time::advance(interval).await;
    yield_to_queue().await;
    executor.started.notified().await;

    let first_queue = queue.clone();
    let first_manual = tokio::spawn(async move { first_queue.request_scan(request()).await });
    yield_to_queue().await;
    let second_queue = queue.clone();
    let second_manual = tokio::spawn(async move { second_queue.request_scan(request()).await });
    yield_to_queue().await;
    assert!(!first_manual.is_finished());
    assert!(!second_manual.is_finished());

    executor.release.notify_one();
    let first_ticket = first_manual.await.unwrap().unwrap();
    let second_ticket = second_manual.await.unwrap().unwrap();
    assert_eq!(first_ticket.scan_id, second_ticket.scan_id);
    assert_eq!(first_ticket.coalesced_requests, 1);
    assert_eq!(second_ticket.coalesced_requests, 1);
    assert_eq!(
        *executor.reasons.lock().unwrap(),
        vec![ScanReason::Periodic, ScanReason::Manual]
    );
    queue.shutdown().await.unwrap();
}

#[tokio::test(start_paused = true, flavor = "current_thread")]
async fn acknowledged_shutdown_stops_periodic_work_and_rejects_new_requests() {
    let interval = Duration::from_secs(5);
    let executor = CountingExecutor {
        runs: Arc::new(AtomicUsize::new(0)),
    };
    let queue = ScanQueue::start_periodic(executor.clone(), 2, interval);
    yield_to_queue().await;
    let request_queue = queue.clone();

    queue.shutdown().await.unwrap();
    assert_eq!(executor.runs.load(Ordering::SeqCst), 0);
    tokio::time::advance(interval * 2).await;
    yield_to_queue().await;
    assert_eq!(executor.runs.load(Ordering::SeqCst), 0);

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        request_queue.request_scan(request()),
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        Err(mfa_ingestion::IngestionError::QueueClosed)
    ));
}
