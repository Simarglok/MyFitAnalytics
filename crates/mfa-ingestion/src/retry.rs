use std::future::Future;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    Waiting,
    AssetFailure,
    TransientFailure,
    CriticalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_transient_attempts: u8,
    pub delays: [Duration; 3],
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_transient_attempts: 3,
            delays: [
                Duration::from_secs(1),
                Duration::from_secs(5),
                Duration::from_secs(30),
            ],
        }
    }
}

impl RetryPolicy {
    pub fn should_retry(&self, class: FailureClass, completed_attempts: u8) -> bool {
        matches!(class, FailureClass::TransientFailure)
            && completed_attempts < self.max_transient_attempts.min(3)
    }

    pub fn delay_for(&self, attempt: u8) -> Duration {
        let index = attempt.saturating_sub(1).min(2) as usize;
        self.delays[index]
    }
}

pub trait RetryClock: Send + Sync {
    fn sleep<'a>(&'a self, duration: Duration) -> crate::queue::BoxFuture<'a, ()>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRetryClock;

impl RetryClock for TokioRetryClock {
    fn sleep<'a>(&'a self, duration: Duration) -> crate::queue::BoxFuture<'a, ()> {
        Box::pin(tokio::time::sleep(duration))
    }
}

pub async fn retry_with_policy<T, F, Fut, C>(
    policy: &RetryPolicy,
    clock: &C,
    mut operation: F,
) -> Result<T, FailureClass>
where
    F: FnMut(u8) -> Fut,
    Fut: Future<Output = Result<T, FailureClass>>,
    C: RetryClock,
{
    let attempts = policy.max_transient_attempts.clamp(1, 3);
    for attempt in 1..=attempts {
        match operation(attempt).await {
            Ok(value) => return Ok(value),
            Err(class) if policy.should_retry(class, attempt) => {
                clock.sleep(policy.delay_for(attempt)).await;
            }
            Err(class) => return Err(class),
        }
    }
    Err(FailureClass::TransientFailure)
}
