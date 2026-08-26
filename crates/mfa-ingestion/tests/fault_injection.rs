use mfa_ingestion::{
    FailureClass, FailurePoint, FaultInjector, RetryClock, RetryPolicy, TestFaultInjector,
    retry_with_policy,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn transient_retry_policy_stops_after_three_attempts_and_uses_increasing_delays() {
    let policy = RetryPolicy {
        max_transient_attempts: 3,
        delays: [
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(40),
        ],
    };

    assert!(policy.should_retry(FailureClass::TransientFailure, 1));
    assert_eq!(policy.delay_for(1), Duration::from_millis(10));
    assert!(policy.should_retry(FailureClass::TransientFailure, 2));
    assert_eq!(policy.delay_for(2), Duration::from_millis(20));
    assert!(!policy.should_retry(FailureClass::TransientFailure, 3));
    assert_eq!(policy.delay_for(3), Duration::from_millis(40));
    assert!(!policy.should_retry(FailureClass::AssetFailure, 1));
    assert!(!policy.should_retry(FailureClass::CriticalFailure, 1));
    assert!(!policy.should_retry(FailureClass::Waiting, 1));
}

#[tokio::test]
async fn transient_operation_uses_test_clock_and_never_exceeds_three_attempts() {
    let sleeps = Arc::new(Mutex::new(Vec::new()));
    let clock = RecordingClock {
        sleeps: Arc::clone(&sleeps),
    };
    let attempts = Arc::new(Mutex::new(0_u8));
    let policy = RetryPolicy {
        max_transient_attempts: 9,
        delays: [
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(40),
        ],
    };

    let result = retry_with_policy(&policy, &clock, {
        let attempts = Arc::clone(&attempts);
        move |_| {
            let attempts = Arc::clone(&attempts);
            async move {
                let mut count = attempts.lock().unwrap();
                *count += 1;
                if *count < 3 {
                    Err(FailureClass::TransientFailure)
                } else {
                    Ok("ready")
                }
            }
        }
    })
    .await;

    assert_eq!(result.unwrap(), "ready");
    assert_eq!(*attempts.lock().unwrap(), 3);
    assert_eq!(
        *sleeps.lock().unwrap(),
        vec![Duration::from_millis(10), Duration::from_millis(20)]
    );
}

struct RecordingClock {
    sleeps: Arc<Mutex<Vec<Duration>>>,
}

impl RetryClock for RecordingClock {
    fn sleep<'a>(&'a self, duration: Duration) -> mfa_ingestion::BoxFuture<'a, ()> {
        self.sleeps.lock().unwrap().push(duration);
        Box::pin(async {})
    }
}

#[test]
fn injected_faults_are_explicit_one_shot_controls_for_each_crash_window() {
    let injector = TestFaultInjector::default();
    injector.fail_once(FailurePoint::ActiveSwitch);
    injector.fail_always(FailurePoint::EventEmission);

    assert!(injector.check(FailurePoint::ActiveSwitch).is_err());
    assert!(injector.check(FailurePoint::ActiveSwitch).is_ok());
    assert!(injector.check(FailurePoint::EventEmission).is_err());
    assert!(injector.check(FailurePoint::EventEmission).is_err());
    assert!(injector.check(FailurePoint::CanonicalInsert).is_ok());
}
