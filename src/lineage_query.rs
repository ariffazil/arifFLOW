//! lineage_query.rs — SEQ-N: belief-lineage queries over the receipt DAG.
//!
//! "What did we believe at seq N, and why?" answered from receipts alone:
//! - ancestry walk with per-edge hash verification (the *why*)
//! - as-of boundary filter (the *when* — receipts after the boundary are
//!   invisible, including their supersessions: true time-travel)
//! - supersession status (belief *death* — visible, non-deleting)
//!
//! Deliberately independent of the governance LineageResolver: that trait
//! machinery serves seal-binding arbitration; this module serves the
//! read-only query surface (daemon `POST /lineage`, MCP `flow_lineage`).

use crate::receipt::FlowReceipt;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Ledger loaded from the daemon's persisted JSONL (append order = time order).
pub struct LoadedLedger {
    pub receipts: Vec<FlowReceipt>,
    index: HashMap<String, usize>,
}

impl LoadedLedger {
    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut receipts = Vec::new();
        let mut index = HashMap::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Unknown/malformed lines are skipped, not fatal — the ledger is
            // append-only reality; a query must witness what parses.
            if let Ok(r) = serde_json::from_str::<FlowReceipt>(line) {
                index.insert(r.receipt_id.to_string(), receipts.len());
                receipts.push(r);
            }
        }
        Ok(Self { receipts, index })
    }

    pub fn get(&self, id: &str) -> Option<&FlowReceipt> {
        self.index.get(id).map(|&i| &self.receipts[i])
    }

    pub fn position(&self, id: &str) -> Option<usize> {
        self.index.get(id).copied()
    }

    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }
}

#[derive(Serialize, Clone)]
pub struct EdgeStatus {
    pub parent_id: String,
    pub claimed_hash: Option<String>,
    pub actual_hash: Option<String>,
    /// verified | mismatch | unverified | missing_parent
    pub verdict: String,
}

#[derive(Serialize)]
pub struct AncestryNode {
    pub receipt_id: String,
    pub created_at: String,
    pub actor_id: String,
    pub step_type: String,
    pub epistemic_label: String,
    pub routed_organ: Option<String>,
    pub jcs_body_hash: Option<String>,
    pub parent_receipt_ids: Vec<String>,
    pub supersedes_receipt_ids: Vec<String>,
    /// Edges from this node to each claimed parent, with verification status.
    pub edges: Vec<EdgeStatus>,
}

#[derive(Serialize)]
pub struct SupersessionClaim {
    pub receipt_id: String,
    pub created_at: String,
    pub actor_id: String,
    pub claimed_hash: Option<String>,
    pub actual_hash: Option<String>,
    pub verdict: String,
}

#[derive(Serialize)]
pub struct LineageReport {
    pub schema: String,
    pub target: AncestryNode,
    /// Inclusive as-of boundary (ledger position): receipts after it are
    /// invisible to this report. None = now.
    pub as_of_receipt_id: Option<String>,
    pub ancestry: Vec<AncestryNode>,
    pub nodes_visited: usize,
    pub depth_reached: usize,
    /// active | superseded — computed WITHIN the as-of window only.
    pub belief_status: String,
    pub superseded_by: Vec<SupersessionClaim>,
}

fn actual_hash_of(r: &FlowReceipt) -> Option<String> {
    r.jcs_body_hash
        .clone()
        .or_else(|| r.compute_jcs_body_hash().ok())
}

