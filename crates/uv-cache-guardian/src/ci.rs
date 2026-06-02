//! CI optimization — detect when multiple PRs download the same packages and
//! suggest cache warming strategies.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A record of a CI pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrRecord {
    /// PR number or identifier.
    pub pr_id: String,
    /// When the run occurred.
    pub timestamp: DateTime<Utc>,
    /// Packages downloaded during this run.
    pub packages_downloaded: Vec<String>,
    /// Total download size in bytes.
    pub total_bytes: u64,
    /// Duration of the run in seconds.
    pub duration_seconds: u64,
    /// Whether the run used a warm cache.
    pub cache_warmed: bool,
}

/// CI pipeline history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrHistory {
    /// All recorded PR runs.
    pub records: Vec<PrRecord>,
}

impl Default for PrHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl PrHistory {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    /// Add a record to history.
    pub fn add(&mut self, record: PrRecord) {
        self.records.push(record);
    }

    /// Find the most frequently downloaded packages across all PRs.
    pub fn most_downloaded_packages(&self, top_n: usize) -> Vec<(String, usize)> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for record in &self.records {
            for pkg in &record.packages_downloaded {
                *counts.entry(pkg.clone()).or_insert(0) += 1;
            }
        }

        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sorted.truncate(top_n);
        sorted
    }

    /// Find packages that are downloaded by multiple PRs within a time window.
    /// These are candidates for cache warming.
    pub fn overlapping_downloads(&self, window_hours: i64) -> Vec<String> {
        let mut package_prs: HashMap<String, HashSet<String>> = HashMap::new();
        let cutoff = Utc::now() - chrono::Duration::hours(window_hours);

        for record in &self.records {
            if record.timestamp < cutoff {
                continue;
            }
            for pkg in &record.packages_downloaded {
                package_prs
                    .entry(pkg.clone())
                    .or_default()
                    .insert(record.pr_id.clone());
            }
        }

        // Only packages downloaded by >1 PR
        let mut overlapping: Vec<String> = package_prs
            .into_iter()
            .filter(|(_, prs)| prs.len() > 1)
            .map(|(pkg, _)| pkg)
            .collect();

        overlapping.sort();
        overlapping
    }

    /// Estimate bytes that could be saved if overlapping packages were cached.
    pub fn potential_savings(&self, window_hours: i64) -> u64 {
        let overlapping: HashSet<String> =
            self.overlapping_downloads(window_hours).into_iter().collect();
        let cutoff = Utc::now() - chrono::Duration::hours(window_hours);

        self.records
            .iter()
            .filter(|r| r.timestamp >= cutoff)
            .flat_map(|r| &r.packages_downloaded)
            .filter(|pkg| overlapping.contains(*pkg))
            .count() as u64
            * 10_000_000 // rough estimate: 10 MB per package download
    }

    /// Generate cache warming suggestions.
    pub fn warming_suggestions(&self, window_hours: i64) -> Vec<WarmingSuggestion> {
        let overlapping = self.overlapping_downloads(window_hours);
        let mut suggestions = Vec::new();

        for pkg in overlapping {
            let prs: Vec<String> = self
                .records
                .iter()
                .filter(|r| r.packages_downloaded.contains(&pkg))
                .map(|r| r.pr_id.clone())
                .collect();

            let total_bytes: u64 = self
                .records
                .iter()
                .filter(|r| r.packages_downloaded.contains(&pkg))
                .map(|r| r.total_bytes)
                .sum();

            let pkg_name = pkg.clone();
            suggestions.push(WarmingSuggestion {
                package: pkg_name.clone(),
                affected_prs: prs,
                estimated_savings_bytes: total_bytes,
                suggestion: format!(
                    "Pre-warm {} in CI cache — {} PR(s) downloaded it",
                    pkg_name,
                    self.records
                        .iter()
                        .filter(|r| r.packages_downloaded.contains(&pkg))
                        .count(),
                ),
            });
        }

        suggestions.sort_by_key(|b| std::cmp::Reverse(b.estimated_savings_bytes));
        suggestions
    }
}

/// A suggestion to warm the cache for a particular package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarmingSuggestion {
    pub package: String,
    pub affected_prs: Vec<String>,
    pub estimated_savings_bytes: u64,
    pub suggestion: String,
}

/// CI optimizer — analyzes PR history and suggests improvements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiOptimizer {
    /// PR history.
    pub history: PrHistory,
    /// Time window for overlap analysis (hours).
    pub analysis_window_hours: i64,
}

impl Default for CiOptimizer {
    fn default() -> Self {
        Self {
            history: PrHistory::new(),
            analysis_window_hours: 24,
        }
    }
}

impl CiOptimizer {
    pub fn new(history: PrHistory) -> Self {
        Self {
            history,
            analysis_window_hours: 24,
        }
    }

    /// Get the top N most downloaded packages.
    pub fn top_packages(&self, n: usize) -> Vec<(String, usize)> {
        self.history.most_downloaded_packages(n)
    }

