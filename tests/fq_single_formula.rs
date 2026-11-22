//! One formula, one source of truth (P1, 2026-11-22).
//!
//! The federation carried two copies of the FQ band table: one in
//! `receipt.rs` (system window) and one in `governance/invariants.rs`
//! (per-actor enforcer). Two copies of a threshold table is how a health
//! surface starts lying — change one, ship it, and two consumers of the same
//! metric disagree about the same actor.
//!
//! `receipt::classify_quotient` is now the canonical decision. This test
//! asserts the per-actor path agrees with it across the whole boundary
//! matrix, so the two can never silently diverge again.

use arifflow::governance::invariants::ActorFlowState;
use arifflow::receipt::{
    classify_quotient, EpistemicLabel, FlowReceipt, RiskClass, StepType,
};

fn rc(actor: &str, step: StepType) -> FlowReceipt {
    let mut r = FlowReceipt::new_first(
        actor,
        "s1",
        step,
        EpistemicLabel::Observation,
        1000,
    );
    r.risk_class = RiskClass::T1Mutate;
    r
}

#[test]
fn per_actor_band_matches_the_canonical_function() {
    for exec in 0..5usize {
        for ver in 0..7usize {
            let mut st = ActorFlowState::new("matrix");
            for _ in 0..exec {
                st.ingest(&rc("matrix", StepType::Execute));
            }
            for _ in 0..ver {
                st.ingest(&rc("matrix", StepType::Verify));
            }
            assert_eq!(
                st.verdict,
                classify_quotient(st.verify_count, st.execute_count),
                "band drift at exec={exec} verify={ver}"
            );
        }
    }
}

#[test]
fn canonical_function_boundaries_are_explicit() {
    use arifflow::receipt::FlowVerdict;
    assert_eq!(classify_quotient(0, 0), FlowVerdict::Unknown);
    assert_eq!(classify_quotient(1, 5), FlowVerdict::Caution);
    assert_eq!(classify_quotient(3, 0), FlowVerdict::Flowing);
    assert_eq!(classify_quotient(4, 1), FlowVerdict::Fossilized);
    assert_eq!(classify_quotient(2, 2), FlowVerdict::Optimal);
    assert_eq!(classify_quotient(3, 4), FlowVerdict::Flowing);
    assert_eq!(classify_quotient(2, 10), FlowVerdict::Stuck);
    assert_eq!(classify_quotient(2, 100), FlowVerdict::Burning); // verify>=2, q=0.02
    assert_eq!(classify_quotient(1, 100), FlowVerdict::Caution); // one verify = coincidence, not pattern
}
