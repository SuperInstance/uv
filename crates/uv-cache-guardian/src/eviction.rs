//! KL divergence-based intelligent cache eviction.
//!
//! The eviction strategy uses KL divergence between dependency profiles
//! to decide which packages to evict first. Packages with highly unique
//! dependency profiles (high divergence from the mean) are evicted first,
//! preserving packages that are widely shared across projects.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// A dependency profile for a project — a probability distribution over packages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyProfile {
    /// Package name → normalized probability weight.
    pub weights: HashMap<String, f64>,
}

impl DependencyProfile {
    /// Create a dependency profile from raw count data.
    ///
    /// Automatically normalizes counts into probabilities.
    pub fn from_counts(packages: Vec<(String, usize)>) -> Self {
        let total: usize = packages.iter().map(|(_, c)| c).sum();
        let total = total.max(1);

        let weights: HashMap<String, f64> = packages
            .into_iter()
            .map(|(pkg, count)| (pkg, count as f64 / total as f64))
            .collect();

        Self { weights }
    }

    /// Normalize the profile to sum to 1.0.
    pub fn normalize(&mut self) {
        let total: f64 = self.weights.values().sum();
        if total > 0.0 {
            for val in self.weights.values_mut() {
                *val /= total;
            }
        }
    }
}

/// A project's cache profile, tracking its dependencies and disk usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    /// Name of the project.
    pub name: String,
    /// Dependency profile of this project.
    pub profile: DependencyProfile,
    /// Estimated total cache usage in bytes.
    pub cache_bytes: u64,
    /// Score for eviction (higher = evict first).
    pub eviction_score: Option<f64>,
}

impl ProjectProfile {
    pub fn new(name: impl Into<String>, profile: DependencyProfile, cache_bytes: u64) -> Self {
        Self {
            name: name.into(),
            profile,
            cache_bytes,
            eviction_score: None,
        }
    }
}

/// Eviction strategy using KL divergence.
#[derive(Debug, Clone)]
pub struct EvictionStrategy {
    /// Whether to use symmetric KL divergence (Jensen-Shannon).
    pub symmetric: bool,
    /// Smoothing factor to avoid division by zero.
    pub epsilon: f64,
}

impl Default for EvictionStrategy {
    fn default() -> Self {
        Self {
            symmetric: true,
            epsilon: 1e-10,
        }
    }
}

impl EvictionStrategy {
    /// Create a new eviction strategy.
    pub fn new(symmetric: bool) -> Self {
        Self {
            symmetric,
            ..Default::default()
        }
    }

    /// Compute KL divergence D_KL(P || Q).
    ///
    /// Measures how much information is lost when Q is used to approximate P.
    pub fn kl_divergence(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
        let mut divergence = 0.0;

        // Union of all keys
        let all_keys: HashSet<&String> = p.keys().chain(q.keys()).collect();

        for key in all_keys {
            let p_val = p.get(key).copied().unwrap_or(self.epsilon);
            let q_val = q.get(key).copied().unwrap_or(self.epsilon);

            // Avoid log(0) by clamping
            let p_val = p_val.max(self.epsilon);
            let q_val = q_val.max(self.epsilon);

            divergence += p_val * (p_val / q_val).ln();
        }

        divergence
    }

    /// Compute Jensen-Shannon divergence (symmetric).
    ///
    /// JSD(P || Q) = 0.5 * D_KL(P || M) + 0.5 * D_KL(Q || M)
    /// where M = 0.5 * (P + Q)
    pub fn js_divergence(&self, p: &HashMap<String, f64>, q: &HashMap<String, f64>) -> f64 {
        let all_keys: HashSet<&String> = p.keys().chain(q.keys()).collect();

        let mut m = HashMap::new();
        for key in &all_keys {
            let p_val = p.get(*key).copied().unwrap_or(0.0);
            let q_val = q.get(*key).copied().unwrap_or(0.0);
            m.insert((*key).clone(), 0.5 * (p_val + q_val));
        }

        0.5 * self.kl_divergence(p, &m) + 0.5 * self.kl_divergence(q, &m)
    }

