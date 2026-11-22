//! Adversarial FQ tests (WS5, 2026-11-22).
//!
//! These attack the specific failure class of a metabolic gate: an actor that
//! inflates the metric without doing the work.
//!
//! Rule under test (P3, 2026-11-22): a `Verify` receipt counts toward FQ
//! unless the daemon stamped it `mechanical_self_verify` — a Verify in a
//! session that never executed. The daemon already refuses those as
//! hold-release evidence, so counting them in the numerator created a
//! one-way ratchet into FOSSILIZED.

use arifflow::governance::invariants::ActorFlowState;
use arifflow::receipt::{
    EpistemicLabel, FlowQuotient, FlowReceipt, ReceiptStore, RiskClass, StepType,
};

fn receipt(actor: &str, session: &str, step: StepType, cost: u64) -> FlowReceipt {
    let mut r = FlowReceipt::new_first(
        actor,
        session,
        step,
        EpistemicLabel::Observation,
        cost,
    );
    r.risk_class = RiskClass::T1Mutate;
    r
}

/// A Verify in a session that never executed cannot witness work.
#[test]
fn session_orphan_verify_does_not_count_toward_fq() {
    let mut st = ActorFlowState::new("fi-008");
    let mut v = receipt("fi-008", "orphan-session", StepType::Verify, 10);
    v.mechanical_self_verify = true; // daemon-stamped orphan
    st.ingest(&v);
    assert_eq!(
        st.verify_count, 0,
        "session-orphan verify must NOT inflate the FQ numerator"
    );
    assert_eq!(st.excluded_verify_count, 1);
}

/// Padding: many orphan verifies must not move the actor toward FOSSILIZED.
#[test]
fn verify_padding_cannot_inflate_health() {
    let mut st = ActorFlowState::new("padder");
    for _ in 0..50 {
        let mut v = receipt("padder", "pad-session", StepType::Verify, 10);
        v.mechanical_self_verify = true;
        st.ingest(&v);
    }
    assert_eq!(st.verify_count, 0);
    assert_eq!(st.excluded_verify_count, 50);
    assert!(
        st.quotient.unwrap_or(0.0) <= 3.0,
        "padding must not reach the FOSSILIZED pole (q > 3.0)"
    );
}

/// The system-level window applies the same rule as the per-actor one.
#[test]
fn system_window_excludes_orphan_verifies() {
    let mut receipts = Vec::new();
    receipts.push(receipt("worker", "real-session", StepType::Execute, 100));
    for _ in 0..20 {
        let mut v = receipt("padder", "orphan-session", StepType::Verify, 10);
        v.mechanical_self_verify = true;
        receipts.push(v);
    }
    let fq = FlowQuotient::compute(&receipts);
    assert_eq!(
        fq.verify_count, 0,
        "orphan verifies must not enter the system-level numerator"
    );
    assert_eq!(fq.execute_count, 1);
}

/// A legitimate verify (session executed, not stamped orphan) DOES count —
/// the gate must not punish real witnesses.
#[test]
fn legitimate_verify_still_counts() {
    let mut st = ActorFlowState::new("worker");
    st.ingest(&receipt("worker", "real-session", StepType::Execute, 100));
    st.ingest(&receipt("worker", "real-session", StepType::Verify, 50));
    assert_eq!(st.execute_count, 1);
    assert_eq!(st.verify_count, 1);
    assert_eq!(st.excluded_verify_count, 0);
}

/// A forged chain link (wrong previous hash) is rejected by the daemon path.
#[test]
fn forged_previous_hash_is_rejected() {
    let mut store = ReceiptStore::new(100);
    store.push_force(receipt("agent", "s1", StepType::Execute, 10));
    let mut forged = receipt("agent", "s1", StepType::Verify, 10);
    forged.previous_receipt_hash = Some("sha256:deadbeef".to_string());
    assert!(store.push_chain_aware(forged).is_err());
}

/// Replay handling — CHARACTERIZATION TEST (known gap, pinned deliberately).
///
/// The directive for this workstream asked for "duplicate replays are
/// rejected". They are NOT, today: `ReceiptStore::push_chain_aware` accepts a
/// receipt whose chain link resolves, even if that exact receipt is already
/// stored. Dedup happens further downstream (the daemon keeps a
/// `duplicates.jsonl` in its persist dir).
///
/// This test pins the CURRENT behaviour so the gap cannot change silently in
/// either direction, and so a future fix flips it visibly. Changing the
/// daemon's ingest semantics to reject replays is a production behaviour
/// change: it needs a gated window (F13/888), not a commit.
#[test]
fn replayed_receipt_is_currently_accepted__known_gap() {
    let mut store = ReceiptStore::new(100);
    store.push_force(receipt("agent", "s1", StepType::Execute, 10));
    let r2 = FlowReceipt::new_chained(
        &store.all().last().unwrap().clone(),
        "agent",
        "s1",
        StepType::Verify,
        EpistemicLabel::Derivation,
        10,
    );
    assert!(store.push_chain_aware(r2.clone()).is_ok());
    // KNOWN GAP: the same receipt is accepted a second time.
    assert!(
        store.push_chain_aware(r2).is_ok(),
        "KNOWN GAP changed: replay rejection at push_chain_aware was implemented — \
         update the README known-gaps table and the CHANGELOG entry"
    );
}