    /// Get overlapping downloads (same package downloaded by multiple PRs).
    pub fn overlapping_downloads(&self) -> Vec<String> {
        self.history.overlapping_downloads(self.analysis_window_hours)
    }

    /// Get cache warming suggestions.
    pub fn warming_suggestions(&self) -> Vec<WarmingSuggestion> {
        self.history.warming_suggestions(self.analysis_window_hours)
    }

    /// Get a CI optimization report.
    pub fn report(&self) -> CiReport {
        let top = self.top_packages(5);
        let overlapping = self.overlapping_downloads();
        let suggestions = self.warming_suggestions();
        let savings = self.history.potential_savings(self.analysis_window_hours);

        let total_downloads: u64 = self
            .history
            .records
            .iter()
            .map(|r| r.packages_downloaded.len() as u64)
            .sum();

        CiReport {
            total_records: self.history.records.len(),
            total_downloads,
            top_packages: top,
            overlapping_packages: overlapping,
            warming_suggestions: suggestions,
            estimated_savings_bytes: savings,
            analysis_window_hours: self.analysis_window_hours,
        }
    }
}

/// CI optimization report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiReport {
    pub total_records: usize,
    pub total_downloads: u64,
    pub top_packages: Vec<(String, usize)>,
    pub overlapping_packages: Vec<String>,
    pub warming_suggestions: Vec<WarmingSuggestion>,
    pub estimated_savings_bytes: u64,
    pub analysis_window_hours: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(
        pr_id: &str,
        packages: &[&str],
        bytes: u64,
        duration: u64,
        warmed: bool,
    ) -> PrRecord {
        PrRecord {
            pr_id: pr_id.to_string(),
            timestamp: Utc::now(),
            packages_downloaded: packages.iter().map(|s| s.to_string()).collect(),
            total_bytes: bytes,
            duration_seconds: duration,
            cache_warmed: warmed,
        }
    }

    #[test]
    fn test_pr_history_add() {
        let mut history = PrHistory::new();
        assert!(history.records.is_empty());
        history.add(make_record("PR-1", &["numpy"], 100, 30, false));
        assert_eq!(history.records.len(), 1);
    }

    #[test]
    fn test_most_downloaded_packages() {
        let mut history = PrHistory::new();
        history.add(make_record("PR-1", &["numpy", "pandas"], 200, 30, false));
        history.add(make_record("PR-2", &["numpy", "flask"], 150, 25, false));
        history.add(make_record("PR-3", &["pandas", "requests"], 180, 28, false));

        let top = history.most_downloaded_packages(5);
        assert_eq!(top[0].0, "numpy");
        assert_eq!(top[0].1, 2);
    }

    #[test]
    fn test_overlapping_downloads() {
        let mut history = PrHistory::new();
        history.add(make_record("PR-1", &["numpy", "pandas"], 200, 30, false));
        history.add(make_record("PR-2", &["numpy", "torch"], 500, 60, false));

        let overlapping = history.overlapping_downloads(48);
        assert!(overlapping.contains(&"numpy".to_string()));
        assert!(!overlapping.contains(&"pandas".to_string()));
    }

    #[test]
    fn test_warming_suggestions() {
        let mut history = PrHistory::new();
        history.add(make_record("PR-1", &["numpy", "pandas"], 200, 30, false));
        history.add(make_record("PR-2", &["numpy", "torch"], 500, 60, false));

        let suggestions = history.warming_suggestions(48);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].package, "numpy");
        assert!(suggestions[0].suggestion.contains("Pre-warm"));
    }

    #[test]
    fn test_ci_optimizer_report() {
        let mut history = PrHistory::new();
        history.add(make_record("PR-1", &["numpy", "pandas"], 200, 30, false));
        history.add(make_record("PR-2", &["numpy", "torch"], 500, 60, false));

        let optimizer = CiOptimizer::new(history);
        let report = optimizer.report();
        assert_eq!(report.total_records, 2);
        assert_eq!(report.total_downloads, 4);
        assert_eq!(report.top_packages[0].0, "numpy");
    }

    #[test]
    fn test_potential_savings() {
        let mut history = PrHistory::new();
        history.add(make_record("PR-1", &["numpy", "pandas"], 200, 30, false));
        history.add(make_record("PR-2", &["numpy", "torch"], 500, 60, false));

        let savings = history.potential_savings(48);
        assert!(savings > 0);
    }

    #[test]
    fn test_empty_history() {
        let history = PrHistory::new();
        assert!(history.most_downloaded_packages(5).is_empty());
        assert!(history.overlapping_downloads(24).is_empty());
        assert!(history.warming_suggestions(24).is_empty());
    }

    #[test]
    fn test_ci_optimizer_default() {
        let optimizer = CiOptimizer::default();
        assert_eq!(optimizer.analysis_window_hours, 24);
        assert!(optimizer.history.records.is_empty());
    }
}