fn node_of(r: &FlowReceipt, ledger: &LoadedLedger, boundary: Option<usize>) -> AncestryNode {
    let edges = r
        .parent_receipt_ids
        .iter()
        .enumerate()
        .map(|(i, pid)| {
            let claimed = r.parent_receipt_hashes.get(i).cloned();
            let visible = ledger
                .position(pid)
                .map(|pos| !boundary.is_some_and(|b| pos > b))
                .unwrap_or(false);
            if !visible {
                return EdgeStatus {
                    parent_id: pid.clone(),
                    claimed_hash: claimed,
                    actual_hash: None,
                    verdict: "missing_parent".into(),
                };
            }
            let parent = ledger.get(pid).expect("position checked above");
            let actual = actual_hash_of(parent);
            let verdict = match (&claimed, &actual) {
                (Some(c), Some(a)) if c == a => "verified",
                (Some(_), Some(_)) => "mismatch",
                _ => "unverified",
            };
            EdgeStatus {
                parent_id: pid.clone(),
                claimed_hash: claimed,
                actual_hash: actual,
                verdict: verdict.into(),
            }
        })
        .collect();
    AncestryNode {
        receipt_id: r.receipt_id.to_string(),
        created_at: r.created_at.to_rfc3339(),
        actor_id: r.actor_id.clone(),
        step_type: format!("{}", r.step_type),
        epistemic_label: format!("{}", r.epistemic_label),
        routed_organ: r.routed_organ.clone(),
        jcs_body_hash: r.jcs_body_hash.clone(),
        parent_receipt_ids: r.parent_receipt_ids.clone(),
        supersedes_receipt_ids: r.supersedes_receipt_ids.clone(),
        edges,
    }
}

/// Reconstruct the belief lineage of `target_id`.
///
/// `before_receipt_id` sets an inclusive as-of boundary: only receipts at or
/// before that ledger position exist for this query — including for
/// supersession status (a death that happened after the boundary did not
/// happen yet, as far as this report is concerned).
pub fn lineage_report(
    ledger: &LoadedLedger,
    target_id: &str,
    before_receipt_id: Option<&str>,
) -> Result<LineageReport, String> {
    let boundary = match before_receipt_id {
        Some(bid) => Some(
            ledger
                .position(bid)
                .ok_or_else(|| format!("as-of receipt {} not found in ledger", bid))?,
        ),
        None => None,
    };
    let visible = |id: &str| -> Option<usize> {
        ledger
            .position(id)
            .filter(|&p| !boundary.is_some_and(|b| p > b))
    };

    let target_pos = visible(target_id).ok_or_else(|| {
        format!(
            "target receipt {} not found{}",
            target_id,
            boundary.map(|_| " within the as-of window").unwrap_or("")
        )
    })?;
    let target = &ledger.receipts[target_pos];

    // ── ancestry walk (BFS, visited-set, parents only within window) ──
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut ancestry: Vec<AncestryNode> = Vec::new();
    let mut depth_reached = 0usize;
    queue.push_back(target_id.to_string());
    visited.insert(target_id.to_string());
    let mut depth: HashMap<String, usize> = HashMap::new();
    depth.insert(target_id.to_string(), 0);
    while let Some(id) = queue.pop_front() {
        let node = ledger
            .get(&id)
            .ok_or_else(|| format!("walk error: {} vanished", id))?;
        ancestry.push(node_of(node, ledger, boundary));
        for pid in &node.parent_receipt_ids {
            if visited.contains(pid) {
                continue;
            }
            if visible(pid).is_some() {
                visited.insert(pid.clone());
                let d = depth[&id] + 1;
                depth_reached = depth_reached.max(d);
                depth.insert(pid.clone(), d);
                queue.push_back(pid.clone());
            }
        }
    }

    // ── belief death: who supersedes the target (within window)? ──
    let mut superseded_by = Vec::new();
    if !ledger.receipts.is_empty() {
        let scan_end = boundary
            .unwrap_or(ledger.receipts.len() - 1)
            .min(ledger.receipts.len() - 1);
        for r in &ledger.receipts[..=scan_end] {
            if let Some(i) = r.supersedes_receipt_ids.iter().position(|x| x == target_id) {
                let claimed = r.supersedes_receipt_hashes.get(i).cloned();
                let actual = actual_hash_of(target);
                let verdict = match (&claimed, &actual) {
                    (Some(c), Some(a)) if c == a => "verified",
                    (Some(_), Some(_)) => "mismatch",
                    _ => "unverified",
                };
                superseded_by.push(SupersessionClaim {
                    receipt_id: r.receipt_id.to_string(),
                    created_at: r.created_at.to_rfc3339(),
                    actor_id: r.actor_id.clone(),
                    claimed_hash: claimed,
                    actual_hash: actual,
                    verdict: verdict.into(),
                });
            }
        }
    }

    let belief_status = if superseded_by.is_empty() {
        "active"
    } else {
        "superseded"
    };

    Ok(LineageReport {
        schema: "arifflow.lineage-report/v1".into(),
        target: node_of(target, ledger, boundary),
        as_of_receipt_id: before_receipt_id.map(str::to_string),
        nodes_visited: ancestry.len(),
        depth_reached,
        ancestry,
        belief_status: belief_status.into(),
        superseded_by,
    })
}

