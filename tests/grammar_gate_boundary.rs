//! GRAMMAR BOUNDARY TEST (2026-11-22 · kimi-code/FI-008)
//!
//! The defect this closes: the ingest path accepted a caller-supplied
//! `floor_verdict` (Pass|Caution|Hold|Void). In the receipt schema those
//! variants are documented as *888_HOLD* and a permanent critical block —
//! arifOS's constitutional words. An agent could therefore write arifOS's
//! judgement into the metabolism ledger.
//!
//! The rule now enforced at the DAEMON (not only at the MCP bridge):
//!   arifFlow's own gate state is authoritative when present, and the legacy
//!   `floor_verdict` is DERIVED from it. A caller's legacy value never wins.
//!
//! arifFlow may not emit SEAL / HOLD / SABAR / VOID. Those belong to arifOS.

use arifflow::grammar::MetabolicGateState;
use arifflow::receipt::FlowReceipt;

fn minimal_receipt_json(extra: &str) -> String {
    format!(
        r#"{{
            "receipt_id": "11111111-2222-3333-4444-555555555555",
            "created_at": "2026-11-22T05:00:00+00:00",
            "actor_id": "grammar-boundary-probe",
            "session_id": "grammar-boundary-probe-session",
            "step_type": "Verify",
            "step_number": 1,
            "cost_ns": 0,
            "epistemic_label": "Observation",
            "floor_verdict": "Pass",
            "cooling_decision": "None"
            {extra}
        }}"#
    )
}

// 1. The gate state is authoritative: a caller claiming "Pass" alongside
//    FLOW_BLOCKED must end up blocked.
#[test]
fn gate_state_overrides_caller_supplied_floor_verdict() {
    let raw = minimal_receipt_json(r#", "metabolic_gate_state": "FLOW_BLOCKED""#);
    let mut receipt: FlowReceipt = serde_json::from_str(&raw).expect("receipt parses");
    assert_eq!(
        receipt.metabolic_gate_state,
        Some(MetabolicGateState::FlowBlocked),
        "gate state must survive deserialisation"
    );
    receipt.normalise_gate_state();
    assert_eq!(
        receipt.floor_verdict,
        arifflow::receipt::FloorVerdict::Hold,
        "the caller's 'Pass' must not beat arifFlow's own gate state"
    );
}

// 2. Backwards compatibility: a legacy receipt with NO gate state still parses,
//    and its legacy verdict is left exactly as the caller sent it.
#[test]
fn legacy_receipt_without_gate_state_is_untouched() {
    let raw = minimal_receipt_json("");
    let mut receipt: FlowReceipt = serde_json::from_str(&raw).expect("legacy receipt parses");
    assert_eq!(receipt.metabolic_gate_state, None);
    receipt.normalise_gate_state();
    assert_eq!(
        receipt.floor_verdict,
        arifflow::receipt::FloorVerdict::Pass,
        "a receipt with no gate state keeps the caller's legacy verdict"
    );
}

// 3. A legacy receipt that DID carry a constitutional verdict keeps it — the
//    daemon does not rewrite history for receipts minted before this release.
#[test]
fn legacy_hold_survives_when_no_gate_state_present() {
    let raw = minimal_receipt_json("").replace(r#""floor_verdict": "Pass""#, r#""floor_verdict": "Hold""#);
    let mut receipt: FlowReceipt = serde_json::from_str(&raw).expect("legacy receipt parses");
    receipt.normalise_gate_state();
    assert_eq!(receipt.floor_verdict, arifflow::receipt::FloorVerdict::Hold);
}

// 4. Fail closed: an unknown or borrowed label must NOT become permission.
//    arifOS's "HOLD" is not arifFlow's vocabulary, and must fail closed.
#[test]
fn unknown_or_borrowed_label_fails_closed() {
    for borrowed in ["HOLD", "SEAL", "VOID", "SABAR", "maybe"] {
        let raw = minimal_receipt_json(&format!(r#", "metabolic_gate_state": "{}""#, borrowed));
        let mut receipt: FlowReceipt = serde_json::from_str(&raw).expect("receipt parses");
        assert_eq!(
            receipt.metabolic_gate_state,
            Some(MetabolicGateState::FlowBlocked),
            "'{}' must fail closed to FLOW_BLOCKED, never to permission",
            borrowed
        );
        receipt.normalise_gate_state();
        assert_eq!(receipt.floor_verdict, arifflow::receipt::FloorVerdict::Hold);
    }
}

// 5. The vocabulary is arifFlow's, and the label set is exactly four.
#[test]
fn gate_state_labels_are_exactly_arif_flow_s_own() {
    assert_eq!(MetabolicGateState::Allow.as_str(), "ALLOW");
    assert_eq!(MetabolicGateState::FlowBlocked.as_str(), "FLOW_BLOCKED");
    assert_eq!(MetabolicGateState::Throttled.as_str(), "THROTTLED");
    assert_eq!(MetabolicGateState::Cooldown.as_str(), "COOLDOWN");
    for label in ["ALLOW", "FLOW_BLOCKED", "THROTTLED", "COOLDOWN"] {
        assert_eq!(
            MetabolicGateState::from_label(label).as_str(),
            label,
            "canonical labels round-trip"
        );
    }
}

// 6. The builder derives the legacy field from the gate state — the two can
//    never disagree, whichever way the receipt was built.
#[test]
fn builder_derives_legacy_field_from_gate_state() {
    let raw = minimal_receipt_json(r#", "metabolic_gate_state": "THROTTLED""#);
    let receipt: FlowReceipt = serde_json::from_str(&raw).expect("receipt parses");
    let receipt = receipt.with_metabolic_gate_state(MetabolicGateState::Throttled);
    assert_eq!(receipt.floor_verdict, arifflow::receipt::FloorVerdict::Caution);
    assert_eq!(
        receipt.metabolic_gate_state,
        Some(MetabolicGateState::Throttled)
    );
}
