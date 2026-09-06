// arifFlow governance/tri_witness.rs
// F3 TRI-WITNESS — W³ consensus across Human × AI × External
//
// GAP P1-3: TRI_WITNESS merge for fan-out topologies.
// Nash (1950) bargaining product: W³ = ∛(h × ai × ext)
// Zero in any channel collapses consensus → DIVERGENT.
//
// Invariant W1: All three channels must be present.
// Invariant W2: Zero in any channel → consensus DIVERGENT (never fake 0.5).
// Invariant W3: W³ ≥ 0.75 → CONSENSUS; 0.50 ≤ W³ < 0.75 → WEAK; < 0.50 → DIVERGENT.

use serde::{Deserialize, Serialize};

/// W³ consensus verdict
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriWitnessVerdict {
    /// W³ ≥ 0.75 — all three witnesses substantially agree
    Consensus,
    /// 0.50 ≤ W³ < 0.75 — agreement exists but weak
    Weak,
    /// W³ < 0.50 — witnesses diverge; 888_HOLD required
    Divergent,
    /// One or more channels missing — incomplete
    Incomplete,
}

impl TriWitnessVerdict {
    pub fn is_consensus(&self) -> bool {
        matches!(self, TriWitnessVerdict::Consensus)
    }

    pub fn is_divergent(&self) -> bool {
        matches!(self, TriWitnessVerdict::Divergent)
    }

    pub fn requires_hold(&self) -> bool {
        matches!(
            self,
            TriWitnessVerdict::Divergent | TriWitnessVerdict::Incomplete
        )
    }
}

/// A single witness channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessChannel {
    /// Confidence in [0.0, 1.0]
    pub confidence: f64,
    /// Source identifier (actor, organ, dataset)
    pub source: String,
    /// Evidence summary
    pub evidence: String,
}

impl WitnessChannel {
    pub fn new(confidence: f64, source: impl Into<String>, evidence: impl Into<String>) -> Self {
        Self {
            confidence: confidence.clamp(0.0, 1.0),
            source: source.into(),
            evidence: evidence.into(),
        }
    }

    /// Unknown witness — explicitly zero confidence (never 0.5)
    pub fn unknown(source: impl Into<String>) -> Self {
        Self {
            confidence: 0.0,
            source: source.into(),
            evidence: "NO_EVIDENCE".into(),
        }
    }
}

/// F3 TRI-WITNESS — three independent witness channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriWitness {
    /// Human witness (sovereign, operator, auditor)
    pub human: WitnessChannel,
    /// AI witness (model, computation, reasoning)
    pub ai: WitnessChannel,
    /// External/Earth witness (data, ground truth, physics)
    pub external: WitnessChannel,
}

impl TriWitness {
    /// Create from three channels. Returns None if any channel is missing.
    pub fn new(human: WitnessChannel, ai: WitnessChannel, external: WitnessChannel) -> Self {
        Self {
            human,
            ai,
            external,
        }
    }

    /// Compute W³ = ∛(h × ai × ext) — Nash bargaining product
    ///
    /// Returns 0.0 if any channel has zero confidence.
    pub fn w3_score(&self) -> f64 {
        let h = self.human.confidence;
        let a = self.ai.confidence;
        let e = self.external.confidence;

        // W2: zero in any channel collapses consensus
        if h == 0.0 || a == 0.0 || e == 0.0 {
            return 0.0;
        }

        // Geometric mean: ∛(h × a × e)
        (h * a * e).cbrt()
    }

    /// Evaluate consensus verdict from W³ score
    pub fn evaluate(&self) -> TriWitnessVerdict {
        let score = self.w3_score();
        if score >= 0.75 {
            TriWitnessVerdict::Consensus
        } else if score >= 0.50 {
            TriWitnessVerdict::Weak
        } else if score > 0.0 {
            TriWitnessVerdict::Divergent
        } else {
            TriWitnessVerdict::Incomplete
        }
    }

    /// Human-readable summary
    pub fn summary(&self) -> String {
        let score = self.w3_score();
        let verdict = self.evaluate();
        format!(
            "W³={:.3} ({:?}) h={:.2} ai={:.2} ext={:.2}",
            score, verdict, self.human.confidence, self.ai.confidence, self.external.confidence
        )
    }
}