#[derive(Serialize)]
pub struct GovEvent {
    pub receipt_id: String,
    pub created_at: String,
    pub actor_id: String,
    pub governance_event: String,
    pub mode: String,
    pub verdict: String,
    pub chain_id: String,
    pub judge_state_hash: String,
    pub seal_purpose: String,
    pub f13_ack: bool,
    /// active | superseded — a verdict revised by a later governance receipt
    /// is a governance belief death (RG-4 × SEQ-N junction).
    pub belief_status: String,
    pub superseded_by: Vec<SupersessionClaim>,
}

/// RG-4 (2026-09-13): list governance events — receipts whose payload carries
/// `governance_event` (fire-seal lane emits seal / seal_refused / bind_failed).
/// Supersession status is computed within the as-of window like lineage_report.
pub fn gov_events(
    ledger: &LoadedLedger,
    before_receipt_id: Option<&str>,
) -> Result<Vec<GovEvent>, String> {
    let boundary = match before_receipt_id {
        Some(bid) => Some(
            ledger
                .position(bid)
                .ok_or_else(|| format!("as-of receipt {} not found in ledger", bid))?,
        ),
        None => None,
    };
    let mut events = Vec::new();
    if ledger.receipts.is_empty() {
        return Ok(events);
    }
    let scan_end = boundary
        .unwrap_or(ledger.receipts.len() - 1)
        .min(ledger.receipts.len() - 1);
    for r in &ledger.receipts[..=scan_end] {
        let Some(pl) = r.payload.as_ref().and_then(|p| p.as_object()) else {
            continue;
        };
        let Some(ev) = pl.get("governance_event").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut superseded_by = Vec::new();
        for k in &ledger.receipts[..=scan_end] {
            if let Some(i) = k
                .supersedes_receipt_ids
                .iter()
                .position(|x| *x == r.receipt_id.to_string())
            {
                let claimed = k.supersedes_receipt_hashes.get(i).cloned();
                let actual = actual_hash_of(r);
                let verdict = match (&claimed, &actual) {
                    (Some(c), Some(a)) if c == a => "verified",
                    (Some(_), Some(_)) => "mismatch",
                    _ => "unverified",
                };
                superseded_by.push(SupersessionClaim {
                    receipt_id: k.receipt_id.to_string(),
                    created_at: k.created_at.to_rfc3339(),
                    actor_id: k.actor_id.clone(),
                    claimed_hash: claimed,
                    actual_hash: actual,
                    verdict: verdict.into(),
                });
            }
        }
        events.push(GovEvent {
            receipt_id: r.receipt_id.to_string(),
            created_at: r.created_at.to_rfc3339(),
            actor_id: r.actor_id.clone(),
            governance_event: ev.to_string(),
            mode: pl.get("mode").and_then(|v| v.as_str()).unwrap_or("").into(),
            verdict: pl
                .get("verdict")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            chain_id: pl
                .get("chain_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            judge_state_hash: pl
                .get("judge_state_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            seal_purpose: pl
                .get("seal_purpose")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            f13_ack: pl.get("f13_ack").and_then(|v| v.as_bool()).unwrap_or(false),
            belief_status: if superseded_by.is_empty() {
                "active".into()
            } else {
                "superseded".into()
            },
            superseded_by,
        });
    }
    Ok(events)
}

#[derive(Serialize)]
pub struct ScarPolicy {
    pub receipt_id: String,
    pub created_at: String,
    pub actor_id: String,
    pub policy_slug: String,
    pub scar_id: String,
    pub scar_fingerprint: String,
    pub enforcement_surface: String,
    pub enforcement_ref: String,
    pub policy_text: String,
    /// Edges to the event/belief receipts this policy was compressed FROM
    /// (causal parents) — "reality changed future behaviour" traversable.
    pub parent_receipt_ids: Vec<String>,
    pub belief_status: String,
    pub superseded_by: Vec<SupersessionClaim>,
}

/// RG-5 (2026-09-13): scar-bound policies — receipts whose payload carries
/// `scar_binding` (a policy compressed from a scar, citing scar id + the
/// surface where it is enforced). Policies revised by later policies show as
/// supersession: policy belief death, same as any other belief.
pub fn scar_policies(
    ledger: &LoadedLedger,
    before_receipt_id: Option<&str>,
) -> Result<Vec<ScarPolicy>, String> {
    let boundary = match before_receipt_id {
        Some(bid) => Some(
            ledger
                .position(bid)
                .ok_or_else(|| format!("as-of receipt {} not found in ledger", bid))?,
        ),
        None => None,
    };
    let mut policies = Vec::new();
    if ledger.receipts.is_empty() {
        return Ok(policies);
    }
    let scan_end = boundary
        .unwrap_or(ledger.receipts.len() - 1)
        .min(ledger.receipts.len() - 1);
    for r in &ledger.receipts[..=scan_end] {
        let Some(pl) = r.payload.as_ref().and_then(|p| p.as_object()) else {
            continue;
        };
        if !pl.contains_key("scar_binding") {
            continue;
        }
        let b = pl.get("scar_binding").cloned().unwrap_or_default();
        let g = |k: &str| b.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut superseded_by = Vec::new();
        for k in &ledger.receipts[..=scan_end] {
            if let Some(i) = k
                .supersedes_receipt_ids
                .iter()
                .position(|x| *x == r.receipt_id.to_string())
            {
                let claimed = k.supersedes_receipt_hashes.get(i).cloned();
                let actual = actual_hash_of(r);
                let verdict = match (&claimed, &actual) {
                    (Some(c), Some(a)) if c == a => "verified",
                    (Some(_), Some(_)) => "mismatch",
                    _ => "unverified",
                };
                superseded_by.push(SupersessionClaim {
                    receipt_id: k.receipt_id.to_string(),
                    created_at: k.created_at.to_rfc3339(),
                    actor_id: k.actor_id.clone(),
                    claimed_hash: claimed,
                    actual_hash: actual,
                    verdict: verdict.into(),
                });
            }
        }
        policies.push(ScarPolicy {
            receipt_id: r.receipt_id.to_string(),
            created_at: r.created_at.to_rfc3339(),
            actor_id: r.actor_id.clone(),
            policy_slug: g("policy_slug"),
            scar_id: g("scar_id"),
            scar_fingerprint: g("scar_fingerprint"),
            enforcement_surface: g("enforcement_surface"),
            enforcement_ref: g("enforcement_ref"),
            policy_text: g("policy_text"),
            parent_receipt_ids: r.parent_receipt_ids.clone(),
            belief_status: if superseded_by.is_empty() {
                "active".into()
            } else {
                "superseded".into()
            },
            superseded_by,
        });
    }
    Ok(policies)
}

#[derive(Serialize)]
pub struct ConsequenceRecord {
    pub receipt_id: String,
    pub created_at: String,
    pub actor_id: String,
    pub claim_slug: String,
    pub observed_outcome: String,
    pub evidence: String,
    /// recovery | regression | neutral — the direction reality moved.
    pub outcome_class: String,
    /// Causal parents: the policy/execution receipts this outcome is
    /// ATTRIBUTED to. Attribution is a claim, kept falsifiable via evidence.
    pub attributed_to: Vec<String>,
    pub belief_status: String,
    pub superseded_by: Vec<SupersessionClaim>,
}

/// RG-7 (2026-09-13): consequence records — reality's invoice as a
/// first-class graph object. A consequence receipt attributes an OBSERVED
/// outcome to the decision/execution receipts that produced it; the DAG edge
/// makes "did belief change reality?" traversable, and the evidence field
/// keeps the attribution falsifiable rather than narrative.
pub fn consequences(
    ledger: &LoadedLedger,
    before_receipt_id: Option<&str>,
) -> Result<Vec<ConsequenceRecord>, String> {
    let boundary = match before_receipt_id {
        Some(bid) => Some(
            ledger
                .position(bid)
                .ok_or_else(|| format!("as-of receipt {} not found in ledger", bid))?,
        ),
        None => None,
    };
    let mut records = Vec::new();
    if ledger.receipts.is_empty() {
        return Ok(records);
    }
    let scan_end = boundary
        .unwrap_or(ledger.receipts.len() - 1)
        .min(ledger.receipts.len() - 1);
    for r in &ledger.receipts[..=scan_end] {
        let Some(pl) = r.payload.as_ref().and_then(|p| p.as_object()) else {
            continue;
        };
        let Some(c) = pl.get("consequence").and_then(|v| v.as_object()) else {
            continue;
        };
        let g = |k: &str| c.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let mut superseded_by = Vec::new();
        for k in &ledger.receipts[..=scan_end] {
            if let Some(i) = k
                .supersedes_receipt_ids
                .iter()
                .position(|x| *x == r.receipt_id.to_string())
            {
                let claimed = k.supersedes_receipt_hashes.get(i).cloned();
                let actual = actual_hash_of(r);
                let verdict = match (&claimed, &actual) {
                    (Some(c2), Some(a)) if c2 == a => "verified",
                    (Some(_), Some(_)) => "mismatch",
                    _ => "unverified",
                };
                superseded_by.push(SupersessionClaim {
                    receipt_id: k.receipt_id.to_string(),
                    created_at: k.created_at.to_rfc3339(),
                    actor_id: k.actor_id.clone(),
                    claimed_hash: claimed,
                    actual_hash: actual,
                    verdict: verdict.into(),
                });
            }
        }
        records.push(ConsequenceRecord {
            receipt_id: r.receipt_id.to_string(),
            created_at: r.created_at.to_rfc3339(),
            actor_id: r.actor_id.clone(),
            claim_slug: g("claim_slug"),
            observed_outcome: g("observed_outcome"),
            evidence: g("evidence"),
            outcome_class: g("outcome_class"),
            attributed_to: r.parent_receipt_ids.clone(),
            belief_status: if superseded_by.is_empty() {
                "active".into()
            } else {
                "superseded".into()
            },
            superseded_by,
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receipt::{EpistemicLabel, StepType};

    fn write_ledger(dir: &std::path::Path, receipts: &[FlowReceipt]) -> LoadedLedger {
        let p = dir.join("receipts.jsonl");
        let mut s = String::new();
        for r in receipts {
            s.push_str(&serde_json::to_string(r).unwrap());
            s.push('\n');
        }
        std::fs::write(&p, s).unwrap();
        LoadedLedger::from_path(&p).unwrap()
    }

    fn stamp(mut r: FlowReceipt) -> FlowReceipt {
        r.jcs_body_hash = Some(r.compute_jcs_body_hash().unwrap());
        r
    }

    #[test]
    fn seqn_ancestry_verified_and_belief_death() {
        let dir = tempfile::tempdir().unwrap();
        let root = stamp(FlowReceipt::new_first(
            "t",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            1,
        ));
        let rid = root.receipt_id.to_string();
        let rhash = root.jcs_body_hash.clone().unwrap();
        let child = stamp(
            FlowReceipt::new_first("t", "s", StepType::Verify, EpistemicLabel::Observation, 1)
                .with_causal_parents(vec![rid.clone()], vec![rhash.clone()]),
        );
        let cid = child.receipt_id.to_string();
        let chash = child.jcs_body_hash.clone().unwrap();
        let killer = stamp(
            FlowReceipt::new_first("t", "s", StepType::Execute, EpistemicLabel::Derivation, 1)
                .with_supersedes(vec![cid.clone()], vec![chash.clone()]),
        );
        let ledger = write_ledger(dir.path(), &[root, child, killer]);

        // child ancestry: edges verified, includes root
        let rep = lineage_report(&ledger, &cid, None).unwrap();
        assert_eq!(rep.belief_status, "superseded");
        assert_eq!(rep.superseded_by.len(), 1);
        assert_eq!(rep.superseded_by[0].verdict, "verified");
        assert_eq!(rep.ancestry.len(), 2);
        let child_node = rep.ancestry.iter().find(|n| n.receipt_id == cid).unwrap();
        assert_eq!(child_node.edges[0].verdict, "verified");

        // TIME TRAVEL: as-of the child itself, the killer does not exist yet —
        // belief was still active at that point in history.
        let rep_past = lineage_report(&ledger, &cid, Some(&cid)).unwrap();
        assert_eq!(rep_past.belief_status, "active");
        assert!(rep_past.superseded_by.is_empty());
    }

    #[test]
    fn seqn_ancestry_detects_mismatch_and_missing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let root = stamp(FlowReceipt::new_first(
            "t",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            1,
        ));
        let rid = root.receipt_id.to_string();
        let rhash = root.jcs_body_hash.clone().unwrap();
        // child claims WRONG hash + references a parent that doesn't exist
        let child = stamp(
            FlowReceipt::new_first("t", "s", StepType::Verify, EpistemicLabel::Observation, 1)
                .with_causal_parents(
                    vec![rid.clone(), "ghost-id".into()],
                    vec!["0".repeat(64), "1".repeat(64)],
                ),
        );
        let cid = child.receipt_id.to_string();
        let ledger = write_ledger(dir.path(), &[root, child]);

        let rep = lineage_report(&ledger, &cid, None).unwrap();
        let node = rep.ancestry.iter().find(|n| n.receipt_id == cid).unwrap();
        assert_eq!(node.edges[0].verdict, "mismatch");
        assert_eq!(node.edges[1].verdict, "missing_parent");
    }

    #[test]
    fn rg7_consequences_attributed_and_as_of() {
        let dir = tempfile::tempdir().unwrap();
        let policy = stamp(FlowReceipt::new_first(
            "a",
            "s",
            StepType::Execute,
            EpistemicLabel::Specification,
            0,
        ));
        let pid = policy.receipt_id.to_string();
        let ph = policy.jcs_body_hash.clone().unwrap();
        let outcome = stamp(
            FlowReceipt::new_first("a", "s", StepType::Verify, EpistemicLabel::Observation, 0)
                .with_causal_parents(vec![pid.clone()], vec![ph]),
        );
        let mut cons = outcome.clone();
        cons.payload = Some(serde_json::json!({
            "consequence": {
                "claim_slug": "retry-recovered-400",
                "observed_outcome": "receipt ingested on retry",
                "evidence": "first attempt 400 EOF, retry 200",
                "outcome_class": "recovery"
            }
        }));
        let cons = stamp(cons);
        let cid = cons.receipt_id.to_string();
        let ledger = write_ledger(dir.path(), &[policy, cons]);
        let recs = consequences(&ledger, None).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].outcome_class, "recovery");
        assert_eq!(recs[0].attributed_to, vec![pid.clone()]);
        // as-of before the consequence: reality had not yet replied
        assert!(consequences(&ledger, Some(&pid)).unwrap().is_empty());
        let _ = cid;
    }

    #[test]
    fn rg5_scar_policies_scan_and_lineage() {
        let dir = tempfile::tempdir().unwrap();
        // the triggering event receipt (the "scar source" witness)
        let mut ev = FlowReceipt::new_first(
            "qwen-code/FI-003",
            "rg15-fi003",
            StepType::Verify,
            EpistemicLabel::Observation,
            0,
        );
        ev.payload = Some(serde_json::json!({"root_cause": "deploy race"}));
        let ev = stamp(ev);
        let evid = ev.receipt_id.to_string();
        let evh = ev.jcs_body_hash.clone().unwrap();
        // the policy compressed from it
        let mut pol = FlowReceipt::new_first(
            "qwen-code/FI-003",
            "rg5-witness",
            StepType::Execute,
            EpistemicLabel::Specification,
            0,
        )
        .with_causal_parents(vec![evid.clone()], vec![evh.clone()]);
        pol.payload = Some(serde_json::json!({
            "scar_binding": {
                "policy_slug": "retry-on-transient-400",
                "scar_id": "deploy-race-20260913",
                "enforcement_surface": "fire-seal.py",
                "enforcement_ref": "scripts 8210114",
                "policy_text": "emitters retry once after 1s on transient 4xx/EOF"
            }
        }));
        let pol = stamp(pol);
        let polid = pol.receipt_id.to_string();
        let ledger = write_ledger(dir.path(), &[ev, pol]);

        let policies = scar_policies(&ledger, None).unwrap();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].policy_slug, "retry-on-transient-400");
        assert_eq!(policies[0].parent_receipt_ids, vec![evid.clone()]);
        assert_eq!(policies[0].belief_status, "active");

        // as-of before the policy: not yet compressed
        let past = scar_policies(&ledger, Some(&evid)).unwrap();
        assert!(past.is_empty());
        let _ = polid;
    }

    #[test]
    fn rg4_gov_events_payload_scan_and_supersession() {
        let dir = tempfile::tempdir().unwrap();
        let mut g1 = FlowReceipt::new_first(
            "arif",
            "fire-seal-lane-a",
            StepType::Cool,
            EpistemicLabel::Observation,
            0,
        );
        g1.payload = Some(serde_json::json!({
            "governance_event": "seal_refused", "mode": "seal", "verdict": "HOLD",
            "chain_id": "cc_x", "f13_ack": false,
        }));
        let g1 = stamp(g1);
        let g1id = g1.receipt_id.to_string();
        let g1h = g1.jcs_body_hash.clone().unwrap();
        let killer = stamp(
            FlowReceipt::new_first(
                "arif",
                "fire-seal-lane-a",
                StepType::Seal,
                EpistemicLabel::Seal,
                0,
            )
            .with_supersedes(vec![g1id.clone()], vec![g1h]),
        );
        let filler = stamp(FlowReceipt::new_first(
            "t",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            1,
        ));
        let ledger = write_ledger(dir.path(), &[filler, g1, killer]);

        let events = gov_events(&ledger, None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].governance_event, "seal_refused");
        assert_eq!(events[0].verdict, "HOLD");
        assert_eq!(events[0].belief_status, "superseded");
        assert_eq!(events[0].superseded_by[0].verdict, "verified");

        // time travel: as-of the refusal itself, the override did not happen
        let past = gov_events(&ledger, Some(&g1id)).unwrap();
        assert_eq!(past.len(), 1);
        assert_eq!(past[0].belief_status, "active");
    }

    #[test]
    fn seqn_target_beyond_boundary_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let a = stamp(FlowReceipt::new_first(
            "t",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            1,
        ));
        let b = stamp(FlowReceipt::new_first(
            "t",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            1,
        ));
        let aid = a.receipt_id.to_string();
        let bid = b.receipt_id.to_string();
        let ledger = write_ledger(dir.path(), &[a, b]);
        assert!(lineage_report(&ledger, &bid, Some(&aid)).is_err());
    }
}
