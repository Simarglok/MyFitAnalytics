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
