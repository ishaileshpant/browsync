use std::time::{Duration, Instant};

use browsync_core::models::Browser;

/// Manages sync scheduling with debounce and periodic full syncs
pub struct SyncScheduler {
    /// Minimum time between syncs for the same browser (debounce)
    debounce: Duration,
    /// Time between periodic full syncs
    periodic_interval: Duration,
    /// Last sync time per browser
    last_sync: std::collections::HashMap<Browser, Instant>,
    /// Last periodic sync
    last_periodic: Instant,
    /// Total syncs performed
    pub total_syncs: u64,
}

impl SyncScheduler {
    pub fn new(debounce_secs: u64, periodic_mins: u64) -> Self {
        Self {
            debounce: Duration::from_secs(debounce_secs),
            periodic_interval: Duration::from_secs(periodic_mins * 60),
            last_sync: std::collections::HashMap::new(),
            last_periodic: Instant::now(),
            total_syncs: 0,
        }
    }

    /// Check if a sync should be triggered for this browser
    pub fn should_sync(&self, browser: Browser) -> bool {
        match self.last_sync.get(&browser) {
            Some(last) => last.elapsed() >= self.debounce,
            None => true,
        }
    }

    /// Record that a sync was performed
    pub fn record_sync(&mut self, browser: Browser) {
        self.last_sync.insert(browser, Instant::now());
        self.total_syncs += 1;
    }

    /// Check if periodic sync is due
    pub fn periodic_due(&self) -> bool {
        self.last_periodic.elapsed() >= self.periodic_interval
    }

    /// Record periodic sync
    pub fn record_periodic(&mut self) {
        self.last_periodic = Instant::now();
    }

    /// Get time until next periodic sync
    pub fn time_until_periodic(&self) -> Duration {
        let elapsed = self.last_periodic.elapsed();
        if elapsed >= self.periodic_interval {
            Duration::ZERO
        } else {
            self.periodic_interval - elapsed
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_initial_state() {
        let sched = SyncScheduler::new(5, 30);
        assert!(sched.should_sync(Browser::Chrome));
        assert_eq!(sched.total_syncs, 0);
    }

    #[test]
    fn test_scheduler_debounce() {
        let mut sched = SyncScheduler::new(5, 30);

        // First sync should be allowed
        assert!(sched.should_sync(Browser::Chrome));
        sched.record_sync(Browser::Chrome);

        // Immediate second sync should be debounced
        assert!(!sched.should_sync(Browser::Chrome));

        // Different browser should still be allowed
        assert!(sched.should_sync(Browser::Firefox));

        assert_eq!(sched.total_syncs, 1);
    }

    #[test]
    fn test_scheduler_periodic() {
        let sched = SyncScheduler::new(5, 0); // 0 min = always due
        assert!(sched.periodic_due());
    }
}
