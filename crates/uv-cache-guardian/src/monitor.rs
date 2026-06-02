//! Core cache monitoring — tracks uv's cache directory size, bandwidth, and install time.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Disk budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskBudget {
    /// Maximum cache directory size in bytes (default 5 GB).
    pub max_bytes: u64,
}

impl Default for DiskBudget {
    fn default() -> Self {
        Self {
            max_bytes: 5 * 1024 * 1024 * 1024, // 5 GB
        }
    }
}

/// Bandwidth budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthBudget {
    /// Maximum download per day in bytes (default 10 GB).
    pub max_bytes_per_day: u64,
    /// Bytes downloaded today so far.
    pub bytes_today: u64,
    /// When the counter was last reset.
    pub last_reset: DateTime<Utc>,
}

impl Default for BandwidthBudget {
    fn default() -> Self {
        Self {
            max_bytes_per_day: 10 * 1024 * 1024 * 1024, // 10 GB
            bytes_today: 0,
            last_reset: Utc::now(),
        }
    }
}

/// Time budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBudget {
    /// Maximum install time per CI run in seconds (default 5 min).
    pub max_seconds: u64,
}

impl Default for TimeBudget {
    fn default() -> Self {
        Self {
            max_seconds: 300, // 5 minutes
        }
    }
}

/// Unified budget configuration combining disk, bandwidth, and time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Budget {
    pub disk: DiskBudget,
    pub bandwidth: BandwidthBudget,
    pub time: TimeBudget,
}

/// Current cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Total size of the uv cache in bytes.
    pub cache_size_bytes: u64,
    /// Number of files in the cache.
    pub file_count: u64,
    /// Number of packages / directories in cache.
    pub package_count: u64,
    /// Timestamp of measurement.
    pub measured_at: DateTime<Utc>,
}

/// The main cache guardian — monitors uv's cache and applies budgets.
#[derive(Debug)]
pub struct CacheGuardian {
    /// Path to uv's cache directory.
    pub cache_path: PathBuf,
    /// Active budget configuration.
    pub budget: Budget,
    /// Whether budgets are enforced.
    pub enabled: bool,
}

impl CacheGuardian {
    /// Create a new CacheGuardian for the given uv cache path.
    pub fn new(cache_path: impl Into<PathBuf>) -> Self {
        Self {
            cache_path: cache_path.into(),
            budget: Budget::default(),
            enabled: true,
        }
    }

    /// Create a CacheGuardian with custom budget.
    pub fn with_budget(cache_path: impl Into<PathBuf>, budget: Budget) -> Self {
        Self {
            cache_path: cache_path.into(),
            budget,
            enabled: true,
        }
    }