    /// Compute the mean dependency profile from a list of profiles.
    fn mean_profile(&self, profiles: &[DependencyProfile]) -> HashMap<String, f64> {
        let mut mean = HashMap::new();
        let count = profiles.len() as f64;

        if count == 0.0 {
            return mean;
        }

        // Collect all keys
        let all_keys: HashSet<&String> =
            profiles.iter().flat_map(|p| p.weights.keys()).collect();

        for key in all_keys {
            let sum: f64 = profiles
                .iter()
                .map(|p| p.weights.get(key).copied().unwrap_or(0.0))
                .sum();
            mean.insert(key.clone(), sum / count);
        }

        mean
    }

    /// Score each project for eviction based on KL divergence from the mean profile.
    ///
    /// Returns a list of (profile, score) pairs sorted by score descending
    /// (highest score = most divergent = evict first).
    pub fn score_profiles(&self, profiles: &[ProjectProfile]) -> Vec<(ProjectProfile, f64)> {
        if profiles.is_empty() {
            return Vec::new();
        }

        let dep_profiles: Vec<&DependencyProfile> =
            profiles.iter().map(|p| &p.profile).collect();
        let mean = self.mean_profile(
            &dep_profiles.iter().map(|p| (*p).clone()).collect::<Vec<_>>(),
        );

        let mut scored: Vec<(ProjectProfile, f64)> = profiles
            .iter()
            .map(|profile| {
                let divergence = if self.symmetric {
                    self.js_divergence(&profile.profile.weights, &mean)
                } else {
                    self.kl_divergence(&profile.profile.weights, &mean)
                };
                (profile.clone(), divergence)
            })
            .collect();

        // Sort by divergence descending (most divergent first)
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored
    }

    /// Select packages to evict to free up `target_bytes`.
    ///
    /// Returns a list of (profile, bytes_freed) that should be evicted.
    pub fn select_evictions(
        &self,
        profiles: &[ProjectProfile],
        target_bytes: u64,
    ) -> Vec<(ProjectProfile, u64)> {
        let scored = self.score_profiles(profiles);
        let mut evictions = Vec::new();
        let mut freed = 0u64;

        for (profile, score) in scored {
            if freed >= target_bytes {
                break;
            }
            let cache_bytes = profile.cache_bytes;
            let mut p = profile;
            p.eviction_score = Some(score);
            evictions.push((p, cache_bytes));
            freed = freed.saturating_add(cache_bytes);
        }

        evictions
    }

    /// Report on cache diversity — how many packages would survive eviction at a given target.
    pub fn eviction_report(&self, profiles: &[ProjectProfile], target_bytes: u64) -> EvictionReport {
        let total_bytes: u64 = profiles.iter().map(|p| p.cache_bytes).sum();
        let evictions = self.select_evictions(profiles, target_bytes);
        let bytes_freed: u64 = evictions.iter().map(|(_, b)| b).sum();
        let scored = self.score_profiles(profiles);

        EvictionReport {
            total_projects: profiles.len(),
            total_cache_bytes: total_bytes,
            target_bytes,
            bytes_freed,
            eviction_count: evictions.len(),
            surviving_projects: profiles.len() - evictions.len(),
            top_divergent: scored
                .iter()
                .take(5)
                .map(|(p, s)| (p.name.clone(), *s))
                .collect(),
        }
    }
}

