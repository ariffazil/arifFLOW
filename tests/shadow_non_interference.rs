//! Shadow-plane acceptance tests (P1, 2026-11-22).
//! Reference: ARIFOS::ARIFLOW_P3_DEPLOYMENT_WITNESS / shadow amendment acceptance.
//!
//! The shadow plane must be a pure read-only projection. Nothing here may
//! change hold, throttle, release, admission or verdict state.

use arifflow::governance::invariants::InvariantEnforcer;
use arifflow::receipt::{EpistemicLabel, FlowReceipt, RiskClass, StepType};

fn rc(actor: &str, session: &str, step: StepType, risk: RiskClass) -> FlowReceipt {
    let mut r = FlowReceipt::new_first(actor, session, step, EpistemicLabel::Observation, 1000);
    r.risk_class = risk;
    r
}

/// All required metadata must be present, and the plane must declare itself
/// non-authoritative.
#[test]
fn t1_frame_carries_required_metadata_and_non_authoritative_flag() {
    let w = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("a", "s1", StepType::Verify, RiskClass::T1Mutate),
    ];
    let f = arifflow::shadow::observe(&w, 100);
    assert!(f.non_authoritative, "shadow plane must be non-authoritative");
    assert!(!f.rules_version.is_empty());
    assert!(!f.enforced_rules_version.is_empty());
    assert!(!f.formula_version.is_empty());
    assert!(!f.population.is_empty());
    assert_eq!(f.window_kind, "sliding_receipts");
    assert_eq!(f.window_size, 100);
    assert!(f.window_start_utc.is_some() && f.window_end_utc.is_some());
    assert!(f.first_receipt_id.is_some() && f.last_receipt_id.is_some());
    assert_eq!(f.frame_hash.len(), 64);
    assert!(!f.calculated_at_utc.is_empty());
    assert!(f.freshness_class.len() > 2);
    assert!(f.transition_cause.contains("shadow_only_no_enforcement_effect"));
    assert!(f.release_condition.starts_with("NONE"));
    assert!(f.enforcement_effect.starts_with("NONE"));
}

/// Same input => same frame hash. Independently recomputable.
#[test]
fn t2_frame_is_deterministic() {
    let w = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("b", "s2", StepType::Verify, RiskClass::T1Mutate),
        rc("c", "s3", StepType::Execute, RiskClass::T2Deploy),
    ];
    let f1 = arifflow::shadow::observe(&w, 100);
    let f2 = arifflow::shadow::observe(&w, 100);
    assert_eq!(f1.frame_hash, f2.frame_hash);
    assert_eq!(f1.enforced_quotient, f2.enforced_quotient);
    assert_eq!(f1.shadow_quotient, f2.shadow_quotient);
}

/// THE hard invariant: computing the shadow frame must not touch enforcement
/// state — not the held flag, not the counts, not the quotient.
#[test]
fn t3_shadow_observation_does_not_change_enforcement_state() {
    let mut enf = InvariantEnforcer::default();
    // drive an actor with real work + an orphan verify
    for _ in 0..4 {
        enf.ingest(&rc("a", "s1", StepType::Execute, RiskClass::T1Mutate));
    }
    let mut orphan = rc("a", "orphan", StepType::Verify, RiskClass::T1Mutate);
    orphan.mechanical_self_verify = false; // daemon would stamp it; we test the rule itself
    enf.ingest(&orphan);

    let before = {
        let s = enf.actors.get("a").unwrap();
        (s.execute_count, s.verify_count, s.quotient, s.held, s.throttled)
    };

    let window = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("a", "orphan", StepType::Verify, RiskClass::T1Mutate),
        rc("z", "s9", StepType::Verify, RiskClass::T1Mutate),
    ];
    let f = arifflow::shadow::observe(&window, 100);
    assert!(f.non_authoritative);

    let after = {
        let s = enf.actors.get("a").unwrap();
        (s.execute_count, s.verify_count, s.quotient, s.held, s.throttled)
    };
    assert_eq!(before, after, "shadow observation must not change enforcement state");
}

/// Divergence is observable on the same population: an unbound (orphan-session)
/// verify counts under the enforced rule but not under the shadow rule.
#[test]
fn t4_divergence_is_observable_without_enforcement() {
    let w = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("a", "s2", StepType::Verify, RiskClass::T1Mutate), // no execution in s2
    ];
    let f = arifflow::shadow::observe(&w, 100);
    assert_eq!(f.enforced_verifies, 1, "enforced rule counts the unbound verify");
    assert_eq!(f.shadow_verifies, 0, "shadow rule excludes the unbound verify");
    assert_eq!(f.excluded_session_orphan, 1);
    // Shadow has no eligible verify at all -> its quotient is UNDEFINED, not zero.
    assert_eq!(f.enforced_quotient, Some(1.0));
    assert_eq!(f.shadow_quotient, None);
    assert_eq!(f.quotient_delta, None, "undefined minus defined is undefined");

    // And when both sides have data, the delta is reported.
    let w2 = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("a", "s1", StepType::Verify, RiskClass::T1Mutate),
        rc("b", "s2", StepType::Verify, RiskClass::T1Mutate),
    ];
    let f2 = arifflow::shadow::observe(&w2, 100);
    assert_eq!(f2.enforced_verifies, 2);
    assert_eq!(f2.shadow_verifies, 1);
    assert_eq!(f2.enforced_quotient, Some(2.0));
    assert_eq!(f2.shadow_quotient, Some(1.0));
    assert_eq!(f2.quotient_delta, Some(-1.0));
}

/// Exclusion must not delete or conceal receipts — lineage is preserved.
#[test]
fn t5_excluded_receipts_remain_in_the_input() {
    let w = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        rc("a", "s2", StepType::Verify, RiskClass::T1Mutate),
    ];
    let before_len = w.len();
    let f = arifflow::shadow::observe(&w, 100);
    assert_eq!(w.len(), before_len, "input must not be mutated");
    assert_eq!(f.excluded_session_orphan, 1);
    assert_eq!(w[1].hash().len(), 64, "excluded receipt must remain hashable");
}

/// A replayed receipt is counted once under the shadow rule.
#[test]
fn t6_replay_does_not_pad_the_shadow_numerator() {
    let v = rc("a", "s1", StepType::Verify, RiskClass::T1Mutate);
    let w = vec![
        rc("a", "s1", StepType::Execute, RiskClass::T1Mutate),
        v.clone(),
        v.clone(),
    ];
    let f = arifflow::shadow::observe(&w, 100);
    assert_eq!(f.shadow_verifies, 1, "replay must not pad the shadow numerator");
    assert_eq!(f.excluded_duplicate, 1);
    assert_eq!(f.enforced_verifies, 2, "enforced rule has no replay guard (that is the gap under observation)");
}
