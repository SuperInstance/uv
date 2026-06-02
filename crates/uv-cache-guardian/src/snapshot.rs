//! Serde snapshots for CI audit trails.
//!
//! Every check produces a snapshot that can be serialized for CI audit, debugging,
//! or trend analysis.

use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::conservation::ConservationResult;
use crate::eviction::ProjectProfile;
use crate::monitor::CacheStats;
use crate::phase::PhaseReport;

/// A complete snapshot of cache state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSnapshot {
    /// Snapshot metadata.
    pub meta: SnapshotMeta,
    /// Current cache statistics.
    pub stats: CacheStats,
    /// Conservation law check results.
    pub conservation: Vec<ConservationResult>,
    /// Phase detection report.
    pub phase: PhaseReport,
    /// Project profiles being tracked.
    pub projects: Vec<ProjectProfile>,
    /// Evictions performed (if any).
    pub evictions: Vec<EvictionEntry>,
}

/// Metadata about the snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    /// Version of the snapshot format.
    pub version: u32,
    /// When the snapshot was created.
    pub timestamp: DateTime<Utc>,
    /// Hostname or CI run identifier.
    pub source: String,
    /// Description of what triggered this snapshot.
    pub reason: String,
}

impl Default for SnapshotMeta {
    fn default() -> Self {
        Self {
            version: 1,
            timestamp: Utc::now(),
            source: "unknown".to_string(),
            reason: "periodic check".to_string(),
        }
    }
}

/// An eviction entry in the audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionEntry {
    /// Package name that was evicted.
    pub package: String,
    /// Project it belonged to.
    pub project: String,
    /// Size freed in bytes.
    pub bytes_freed: u64,
    /// Why this package was selected for eviction.
    pub reason: String,
    /// When the eviction occurred.
    pub timestamp: DateTime<Utc>,
}

impl CacheSnapshot {
    /// Create a new cache snapshot builder.
    pub fn builder(source: impl Into<String>) -> SnapshotBuilder {
        SnapshotBuilder::new(source)
    }

