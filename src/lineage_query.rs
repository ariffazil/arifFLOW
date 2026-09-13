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
                .map(|pos| boundary.map_or(true, |b| pos <= b))
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
            .filter(|&p| boundary.map_or(true, |b| p <= b))
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
