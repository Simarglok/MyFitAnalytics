use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimits {
    pub max_memory_bytes: usize,
    pub fuel: u64,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024,
            fuel: 10_000_000,
            timeout: Duration::from_secs(2),
            max_output_bytes: 1024 * 1024,
        }
    }
}

impl RuntimeLimits {
    /// Finite limits for trusted local source imports. Source batches include
    /// immutable raw rows, so real annual exports need more headroom than a
    /// dashboard document while retaining a bounded execution budget.
    pub fn source_import_default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024,
            fuel: 3_000_000_000,
            timeout: Duration::from_secs(30),
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeLimits;
    use std::time::Duration;

    #[test]
    fn source_import_limits_remain_finite_and_do_not_change_dashboard_defaults() {
        assert_eq!(RuntimeLimits::default().timeout, Duration::from_secs(2));
        assert_eq!(RuntimeLimits::default().max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(
            RuntimeLimits::source_import_default().timeout,
            Duration::from_secs(30)
        );
        assert_eq!(
            RuntimeLimits::source_import_default().max_memory_bytes,
            512 * 1024 * 1024
        );
        assert_eq!(RuntimeLimits::source_import_default().fuel, 3_000_000_000);
        assert_eq!(
            RuntimeLimits::source_import_default().max_output_bytes,
            16 * 1024 * 1024
        );
    }
}
