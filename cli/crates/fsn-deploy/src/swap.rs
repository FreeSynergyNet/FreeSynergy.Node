// SwapPlanner — compatibility check and plan generation for service swaps.
//
// A "swap" replaces one running service instance with another service class
// while preserving data and minimising downtime.
//
// Design:
//   - `SwapPlanner::check_compatibility` compares capability sets and produces
//     a `SwapCompatibility` report (shared capabilities + warnings for gaps).
//   - `SwapPlanner::plan` wraps both IDs + the compatibility report into a
//     `SwapPlan` ready for the deploy engine to execute.

use serde::{Deserialize, Serialize};

// ── Compatibility types ───────────────────────────────────────────────────────

/// A capability that is present in the source service but absent from the target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWarning {
    /// Capability identifier (e.g. `"iam_scim"`).
    pub capability: String,
    /// Human-readable explanation of the impact.
    pub message: String,
}

/// Result of comparing two capability sets before a swap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapCompatibility {
    /// Capabilities present in both the source and the target (safe to swap).
    pub compatible_capabilities: Vec<String>,
    /// Capabilities that the source provides but the target does not.
    /// The swap can proceed, but operators should review the warnings.
    pub warnings: Vec<CapabilityWarning>,
}

impl SwapCompatibility {
    /// Returns `true` when no capability gaps were detected.
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }
}

// ── Swap Plan ────────────────────────────────────────────────────────────────

/// A complete, validated swap plan produced by `SwapPlanner::plan`.
///
/// The deploy engine reads `SwapPlan` to orchestrate the swap sequence:
///   1. Run `on_swap` hooks on the source instance.
///   2. Install and configure the target instance.
///   3. Perform health checks on the target.
///   4. Reroute proxy traffic to the target.
///   5. Decommission the source instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwapPlan {
    /// Unique instance ID of the service being replaced.
    pub source_id: String,
    /// Unique instance ID of the service being installed as a replacement.
    pub target_id: String,
    /// Compatibility analysis between source and target capabilities.
    pub compatibility: SwapCompatibility,
}

impl SwapPlan {
    /// Returns `true` when the swap has no capability warnings and can proceed
    /// without operator review.
    pub fn is_clean(&self) -> bool {
        self.compatibility.is_clean()
    }
}

// ── Planner ───────────────────────────────────────────────────────────────────

/// Builds and validates service swap plans.
#[derive(Debug, Clone, Default)]
pub struct SwapPlanner;

impl SwapPlanner {
    /// Compare two capability slices and return a `SwapCompatibility` report.
    ///
    /// - Capabilities present in both sets are listed as `compatible_capabilities`.
    /// - Capabilities only in `source_caps` produce a `CapabilityWarning` because
    ///   the replacement service cannot fulfil them.
    /// - Capabilities only in `target_caps` are silently ignored (additive).
    pub fn check_compatibility(
        &self,
        source_caps: &[String],
        target_caps: &[String],
    ) -> SwapCompatibility {
        let mut compatible_capabilities: Vec<String> = Vec::new();
        let mut warnings: Vec<CapabilityWarning> = Vec::new();

        for cap in source_caps {
            if target_caps.contains(cap) {
                compatible_capabilities.push(cap.clone());
            } else {
                warnings.push(CapabilityWarning {
                    capability: cap.clone(),
                    message: format!(
                        "The replacement service does not provide capability '{cap}'. \
                         Features relying on it may stop working after the swap."
                    ),
                });
            }
        }

        SwapCompatibility { compatible_capabilities, warnings }
    }

    /// Build a `SwapPlan` for replacing `source_id` with `target_id`.
    pub fn plan(
        &self,
        source_id: &str,
        source_caps: &[String],
        target_id: &str,
        target_caps: &[String],
    ) -> SwapPlan {
        let compatibility = self.check_compatibility(source_caps, target_caps);
        SwapPlan {
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            compatibility,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn clean_swap_when_all_caps_covered() {
        let planner = SwapPlanner;
        let plan = planner.plan(
            "kanidm-1",
            &caps(&["iam_oidc", "iam_ldap"]),
            "kanidm-2",
            &caps(&["iam_oidc", "iam_ldap", "iam_scim"]),
        );
        assert!(plan.is_clean());
        assert_eq!(plan.compatibility.compatible_capabilities.len(), 2);
    }

    #[test]
    fn warning_when_source_cap_missing_in_target() {
        let planner = SwapPlanner;
        let plan = planner.plan(
            "kanidm-1",
            &caps(&["iam_oidc", "iam_ldap"]),
            "authentik-1",
            &caps(&["iam_oidc"]),
        );
        assert!(!plan.is_clean());
        assert_eq!(plan.compatibility.warnings.len(), 1);
        assert_eq!(plan.compatibility.warnings[0].capability, "iam_ldap");
    }

    #[test]
    fn empty_source_caps_is_always_clean() {
        let planner = SwapPlanner;
        let plan = planner.plan("svc-a", &[], "svc-b", &caps(&["iam_oidc"]));
        assert!(plan.is_clean());
    }
}
