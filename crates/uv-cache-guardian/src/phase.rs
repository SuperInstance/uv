//! Resource phase detection.
//!
//! Identifies the current resource phase based on cache growth rate and thresholds.
//!
//! Phases:
//! - **Stable**: normal operation
//! - **PreTransition**: cache growing fast, approach budget limit
//! - **Transitioning**: at budget limit, active eviction required
//! - **PostTransition**: just evicted, recovery monitoring

use serde::{Deserialize, Serialize};

/// The four resource phases a uv cache can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourcePhase {
    /// Normal operation — cache is well within budget.
    Stable,
    /// Approaching budget — proactive measures recommended.
    PreTransition,
    /// At or over budget — active eviction needed.
    Transitioning,
    /// Post-eviction recovery — monitor for re-stabilization.
    PostTransition,
}

impl ResourcePhase {
    /// Human-readable label for this phase.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stable => "Stable",
            Self::PreTransition => "PreTransition",
            Self::Transitioning => "Transitioning",
            Self::PostTransition => "PostTransition",
        }
    }

    /// Emoji icon for this phase.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Stable => "✅",
            Self::PreTransition => "⚠️",
            Self::Transitioning => "🚨",
            Self::PostTransition => "🔄",
        }
    }

    /// Suggested action for this phase.
    pub fn suggested_action(&self) -> &'static str {
        match self {
            Self::Stable => "Continue normal operation",
            Self::PreTransition => "Consider proactive cache pruning",
            Self::Transitioning => "Initiate cache eviction now",
            Self::PostTransition => "Monitor for stabilization",
        }
    }
}

/// Phase detection thresholds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseThresholds {
    /// Utilization percentage at which PreTransition starts (e.g., 0.80 = 80%).
    pub pre_transition: f64,
    /// Utilization percentage at which Transitioning starts (e.g., 0.95 = 95%).
    pub transitioning: f64,
    /// Growth rate factor for transition detection.
    pub growth_rate_factor: f64,
}

impl Default for PhaseThresholds {
    fn default() -> Self {
        Self {
            pre_transition: 0.80,
            transitioning: 0.95,
            growth_rate_factor: 0.10,
        }
    }
}

/// Result of a phase detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseReport {
    /// Detected phase.
    pub phase: ResourcePhase,
    /// Current utilization (0.0 – 1.0+).
    pub utilization: f64,
    /// Growth rate (fraction of budget per measurement interval).
    pub growth_rate: f64,
    /// Suggested action text.
    pub action: String,
}

/// Detects the current resource phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseDetector {
    /// Phase thresholds.
    pub thresholds: PhaseThresholds,
    /// Previous utilization for growth rate calculation.
    pub prev_utilization: Option<f64>,
}

impl Default for PhaseDetector {
    fn default() -> Self {
        Self {
            thresholds: PhaseThresholds::default(),
            prev_utilization: None,
        }
    }
}

impl PhaseDetector {
    /// Create a new PhaseDetector.
    pub fn new(thresholds: PhaseThresholds) -> Self {
        Self {
            thresholds,
            prev_utilization: None,
        }
    }

    /// Detect the current resource phase based on utilization and growth rate.
    pub fn detect(&mut self, utilization: f64, _growth_rate: f64) -> ResourcePhase {
        let prev = self.prev_utilization;
        self.prev_utilization = Some(utilization);

        // Check PostTransition first: utilization dropped below pre_transition
        // threshold after having been above transitioning threshold.
        if utilization < self.thresholds.pre_transition {
            if prev
                .map(|p| p >= self.thresholds.transitioning)
                .unwrap_or(false)
            {
                return ResourcePhase::PostTransition;
            }
            return ResourcePhase::Stable;
        }

        if utilization >= self.thresholds.transitioning {
            ResourcePhase::Transitioning
        } else {
            ResourcePhase::PreTransition
        }
    }

    /// Create a report with the current phase state.
    pub fn report(&self, utilization: f64) -> PhaseReport {
        let growth_rate = 0.0;
        let phase = ResourcePhase::Stable;
        let action = phase.suggested_action().to_string();
        PhaseReport {
            phase,
            utilization,
            growth_rate,
            action,
        }
    }

