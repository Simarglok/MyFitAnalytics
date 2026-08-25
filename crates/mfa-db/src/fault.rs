use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DatabaseFailurePoint {
    TransactionStart,
    CanonicalInsert,
    ActiveSwitch,
    ExtensionContractRegistration,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("injected database failure at {point:?}")]
pub struct DatabaseFault {
    pub point: DatabaseFailurePoint,
}

pub trait DatabaseFaultInjector: Send + Sync {
    fn check(&self, point: DatabaseFailurePoint) -> Result<(), DatabaseFault>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoDatabaseFaultInjector;

impl DatabaseFaultInjector for NoDatabaseFaultInjector {
    fn check(&self, _point: DatabaseFailurePoint) -> Result<(), DatabaseFault> {
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct TestDatabaseFaultInjector {
    remaining: Arc<Mutex<BTreeMap<DatabaseFailurePoint, u32>>>,
}

impl TestDatabaseFaultInjector {
    pub fn fail_once(&self, point: DatabaseFailurePoint) {
        self.remaining.lock().unwrap().insert(point, 1);
    }

    pub fn fail_always(&self, point: DatabaseFailurePoint) {
        self.remaining.lock().unwrap().insert(point, u32::MAX);
    }
}

impl DatabaseFaultInjector for TestDatabaseFaultInjector {
    fn check(&self, point: DatabaseFailurePoint) -> Result<(), DatabaseFault> {
        let mut remaining = self.remaining.lock().unwrap();
        let Some(budget) = remaining.get_mut(&point) else {
            return Ok(());
        };
        if *budget != u32::MAX {
            *budget = budget.saturating_sub(1);
            if *budget == 0 {
                remaining.remove(&point);
            }
        }
        Err(DatabaseFault { point })
    }
}