    /// Measure current cache statistics.
    pub fn measure(&self) -> Result<CacheStats> {
        let path = &self.cache_path;
        let mut total_size = 0u64;
        let mut file_count = 0u64;
        let mut package_count = 0u64;

        if path.exists() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Ok(meta) = entry.metadata() {
                        total_size += meta.len();
                    }
                    file_count += 1;
                } else if entry.file_type().is_dir() {
                    // Count top-level directories as packages
                    if entry.depth() == 1 {
                        package_count += 1;
                    }
                }
            }
        }

        Ok(CacheStats {
            cache_size_bytes: total_size,
            file_count,
            package_count,
            measured_at: Utc::now(),
        })
    }

    /// Check if the cache exceeds the disk budget.
    pub fn disk_budget_exceeded(&self, stats: &CacheStats) -> bool {
        stats.cache_size_bytes > self.budget.disk.max_bytes
    }

    /// Check if bandwidth budget has been exceeded.
    pub fn bandwidth_budget_exceeded(&self) -> bool {
        let bw = &self.budget.bandwidth;
        bw.bytes_today > bw.max_bytes_per_day
    }

    /// Record a download of `bytes` bytes toward the bandwidth budget.
    pub fn record_download(&mut self, bytes: u64) {
        let now = Utc::now();
        let bw = &mut self.budget.bandwidth;

        // Reset counter if it's a new day
        if bw.last_reset.date_naive() != now.date_naive() {
            bw.bytes_today = 0;
            bw.last_reset = now;
        }

        bw.bytes_today = bw.bytes_today.saturating_add(bytes);
    }

    /// Estimate remaining time budget for a CI run.
    pub fn remaining_time_budget(&self, elapsed_seconds: u64) -> u64 {
        self.budget.time.max_seconds.saturating_sub(elapsed_seconds)
    }

    /// Measure installation time of a run.
    pub fn measure_install_time<F, T>(&self, f: F) -> (Duration, T)
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        (elapsed, result)
    }

    /// Format a byte count into a human-readable string.
    pub fn format_bytes(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;
        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }
        format!("{:.2} {}", size, UNITS[unit_idx])
    }

    /// Get a human-readable summary of current cache usage vs budget.
    pub fn summary(&self, stats: &CacheStats) -> String {
        let disk_pct = if self.budget.disk.max_bytes > 0 {
            (stats.cache_size_bytes as f64 / self.budget.disk.max_bytes as f64) * 100.0
        } else {
            0.0
        };
        let bw_pct = if self.budget.bandwidth.max_bytes_per_day > 0 {
            (self.budget.bandwidth.bytes_today as f64
                / self.budget.bandwidth.max_bytes_per_day as f64)
                * 100.0
        } else {
            0.0
        };

        format!(
            "📦 Cache: {} / {} ({:.1}%) | 🌐 Bandwidth: {} / {} ({:.1}%) | ⏱ Budget: {}s",
            Self::format_bytes(stats.cache_size_bytes),
            Self::format_bytes(self.budget.disk.max_bytes),
            disk_pct,
            Self::format_bytes(self.budget.bandwidth.bytes_today),
            Self::format_bytes(self.budget.bandwidth.max_bytes_per_day),
            bw_pct,
            self.budget.time.max_seconds,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_cache_guardian_new() {
        let guardian = CacheGuardian::new("/tmp/uv-cache");
        assert!(guardian.enabled);
        assert_eq!(guardian.cache_path, PathBuf::from("/tmp/uv-cache"));
    }

    #[test]
    fn test_disk_budget_default() {
        let budget = DiskBudget::default();
        assert_eq!(budget.max_bytes, 5 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_bandwidth_budget_default() {
        let budget = BandwidthBudget::default();
        assert_eq!(budget.max_bytes_per_day, 10 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_time_budget_default() {
        let budget = TimeBudget::default();
        assert_eq!(budget.max_seconds, 300);
    }

    #[test]
    fn test_budget_default() {
        let budget = Budget::default();
        assert_eq!(budget.disk.max_bytes, 5 * 1024 * 1024 * 1024);
        assert_eq!(budget.time.max_seconds, 300);
    }

    #[test]
    fn test_measure_empty_cache() -> Result<()> {
        let tmp = TempDir::new()?;
        let guardian = CacheGuardian::new(tmp.path());
        let stats = guardian.measure()?;
        assert_eq!(stats.cache_size_bytes, 0);
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.package_count, 0);
        Ok(())
    }

    #[test]
    fn test_measure_non_empty_cache() -> Result<()> {
        let tmp = TempDir::new()?;
        let pkg_dir = tmp.path().join("numpy");
        fs::create_dir_all(&pkg_dir)?;
        fs::write(pkg_dir.join("numpy-1.26.0.whl"), "fake-wheel-data")?;
        fs::write(pkg_dir.join("METADATA"), "metadata-content")?;

        let guardian = CacheGuardian::new(tmp.path());
        let stats = guardian.measure()?;
        assert!(stats.cache_size_bytes > 0);
        assert_eq!(stats.file_count, 2);
        assert_eq!(stats.package_count, 1);
        Ok(())
    }

    #[test]
    fn test_disk_budget_exceeded() -> Result<()> {
        let tmp = TempDir::new()?;
        let pkg_dir = tmp.path().join("big-pkg");
        fs::create_dir_all(&pkg_dir)?;
        let data = vec![0u8; 100];
        fs::write(pkg_dir.join("data.bin"), &data)?;

        let mut budget = Budget::default();
        budget.disk.max_bytes = 50;
        let guardian = CacheGuardian::with_budget(tmp.path(), budget);
        let stats = guardian.measure()?;
        assert!(guardian.disk_budget_exceeded(&stats));
        Ok(())
    }

    #[test]
    fn test_disk_budget_not_exceeded() -> Result<()> {
        let tmp = TempDir::new()?;
        let guardian = CacheGuardian::new(tmp.path());
        let stats = guardian.measure()?;
        assert!(!guardian.disk_budget_exceeded(&stats));
        Ok(())
    }

    #[test]
    fn test_bandwidth_budget_exceeded() {
        let mut guardian = CacheGuardian::new("/tmp/uv-cache");
        guardian.budget.bandwidth.max_bytes_per_day = 100;
        guardian.record_download(150);
        assert!(guardian.bandwidth_budget_exceeded());
    }

    #[test]
    fn test_bandwidth_budget_not_exceeded() {
        let mut guardian = CacheGuardian::new("/tmp/uv-cache");
        guardian.budget.bandwidth.max_bytes_per_day = 1000;
        guardian.record_download(500);
        assert!(!guardian.bandwidth_budget_exceeded());
    }

    #[test]
    fn test_remaining_time_budget() {
        let guardian = CacheGuardian::new("/tmp/uv-cache");
        assert_eq!(guardian.remaining_time_budget(60), 240);
        assert_eq!(guardian.remaining_time_budget(350), 0);
    }

    #[test]
    fn test_measure_install_time() {
        let guardian = CacheGuardian::new("/tmp/uv-cache");
        let (duration, result) = guardian.measure_install_time(|| 42);
        assert_eq!(result, 42);
        assert!(duration.as_millis() < 1000);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(CacheGuardian::format_bytes(0), "0.00 B");
        assert_eq!(CacheGuardian::format_bytes(1024), "1.00 KB");
        assert_eq!(CacheGuardian::format_bytes(1_048_576), "1.00 MB");
        assert_eq!(CacheGuardian::format_bytes(1_073_741_824), "1.00 GB");
        assert!(CacheGuardian::format_bytes(5_368_709_120).starts_with("5.00"));
    }

    #[test]
    fn test_record_download_resets_on_new_day() {
        let mut guardian = CacheGuardian::new("/tmp/uv-cache");
        guardian.budget.bandwidth.bytes_today = 900;
        guardian.budget.bandwidth.last_reset =
            Utc::now() - chrono::Duration::days(2);
        guardian.record_download(100);
        assert_eq!(guardian.budget.bandwidth.bytes_today, 100);
    }

    #[test]
    fn test_summary_format() -> Result<()> {
        let tmp = TempDir::new()?;
        let guardian = CacheGuardian::new(tmp.path());
        let stats = guardian.measure()?;
        let summary = guardian.summary(&stats);
        assert!(summary.contains("Cache:"));
        assert!(summary.contains("Bandwidth:"));
        assert!(summary.contains("Budget:"));
        Ok(())
    }
}
