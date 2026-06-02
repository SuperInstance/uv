//! Conservation law enforcement.
//!
//! Three resource conservation laws modeled after the paradigm:
//! disk, bandwidth, and time budgets with utilization tracking.

use serde::{Deserialize, Serialize};

/// Which conservation law this check pertains to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConservationLaw {
    /// Disk conservation — cache must not exceed disk budget.
    Disk,
    /// Bandwidth conservation — daily downloads must not exceed bandwidth budget.
    Bandwidth,
    /// Time conservation — per-CI-run install time must not exceed time budget.
    Time,
}

impl ConservationLaw {
    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disk => "Disk Conservation",
            Self::Bandwidth => "Bandwidth Conservation",
            Self::Time => "Time Conservation",
        }
    }

    /// Short symbol.
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Disk => "💾",
            Self::Bandwidth => "🌐",
            Self::Time => "⏱",
        }
    }
}

/// Result of a conservation law check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConservationResult {
    /// Which law was checked.
    pub law: ConservationLaw,
    /// Whether the law is satisfied (budget not exceeded).
    pub satisfied: bool,
    /// Current value.
    pub current: f64,
    /// Maximum allowed.
    pub limit: f64,
    /// Utilization as a percentage (0–100).
    pub utilization_pct: f64,
    /// Human-readable message.
    pub message: String,
}

impl ConservationResult {
    pub fn new(
        law: ConservationLaw,
        satisfied: bool,
        current: f64,
        limit: f64,
    ) -> Self {
        let utilization_pct = if limit > 0.0 {
            (current / limit) * 100.0
        } else {
            100.0
        };

        let message = if satisfied {
            format!(
                "{} ✓ {} — {:.1}% used ({:.1} / {:.1})",
                law.symbol(),
                law.label(),
                utilization_pct,
                current,
                limit,
            )
        } else {
            format!(
                "{} ✗ {} EXCEEDED — {:.1}% used ({:.1} / {:.1})",
                law.symbol(),
                law.label(),
                utilization_pct,
                current,
                limit,
            )
        };

        Self {
            law,
            satisfied,
            current,
            limit,
            utilization_pct,
            message,
        }
    }
}

/// Conservation checker that evaluates all three laws.
#[derive(Debug, Default)]
pub struct ConservationChecker;

impl ConservationChecker {
    /// Check disk conservation: is cache size ≤ disk budget?
    pub fn check_disk(cache_size_bytes: u64, max_bytes: u64) -> ConservationResult {
        ConservationResult::new(
            ConservationLaw::Disk,
            cache_size_bytes <= max_bytes,
            cache_size_bytes as f64,
            max_bytes as f64,
        )
    }

    /// Check bandwidth conservation: is daily download ≤ daily budget?
    pub fn check_bandwidth(bytes_today: u64, max_bytes_per_day: u64) -> ConservationResult {
        ConservationResult::new(
            ConservationLaw::Bandwidth,
            bytes_today <= max_bytes_per_day,
            bytes_today as f64,
            max_bytes_per_day as f64,
        )
    }

    /// Check time conservation: is elapsed time ≤ time budget?
    pub fn check_time(elapsed_seconds: u64, max_seconds: u64) -> ConservationResult {
        ConservationResult::new(
            ConservationLaw::Time,
            elapsed_seconds <= max_seconds,
            elapsed_seconds as f64,
            max_seconds as f64,
        )
    }

    /// Run all three conservation checks and return results.
    pub fn check_all(
        cache_size_bytes: u64,
        max_bytes: u64,
        bytes_today: u64,
        max_bytes_per_day: u64,
        elapsed_seconds: u64,
        max_seconds: u64,
    ) -> Vec<ConservationResult> {
        vec![
            Self::check_disk(cache_size_bytes, max_bytes),
            Self::check_bandwidth(bytes_today, max_bytes_per_day),
            Self::check_time(elapsed_seconds, max_seconds),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conservation_law_labels() {
        assert_eq!(ConservationLaw::Disk.label(), "Disk Conservation");
        assert_eq!(ConservationLaw::Bandwidth.label(), "Bandwidth Conservation");
        assert_eq!(ConservationLaw::Time.label(), "Time Conservation");
    }

    #[test]
    fn test_check_disk_satisfied() {
        let result = ConservationChecker::check_disk(1_000_000_000, 5_000_000_000);
        assert!(result.satisfied);
        assert_eq!(result.utilization_pct, 20.0);
    }

    #[test]
    fn test_check_disk_exceeded() {
        let result = ConservationChecker::check_disk(6_000_000_000, 5_000_000_000);
        assert!(!result.satisfied);
        assert_eq!(result.utilization_pct, 120.0);
    }

    #[test]
    fn test_check_bandwidth_satisfied() {
        let result = ConservationChecker::check_bandwidth(500_000_000, 10_000_000_000);
        assert!(result.satisfied);
        assert_eq!(result.utilization_pct, 5.0);
    }

    #[test]
    fn test_check_bandwidth_exceeded() {
        let result = ConservationChecker::check_bandwidth(15_000_000_000, 10_000_000_000);
        assert!(!result.satisfied);
        assert_eq!(result.utilization_pct, 150.0);
    }

    #[test]
    fn test_check_time_satisfied() {
        let result = ConservationChecker::check_time(120, 300);
        assert!(result.satisfied);
        assert_eq!(result.utilization_pct, 40.0);
    }

    #[test]
    fn test_check_time_exceeded() {
        let result = ConservationChecker::check_time(350, 300);
        assert!(!result.satisfied);
        assert_eq!(result.utilization_pct, 116.66666666666667);
    }

    #[test]
    fn test_check_all() {
        let results = ConservationChecker::check_all(
            5_000_000_000,
            5_000_000_000,
            8_000_000_000,
            10_000_000_000,
            300,
            300,
        );
        assert_eq!(results.len(), 3);
        assert!(results[0].satisfied);
        assert!(results[1].satisfied);
        assert!(results[2].satisfied);
    }

    #[test]
    fn test_conservation_result_message_satisfied() {
        let r = ConservationResult::new(ConservationLaw::Disk, true, 2.0, 5.0);
        assert!(r.message.contains('✓'));
        assert!(r.message.contains("40.0%"));
    }

    #[test]
    fn test_conservation_result_message_exceeded() {
        let r = ConservationResult::new(ConservationLaw::Bandwidth, false, 20.0, 10.0);
        assert!(r.message.contains('✗'));
        assert!(r.message.contains("200.0%"));
    }
}