/// Result of merging multiple witness attestations (fan-out merge)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessMergeResult {
    /// Aggregated tri-witness from all lanes
    pub aggregate: TriWitness,
    /// Per-lane witness attestations
    pub per_lane: Vec<(String, TriWitness)>,
    /// W³ score
    pub w3_score: f64,
    /// Verdict
    pub verdict: TriWitnessVerdict,
    /// Whether merge passed (consensus or weak)
    pub merged: bool,
}

impl WitnessMergeResult {
    /// Merge multiple lane witnesses using the MINIMUM confidence per channel
    /// (conservative — weakest link determines aggregate).
    pub fn merge(lane_witnesses: Vec<(String, TriWitness)>) -> Self {
        if lane_witnesses.is_empty() {
            return Self {
                aggregate: TriWitness::new(
                    WitnessChannel::unknown("human"),
                    WitnessChannel::unknown("ai"),
                    WitnessChannel::unknown("external"),
                ),
                per_lane: vec![],
                w3_score: 0.0,
                verdict: TriWitnessVerdict::Incomplete,
                merged: false,
            };
        }

        // Conservative merge: MIN confidence across all lanes per channel
        let min_h = lane_witnesses
            .iter()
            .map(|(_, w)| w.human.confidence)
            .fold(1.0, f64::min);
        let min_ai = lane_witnesses
            .iter()
            .map(|(_, w)| w.ai.confidence)
            .fold(1.0, f64::min);
        let min_ext = lane_witnesses
            .iter()
            .map(|(_, w)| w.external.confidence)
            .fold(1.0, f64::min);

        let aggregate = TriWitness::new(
            WitnessChannel::new(
                min_h,
                "merge:min_human",
                "Minimum human confidence across all lanes",
            ),
            WitnessChannel::new(
                min_ai,
                "merge:min_ai",
                "Minimum AI confidence across all lanes",
            ),
            WitnessChannel::new(
                min_ext,
                "merge:min_external",
                "Minimum external confidence across all lanes",
            ),
        );

        let w3_score = aggregate.w3_score();
        let verdict = aggregate.evaluate();

        Self {
            aggregate,
            per_lane: lane_witnesses,
            w3_score,
            verdict,
            merged: verdict == TriWitnessVerdict::Consensus || verdict == TriWitnessVerdict::Weak,
        }
    }

    /// Resolve consensus with risk-aware tiebreaker (Fix 1).
    pub fn resolve_with_tiebreaker(
        &self,
        risk_class: &crate::receipt::RiskClass,
        candidates: &[AgentCandidate],
    ) -> ConsensusResolution {
        resolve_consensus_with_tiebreaker(self.w3_score, self.verdict, risk_class, candidates)
    }
}

/// Candidate agent with its current Flow Quotient (FQ).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentCandidate {
    pub agent_id: String,
    pub fq: f64,
}

impl AgentCandidate {
    pub fn new(agent_id: impl Into<String>, fq: f64) -> Self {
        Self {
            agent_id: agent_id.into(),
            fq,
        }
    }

    /// Distance from ideal Flow Quotient (1.0).
    /// Lower distance indicates balance between execution and verification.
    pub fn fq_distance_from_ideal(&self) -> f64 {
        (self.fq - 1.0).abs()
    }
}

/// Outcome of consensus resolution under risk-aware tiebreaker policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsensusResolution {
    pub w3_score: f64,
    pub verdict: TriWitnessVerdict,
    pub tiebreaker_applied: bool,
    pub selected_agent: Option<String>,
    pub hold_enforced: bool,
    pub reason: String,
}