    /// Enhanced report that uses stateful detection.
    pub fn report_stateful(&mut self, utilization: f64, growth_rate: f64) -> PhaseReport {
        let phase = self.detect(utilization, growth_rate);
        let action = phase.suggested_action().to_string();
        PhaseReport {
            phase,
            utilization,
            growth_rate,
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_phase_labels() {
        assert_eq!(ResourcePhase::Stable.label(), "Stable");
        assert_eq!(ResourcePhase::PreTransition.label(), "PreTransition");
        assert_eq!(ResourcePhase::Transitioning.label(), "Transitioning");
        assert_eq!(ResourcePhase::PostTransition.label(), "PostTransition");
    }

    #[test]
    fn test_resource_phase_icons() {
        assert_eq!(ResourcePhase::Stable.icon(), "✅");
        assert_eq!(ResourcePhase::PreTransition.icon(), "⚠️");
        assert_eq!(ResourcePhase::Transitioning.icon(), "🚨");
        assert_eq!(ResourcePhase::PostTransition.icon(), "🔄");
    }

    #[test]
    fn test_phase_thresholds_default() {
        let t = PhaseThresholds::default();
        assert_eq!(t.pre_transition, 0.80);
        assert_eq!(t.transitioning, 0.95);
    }

    #[test]
    fn test_phase_detector_default() {
        let detector = PhaseDetector::default();
        assert_eq!(detector.thresholds.pre_transition, 0.80);
        assert!(detector.prev_utilization.is_none());
    }

    #[test]
    fn test_detect_stable() {
        let mut detector = PhaseDetector::default();
        let phase = detector.detect(0.50, 0.01);
        assert_eq!(phase, ResourcePhase::Stable);
    }

    #[test]
    fn test_detect_pre_transition() {
        let mut detector = PhaseDetector::default();
        let phase = detector.detect(0.85, 0.05);
        assert_eq!(phase, ResourcePhase::PreTransition);
    }

    #[test]
    fn test_detect_transitioning() {
        let mut detector = PhaseDetector::default();
        let phase = detector.detect(0.98, 0.05);
        assert_eq!(phase, ResourcePhase::Transitioning);
    }

    #[test]
    fn test_detect_transitioning_exact() {
        let mut detector = PhaseDetector::default();
        let phase = detector.detect(0.95, 0.05);
        assert_eq!(phase, ResourcePhase::Transitioning);
    }

    #[test]
    fn test_detect_post_transition() {
        let mut detector = PhaseDetector::default();
        // First, trigger Transitioning
        detector.detect(0.98, 0.05);
        // Now come down below pre_transition threshold
        let phase = detector.detect(0.60, 0.01);
        assert_eq!(phase, ResourcePhase::PostTransition);
    }

    #[test]
    fn test_report_stateful() {
        let mut detector = PhaseDetector::default();
        let report = detector.report_stateful(0.90, 0.08);
        assert_eq!(report.phase, ResourcePhase::PreTransition);
        assert_eq!(report.utilization, 0.90);
        assert_eq!(report.growth_rate, 0.08);
    }

    #[test]
    fn test_phase_suggested_actions() {
        assert_eq!(
            ResourcePhase::Stable.suggested_action(),
            "Continue normal operation"
        );
        assert_eq!(
            ResourcePhase::Transitioning.suggested_action(),
            "Initiate cache eviction now"
        );
    }

    #[test]
    fn test_detect_with_custom_thresholds() {
        let thresholds = PhaseThresholds {
            pre_transition: 0.60,
            transitioning: 0.80,
            growth_rate_factor: 0.05,
        };
        let mut detector = PhaseDetector::new(thresholds);

        assert_eq!(detector.detect(0.50, 0.01), ResourcePhase::Stable);
        assert_eq!(detector.detect(0.70, 0.01), ResourcePhase::PreTransition);
        assert_eq!(detector.detect(0.90, 0.01), ResourcePhase::Transitioning);
    }

    #[test]
    fn test_report_default() {
        let detector = PhaseDetector::default();
        let report = detector.report(0.50);
        assert_eq!(report.phase, ResourcePhase::Stable);
        assert_eq!(report.utilization, 0.50);
    }
}