/// Summary report of an eviction analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionReport {
    pub total_projects: usize,
    pub total_cache_bytes: u64,
    pub target_bytes: u64,
    pub bytes_freed: u64,
    pub eviction_count: usize,
    pub surviving_projects: usize,
    pub top_divergent: Vec<(String, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(name: &str, pkgs: &[(&str, usize)], bytes: u64) -> ProjectProfile {
        ProjectProfile::new(
            name,
            DependencyProfile::from_counts(pkgs.iter().map(|(n, c)| (n.to_string(), *c)).collect()),
            bytes,
        )
    }

    #[test]
    fn test_kl_divergence_same() {
        let strategy = EvictionStrategy::default();
        let mut p = HashMap::new();
        p.insert("numpy".to_string(), 0.5);
        p.insert("pandas".to_string(), 0.5);

        let divergence = strategy.kl_divergence(&p, &p);
        assert!(
            (divergence - 0.0).abs() < 1e-6,
            "KL divergence of identical distributions should be 0, got {}",
            divergence
        );
    }

    #[test]
    fn test_kl_divergence_different() {
        let strategy = EvictionStrategy::default();
        let mut p = HashMap::new();
        p.insert("numpy".to_string(), 1.0);

        let mut q = HashMap::new();
        q.insert("pandas".to_string(), 1.0);

        let divergence = strategy.kl_divergence(&p, &q);
        assert!(
            divergence > 0.0,
            "KL divergence of different distributions should be > 0, got {}",
            divergence
        );
    }

    #[test]
    fn test_js_divergence_symmetric() {
        let strategy = EvictionStrategy::default();

        let mut p = HashMap::new();
        p.insert("numpy".to_string(), 1.0);

        let mut q = HashMap::new();
        q.insert("pandas".to_string(), 1.0);

        let d1 = strategy.js_divergence(&p, &q);
        let d2 = strategy.js_divergence(&q, &p);
        assert!(
            (d1 - d2).abs() < 1e-6,
            "JSD should be symmetric: {} vs {}",
            d1,
            d2
        );
    }

    #[test]
    fn test_js_divergence_identical() {
        let strategy = EvictionStrategy::default();
        let mut p = HashMap::new();
        p.insert("numpy".to_string(), 1.0);

        let jsd = strategy.js_divergence(&p, &p);
        assert!(
            jsd.abs() < 1e-6,
            "JSD of identical distributions should be 0, got {}",
            jsd
        );
    }

    #[test]
    fn test_score_profiles() {
        let strategy = EvictionStrategy::default();

        let profiles = vec![
            make_profile("project-a", &[("numpy", 1), ("pandas", 1)], 100),
            make_profile("project-b", &[("numpy", 1), ("numba", 1)], 200),
            make_profile("project-c", &[("numpy", 1), ("pandas", 1)], 150),
        ];

        let scored = strategy.score_profiles(&profiles);
        assert_eq!(scored.len(), 3);

        // project-b has numba which is unique, should be most divergent
        assert_eq!(scored[0].0.name, "project-b");
    }

    #[test]
    fn test_select_evictions() {
        let strategy = EvictionStrategy::default();

        let profiles = vec![
            make_profile("project-a", &[("numpy", 1)], 100),
            make_profile("project-b", &[("torch", 1)], 500),
            make_profile("project-c", &[("numpy", 1)], 100),
        ];

        let evictions = strategy.select_evictions(&profiles, 400);
        assert!(!evictions.is_empty());

        // project-b has unique deps, should be evicted first
        assert_eq!(evictions[0].0.name, "project-b");
    }

    #[test]
    fn test_select_evictions_target_met() {
        let strategy = EvictionStrategy::default();
        let profiles = vec![
            make_profile("project-a", &[("numpy", 1)], 500),
            make_profile("project-b", &[("torch", 1)], 500),
        ];

        let evictions = strategy.select_evictions(&profiles, 300);
        assert_eq!(evictions.len(), 1);
        assert!((evictions[0].1 as i64) >= 300);
    }

    #[test]
    fn test_eviction_report() {
        let strategy = EvictionStrategy::default();
        let profiles = vec![make_profile("project-a", &[("numpy", 1)], 100)];

        let report = strategy.eviction_report(&profiles, 100);
        assert_eq!(report.total_projects, 1);
        assert_eq!(report.bytes_freed, 100);
        assert_eq!(report.eviction_count, 1);
    }

    #[test]
    fn test_empty_profiles() {
        let strategy = EvictionStrategy::default();
        let scored = strategy.score_profiles(&[]);
        assert!(scored.is_empty());

        let evictions = strategy.select_evictions(&[], 1000);
        assert!(evictions.is_empty());
    }

    #[test]
    fn test_dependency_profile_creation() {
        let items = vec![
            ("numpy".to_string(), 3),
            ("pandas".to_string(), 2),
            ("flask".to_string(), 1),
        ];
        let profile = DependencyProfile::from_counts(items);
        let total: f64 = profile.weights.values().sum();
        assert!((total - 1.0).abs() < 1e-6);
        assert!((profile.weights["numpy"] - 0.5).abs() < 1e-6);
    }
}