    /// Serialize to pretty JSON string.
    pub fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Serialize to compact JSON string.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Write the snapshot to a file as JSON.
    pub fn write_json(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.to_json_pretty()?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Deserialize a snapshot from a JSON file.
    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Get a one-line summary of this snapshot.
    pub fn summary_line(&self) -> String {
        let all_satisfied = self.conservation.iter().all(|r| r.satisfied);
        let status = if all_satisfied { "✅" } else { "⚠️" };

        format!(
            "{} [{}] Cache: {} — {} conservation check(s), phase: {} ({}), {} project(s)",
            status,
            self.meta.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            crate::monitor::CacheGuardian::format_bytes(self.stats.cache_size_bytes),
            self.conservation.len(),
            self.phase.phase.label(),
            self.phase.action,
            self.projects.len(),
        )
    }
}

/// Builder pattern for constructing cache snapshots.
#[derive(Debug)]
pub struct SnapshotBuilder {
    meta: SnapshotMeta,
    stats: Option<CacheStats>,
    conservation: Vec<ConservationResult>,
    phase: Option<PhaseReport>,
    projects: Vec<ProjectProfile>,
    evictions: Vec<EvictionEntry>,
}

impl SnapshotBuilder {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            meta: SnapshotMeta {
                source: source.into(),
                ..Default::default()
            },
            stats: None,
            conservation: Vec::new(),
            phase: None,
            projects: Vec::new(),
            evictions: Vec::new(),
        }
    }

    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.meta.reason = reason.into();
        self
    }

    pub fn stats(mut self, stats: CacheStats) -> Self {
        self.stats = Some(stats);
        self
    }

    pub fn conservation(mut self, results: Vec<ConservationResult>) -> Self {
        self.conservation = results;
        self
    }

    pub fn phase(mut self, report: PhaseReport) -> Self {
        self.phase = Some(report);
        self
    }

    pub fn projects(mut self, projects: Vec<ProjectProfile>) -> Self {
        self.projects = projects;
        self
    }

    pub fn eviction(mut self, entry: EvictionEntry) -> Self {
        self.evictions.push(entry);
        self
    }

    pub fn evictions(mut self, entries: Vec<EvictionEntry>) -> Self {
        self.evictions = entries;
        self
    }

    pub fn build(self) -> Result<CacheSnapshot> {
        Ok(CacheSnapshot {
            meta: self.meta,
            stats: self.stats.unwrap_or(CacheStats {
                cache_size_bytes: 0,
                file_count: 0,
                package_count: 0,
                measured_at: Utc::now(),
            }),
            conservation: self.conservation,
            phase: self.phase.unwrap_or(
                crate::phase::PhaseDetector::default().report(0.0),
            ),
            projects: self.projects,
            evictions: self.evictions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conservation::ConservationChecker;
    use crate::monitor::CacheStats;
    use crate::phase::{PhaseDetector, ResourcePhase};
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_builder() -> Result<()> {
        let stats = CacheStats {
            cache_size_bytes: 1_000_000,
            file_count: 100,
            package_count: 10,
            measured_at: Utc::now(),
        };

        let conservation = ConservationChecker::check_all(
            1_000_000,
            5_000_000_000,
            500_000_000,
            10_000_000_000,
            120,
            300,
        );

        let detector = PhaseDetector::default();
        let phase = detector.report(0.20);

        let snapshot = CacheSnapshot::builder("test-ci-run")
            .reason("integration test")
            .stats(stats)
            .conservation(conservation)
            .phase(phase)
            .build()?;

        assert_eq!(snapshot.meta.source, "test-ci-run");
        assert_eq!(snapshot.meta.reason, "integration test");
        assert_eq!(snapshot.stats.cache_size_bytes, 1_000_000);
        assert_eq!(snapshot.conservation.len(), 3);
        assert_eq!(snapshot.phase.phase, ResourcePhase::Stable);
        Ok(())
    }

    #[test]
    fn test_snapshot_json_roundtrip() -> Result<()> {
        let snapshot = CacheSnapshot::builder("test")
            .reason("roundtrip test")
            .projects(vec![ProjectProfile::new(
                "test-project",
                crate::eviction::DependencyProfile::from_counts(
                    vec![("numpy".to_string(), 1)],
                ),
                1_000_000,
            )])
            .eviction(EvictionEntry {
                package: "numpy".to_string(),
                project: "test-project".to_string(),
                bytes_freed: 1_000_000,
                reason: "test eviction".to_string(),
                timestamp: Utc::now(),
            })
            .build()?;

        let json = snapshot.to_json_pretty()?;
        let restored = serde_json::from_str::<CacheSnapshot>(&json)?;

        assert_eq!(restored.meta.source, snapshot.meta.source);
        assert_eq!(restored.projects.len(), 1);
        assert_eq!(restored.evictions.len(), 1);
        assert_eq!(restored.evictions[0].package, "numpy");
        Ok(())
    }

    #[test]
    fn test_snapshot_write_read_file() -> Result<()> {
        let tmp = TempDir::new()?;
        let path = tmp.path().join("snapshot.json");

        let snapshot = CacheSnapshot::builder("test")
            .reason("file test")
            .build()?;

        snapshot.write_json(&path)?;
        assert!(path.exists());

        let restored = CacheSnapshot::from_json_file(&path)?;
        assert_eq!(restored.meta.reason, "file test");
        assert_eq!(restored.meta.source, "test");
        Ok(())
    }

    #[test]
    fn test_snapshot_summary_line() -> Result<()> {
        let stats = CacheStats {
            cache_size_bytes: 1024,
            file_count: 5,
            package_count: 2,
            measured_at: Utc::now(),
        };

        let conservation = ConservationChecker::check_all(
            1024, 5_000_000_000, 0, 10_000_000_000, 10, 300,
        );

        let detector = PhaseDetector::default();
        let phase = detector.report(0.01);

        let snapshot = CacheSnapshot::builder("ci-run-42")
            .reason("periodic")
            .stats(stats)
            .conservation(conservation)
            .phase(phase)
            .build()?;

        let line = snapshot.summary_line();
        assert!(line.contains("✅"), "line: {line}");
        assert!(line.contains("1.00 KB"), "line: {line}");
        assert!(line.contains("3 conservation"), "line: {line}");
        Ok(())
    }

    #[test]
    fn test_eviction_entry_serde() {
        let entry = EvictionEntry {
            package: "requests".to_string(),
            project: "web-app".to_string(),
            bytes_freed: 500_000,
            reason: "KL divergence outlier — unique dependency".to_string(),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let restored: EvictionEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.package, "requests");
        assert_eq!(restored.bytes_freed, 500_000);
    }

    #[test]
    fn test_meta_default() {
        let meta = SnapshotMeta::default();
        assert_eq!(meta.version, 1);
        assert_eq!(meta.reason, "periodic check");
    }
}