/// Auto-tiebreaker resolution (Fix 1):
/// If W³ < 0.80 and Task_Risk == NON_CRITICAL (i.e. not T3Irreversible),
/// auto-select the agent with Flow Quotient (FQ) nearest to 1.0 as tie-breaker
/// instead of stalling on HOLD.
/// Reserve 888_HOLD exclusively for irreversible mutations (T3).
pub fn resolve_consensus_with_tiebreaker(
    w3_score: f64,
    verdict: TriWitnessVerdict,
    risk_class: &crate::receipt::RiskClass,
    candidates: &[AgentCandidate],
) -> ConsensusResolution {
    let is_critical = matches!(risk_class, crate::receipt::RiskClass::T3Irreversible);

    // Strong consensus (W³ >= 0.80) and consensus verdict
    if w3_score >= 0.80 && verdict.is_consensus() {
        return ConsensusResolution {
            w3_score,
            verdict,
            tiebreaker_applied: false,
            selected_agent: candidates.first().map(|c| c.agent_id.clone()),
            hold_enforced: false,
            reason: format!("Strong W³ consensus ({:.3} >= 0.80)", w3_score),
        };
    }

    // Critical mutation (T3 / Irreversible) requires strict W³ >= 0.80; else 888_HOLD
    if is_critical {
        let hold = w3_score < 0.80 || verdict.requires_hold();
        return ConsensusResolution {
            w3_score,
            verdict,
            tiebreaker_applied: false,
            selected_agent: if hold { None } else { candidates.first().map(|c| c.agent_id.clone()) },
            hold_enforced: hold,
            reason: if hold {
                format!(
                    "888_HOLD: W³={:.3} < 0.80 on critical irreversible mutation ({})",
                    w3_score,
                    risk_class.code()
                )
            } else {
                format!("Critical mutation cleared W³={:.3} >= 0.80", w3_score)
            },
        };
    }

    // NON-CRITICAL task: if W³ < 0.80, apply auto-tiebreaker using FQ nearest to 1.0
    if !candidates.is_empty() {
        let mut best = &candidates[0];
        let mut min_dist = best.fq_distance_from_ideal();

        for cand in &candidates[1..] {
            let dist = cand.fq_distance_from_ideal();
            if dist < min_dist {
                min_dist = dist;
                best = cand;
            }
        }

        ConsensusResolution {
            w3_score,
            verdict,
            tiebreaker_applied: true,
            selected_agent: Some(best.agent_id.clone()),
            hold_enforced: false,
            reason: format!(
                "Auto-tiebreaker selected '{}' (|FQ {:.3} - 1.0| = {:.3}) under non-critical risk ({}) with W³={:.3}",
                best.agent_id, best.fq, min_dist, risk_class.code(), w3_score
            ),
        }
    } else {
        ConsensusResolution {
            w3_score,
            verdict,
            tiebreaker_applied: false,
            selected_agent: None,
            hold_enforced: verdict.requires_hold(),
            reason: "W³ < 0.80 but no agent candidates provided for tiebreaker".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn witness(conf: f64, src: &str) -> WitnessChannel {
        WitnessChannel::new(conf, src, format!("evidence from {}", src))
    }

    #[test]
    fn test_w3_full_consensus() {
        let tw = TriWitness::new(
            witness(0.95, "arif"),
            witness(0.90, "ai"),
            witness(0.85, "geo"),
        );
        let score = tw.w3_score();
        assert!(
            score >= 0.75,
            "Full consensus should be >= 0.75, got {:.3}",
            score
        );
        assert_eq!(tw.evaluate(), TriWitnessVerdict::Consensus);
    }

    #[test]
    fn test_w3_zero_collapses() {
        let tw = TriWitness::new(
            witness(0.95, "arif"),
            witness(0.90, "ai"),
            witness(0.0, "geo"),
        );
        assert_eq!(tw.w3_score(), 0.0, "Zero in any channel should collapse");
        assert_eq!(tw.evaluate(), TriWitnessVerdict::Incomplete);
    }

    #[test]
    fn test_w3_weak_consensus() {
        let tw = TriWitness::new(
            witness(0.70, "arif"),
            witness(0.70, "ai"),
            witness(0.50, "geo"),
        );
        let score = tw.w3_score();
        assert!(
            score >= 0.50 && score < 0.75,
            "Weak should be [0.50, 0.75), got {:.3}",
            score
        );
        assert_eq!(tw.evaluate(), TriWitnessVerdict::Weak);
    }

    #[test]
    fn test_w3_divergent() {
        let tw = TriWitness::new(
            witness(0.30, "arif"),
            witness(0.60, "ai"),
            witness(0.40, "geo"),
        );
        let score = tw.w3_score();
        assert!(
            score < 0.50 && score > 0.0,
            "Divergent should be < 0.50, got {:.3}",
            score
        );
        assert_eq!(tw.evaluate(), TriWitnessVerdict::Divergent);
    }

    #[test]
    fn test_w3_unknown_channel() {
        let unknown = WitnessChannel::unknown("human");
        assert_eq!(unknown.confidence, 0.0);
        assert_eq!(unknown.evidence, "NO_EVIDENCE");
    }

    #[test]
    fn test_merge_min_confidence() {
        let lane1 = TriWitness::new(
            witness(0.90, "h1"),
            witness(0.80, "ai1"),
            witness(0.70, "e1"),
        );
        let lane2 = TriWitness::new(
            witness(0.60, "h2"),
            witness(0.95, "ai2"),
            witness(0.85, "e2"),
        );

        let result =
            WitnessMergeResult::merge(vec![("lane1".into(), lane1), ("lane2".into(), lane2)]);

        // Conservative: min per channel
        assert_eq!(
            result.aggregate.human.confidence, 0.60,
            "Should use min human"
        );
        assert_eq!(result.aggregate.ai.confidence, 0.80, "Should use min ai");
        assert_eq!(
            result.aggregate.external.confidence, 0.70,
            "Should use min external"
        );
        assert!(result.merged, "Merge should succeed with min confidences");
    }

    #[test]
    fn test_merge_empty_fails() {
        let result = WitnessMergeResult::merge(vec![]);
        assert!(!result.merged);
        assert_eq!(result.verdict, TriWitnessVerdict::Incomplete);
    }

    #[test]
    fn test_divergent_requires_hold() {
        assert!(TriWitnessVerdict::Divergent.requires_hold());
        assert!(TriWitnessVerdict::Incomplete.requires_hold());
        assert!(!TriWitnessVerdict::Consensus.requires_hold());
        assert!(!TriWitnessVerdict::Weak.requires_hold());
    }

    #[test]
    fn test_tiebreaker_non_critical_w3_below_80() {
        use crate::receipt::RiskClass;

        let candidates = vec![
            AgentCandidate::new("agent-a", 0.50), // dist = 0.50
            AgentCandidate::new("agent-b", 1.05), // dist = 0.05 (nearest to 1.0)
            AgentCandidate::new("agent-c", 1.80), // dist = 0.80
        ];

        // Non-critical task (T1Mutate) with W³ = 0.72 (< 0.80)
        let res = resolve_consensus_with_tiebreaker(
            0.72,
            TriWitnessVerdict::Weak,
            &RiskClass::T1Mutate,
            &candidates,
        );

        assert!(res.tiebreaker_applied, "Tiebreaker should be applied when W³ < 0.80 on non-critical task");
        assert!(!res.hold_enforced, "HOLD should NOT be enforced for non-critical task with tiebreaker");
        assert_eq!(res.selected_agent.as_deref(), Some("agent-b"), "Should select agent with FQ nearest to 1.0");
    }

    #[test]
    fn test_tiebreaker_reserved_hold_for_critical_t3() {
        use crate::receipt::RiskClass;

        let candidates = vec![
            AgentCandidate::new("agent-b", 1.02),
        ];

        // Critical irreversible mutation (T3Irreversible) with W³ = 0.72 (< 0.80)
        let res = resolve_consensus_with_tiebreaker(
            0.72,
            TriWitnessVerdict::Weak,
            &RiskClass::T3Irreversible,
            &candidates,
        );

        assert!(!res.tiebreaker_applied, "Tiebreaker should NOT bypass critical mutation requirement");
        assert!(res.hold_enforced, "888_HOLD MUST be enforced on critical mutation with W³ < 0.80");
        assert_eq!(res.selected_agent, None, "No agent should be auto-selected under 888_HOLD");
        assert!(res.reason.starts_with("888_HOLD"), "Reason must clearly cite 888_HOLD");
    }

    #[test]
    fn test_consensus_cleared_when_w3_ge_80() {
        use crate::receipt::RiskClass;

        let candidates = vec![
            AgentCandidate::new("primary-agent", 1.0),
        ];

        // Critical task with high consensus W³ = 0.92 (>= 0.80)
        let res = resolve_consensus_with_tiebreaker(
            0.92,
            TriWitnessVerdict::Consensus,
            &RiskClass::T3Irreversible,
            &candidates,
        );

        assert!(!res.tiebreaker_applied);
        assert!(!res.hold_enforced);
        assert_eq!(res.selected_agent.as_deref(), Some("primary-agent"));
    }
}

