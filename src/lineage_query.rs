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
use chrono;
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

#[derive(Serialize)]
pub struct GraphConnectivity {
    /// parent_receipt_ids entries examined across the scanned ledger
    pub edges_total: usize,
    /// both endpoints organ-tagged and the canonical organ key matches
    pub same_organ_edges: usize,
    /// both endpoints organ-tagged and the canonical organ keys differ —
    /// the only edges that make the graph a FEDERATION graph rather than
    /// N private lane threads
    pub cross_organ_edges: usize,
    /// edges that cannot be classified: parent outside the scan window,
    /// or either endpoint carrying no routed_organ
    pub unclassifiable_edges: usize,
    /// distinct raw `routed_organ` strings seen in the ledger
    pub organ_labels_raw: usize,
    /// distinct canonical organ keys — raw minus spelling variants
    pub organ_keys_canonical: usize,
    /// connected components over canonical organ keys; 1 = the organs form
    /// a single causal graph, N = N islands
    pub organ_components: usize,
    /// heaviest cross-organ flows, "parent → child", descending
    pub top_cross_organ_pairs: Vec<String>,
    pub note: String,
}

/// Canonical organ equivalence key (2026-10-01). `routed_organ` is free text,
/// and the ledger had already fragmented into 18 labels for ~9 organs
/// (`AAA`/`aaa`, `arifOS`/`arifos`, `A-FORGE`/`aforge`, `WELL`/`well`…), so
/// organ-level traversal was impossible: two spellings of one organ were two
/// graph nodes.
///
/// Deliberately NOT a name table: arifFlow does not own organ names (AAA's
/// organs.yaml is the SOT). This is a deterministic normalisation that
/// collapses spelling variants into one equivalence class while leaving the
/// stored value untouched — read-side only, no history rewritten, no new
/// authority claimed.
pub fn canonical_organ_key(raw: &str) -> String {
    raw.chars()
        .filter(|c| *c != '-' && *c != '_' && *c != ' ')
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[derive(Serialize)]
pub struct FqGraphReport {
    pub schema: String,
    pub receipts_scanned: usize,
    pub beliefs_born: usize,
    pub beliefs_superseded: usize,
    pub supersession_events: usize,
    pub governance_events: usize,
    pub policies_emitted: usize,
    pub invoices_received: usize,
    /// supersession_events / beliefs_born — fraction of witnessed beliefs
    /// that died. Sample-size honesty: meaningless below ~30 beliefs.
    pub revision_rate: f64,
    /// invoices / policies — did compressed scars ever get billed by reality?
    pub invoice_yield: f64,
    /// belief born → belief died, milliseconds (per supersession, from
    /// target created_at to killer created_at).
    pub belief_lifetime_ms: Vec<u64>,
    /// scar-source event → policy, milliseconds (policy causal parents).
    pub scar_to_policy_ms: Vec<u64>,
    /// policy → invoice, milliseconds (consequence attributed_to).
    pub policy_to_invoice_ms: Vec<u64>,
    /// Federation connectivity of the causal DAG, measured over canonical
    /// organ keys. Added 2026-10-01 because until then inter-organ edge
    /// density was unmeasurable by any live surface — computing it required
    /// an ad-hoc 94k-line scan by whoever happened to ask.
    pub connectivity: GraphConnectivity,
    pub note: String,
}

fn age_ms(from: &str, to: &str) -> Option<u64> {
    let f = chrono::DateTime::parse_from_rfc3339(from).ok()?;
    let t = chrono::DateTime::parse_from_rfc3339(to).ok()?;
    (t.timestamp_millis() - f.timestamp_millis())
        .try_into()
        .ok()
}

/// Federation connectivity census. Walks every causal parent edge once and
/// classifies it by canonical organ key, then unions organ keys so the report
/// can state how many isolated organ-islands the graph actually has.
fn measure_connectivity(ledger: &LoadedLedger) -> GraphConnectivity {
    let by_id: HashMap<String, &FlowReceipt> = ledger
        .receipts
        .iter()
        .map(|r| (r.receipt_id.to_string(), r))
        .collect();
    let mut edges_total = 0usize;
    let mut same_organ = 0usize;
    let mut cross_organ = 0usize;
    let mut unclassifiable = 0usize;
    let mut labels_raw: HashMap<String, usize> = HashMap::new();
    let mut keys: HashMap<String, usize> = HashMap::new();
    let mut pair_counts: HashMap<String, usize> = HashMap::new();
    let mut uf: HashMap<String, String> = HashMap::new();

    fn find(uf: &mut HashMap<String, String>, x: &str) -> String {
        if !uf.contains_key(x) {
            uf.insert(x.to_string(), x.to_string());
        }
        let mut root = x.to_string();
        loop {
            let next = uf.get(&root).cloned().unwrap_or_else(|| root.clone());
            if next == root {
                break;
            }
            root = next;
        }
        // path compression
        let mut cur = x.to_string();
        while let Some(n) = uf.get(&cur).cloned() {
            if n == root {
                break;
            }
            uf.insert(cur.clone(), root.clone());
            cur = n;
        }
        root
    }

    for r in &ledger.receipts {
        if let Some(o) = r.routed_organ.as_deref() {
            *labels_raw.entry(o.to_string()).or_insert(0) += 1;
            *keys.entry(canonical_organ_key(o)).or_insert(0) += 1;
        }
        for pid in r.parent_receipt_ids.iter() {
            edges_total += 1;
            let parent = match by_id.get(pid.as_str()) {
                Some(p) => *p,
                None => {
                    unclassifiable += 1;
                    continue;
                }
            };
            let (pk, ck) = match (parent.routed_organ.as_deref(), r.routed_organ.as_deref()) {
                (Some(a), Some(b)) => (canonical_organ_key(a), canonical_organ_key(b)),
                _ => {
                    unclassifiable += 1;
                    continue;
                }
            };
            if pk == ck {
                same_organ += 1;
            } else {
                cross_organ += 1;
                *pair_counts.entry(format!("{} → {}", pk, ck)).or_insert(0) += 1;
            }
            // union the two organ keys (same-organ edges are single-node)
            if pk != ck {
                let a = find(&mut uf, &pk);
                let b = find(&mut uf, &ck);
                if a != b {
                    uf.insert(a, b);
                }
            } else {
                find(&mut uf, &pk);
            }
        }
    }
    // Every organ that ever appears in the ledger is a node, even if no edge
    // ever touched it — otherwise a fully isolated organ would be invisible
    // and the island count would lie.
    for k in keys.keys() {
        find(&mut uf, k);
    }
    let roots: HashMap<String, ()> = {
        let ks: Vec<String> = uf.keys().cloned().collect();
        ks.iter().map(|k| (find(&mut uf, k), ())).collect()
    };
    let mut pairs: Vec<(String, usize)> = pair_counts.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    GraphConnectivity {
        edges_total,
        same_organ_edges: same_organ,
        cross_organ_edges: cross_organ,
        unclassifiable_edges: unclassifiable,
        organ_labels_raw: labels_raw.len(),
        organ_keys_canonical: keys.len(),
        organ_components: roots.len(),
        top_cross_organ_pairs: pairs.into_iter().take(8).map(|(p, _)| p).collect(),
        note: "cross-organ edges are the federation signal; same-organ edges are lane threads. \
               organ_labels_raw > organ_keys_canonical measures routed_organ spelling \
               fragmentation itself (read-side canonicalisation, stored values untouched)"
            .into(),
    }
}

/// FQ_G (RG-9, 2026-09-13): institutional metabolism rate — measured LAST.
/// Counts the primitives the Reality Graph now records (beliefs born/died,
/// policies compressed from scars, reality invoices) and the latencies
/// between them. v1 reports DISTRIBUTIONS ONLY — thresholds are not invented
/// (n is small; the FQ≈1 equilibrium doctrine does not transfer to metabolism
/// rates without data). The ruler reads; it does not yet judge.
pub fn fq_graph(ledger: &LoadedLedger) -> FqGraphReport {
    let mut beliefs_born = 0usize;
    let mut supersession_events = 0usize;
    let mut governance_events = 0usize;
    let mut policies = 0usize;
    let mut invoices = 0usize;
    let mut belief_lifetime_ms = Vec::new();
    let mut scar_to_policy_ms = Vec::new();
    let mut policy_to_invoice_ms = Vec::new();
    let by_id: HashMap<String, &FlowReceipt> = ledger
        .receipts
        .iter()
        .map(|r| (r.receipt_id.to_string(), r))
        .collect();

    for r in &ledger.receipts {
        beliefs_born += 1;
        let pl = r.payload.as_ref().and_then(|p| p.as_object());
        if let Some(pl) = pl {
            if pl.contains_key("governance_event") {
                governance_events += 1;
            }
            if pl.contains_key("scar_binding") {
                policies += 1;
                // scar → policy latency via causal parents
                for pid in r.parent_receipt_ids.iter() {
                    if let Some(ms) = by_id.get(pid.as_str()).and_then(|p| {
                        age_ms(&p.created_at.to_rfc3339(), &r.created_at.to_rfc3339())
                    }) {
                        scar_to_policy_ms.push(ms);
                    }
                }
            }
            if pl.contains_key("consequence") {
                invoices += 1;
                for pid in r.parent_receipt_ids.iter() {
                    if let Some(ms) = by_id.get(pid.as_str()).and_then(|p| {
                        age_ms(&p.created_at.to_rfc3339(), &r.created_at.to_rfc3339())
                    }) {
                        policy_to_invoice_ms.push(ms);
                    }
                }
            }
        }
        // supersessions emitted by this receipt (belief deaths it caused)
        if !r.supersedes_receipt_ids.is_empty() {
            for tid in &r.supersedes_receipt_ids {
                supersession_events += 1;
                if let Some(ms) = by_id
                    .get(tid.as_str())
                    .and_then(|t| age_ms(&t.created_at.to_rfc3339(), &r.created_at.to_rfc3339()))
                {
                    belief_lifetime_ms.push(ms);
                }
            }
        }
    }
    let beliefs_superseded = ledger
        .receipts
        .iter()
        .filter(|r| {
            ledger.receipts.iter().any(|k| {
                k.supersedes_receipt_ids
                    .iter()
                    .any(|x| *x == r.receipt_id.to_string())
            })
        })
        .count();
    FqGraphReport {
        schema: "arifflow.fq-g/v1".into(),
        receipts_scanned: ledger.len(),
        beliefs_born,
        beliefs_superseded,
        supersession_events,
        governance_events,
        policies_emitted: policies,
        invoices_received: invoices,
        revision_rate: if beliefs_born > 0 {
            supersession_events as f64 / beliefs_born as f64
        } else {
            0.0
        },
        invoice_yield: if policies > 0 {
            invoices as f64 / policies as f64
        } else {
            0.0
        },
        belief_lifetime_ms,
        scar_to_policy_ms,
        policy_to_invoice_ms,
        connectivity: measure_connectivity(ledger),
        note: "v1 distributions only — thresholds deliberately not invented (small n; measure-first doctrine)".into(),
    }
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

    // ── connectivity census (2026-10-01) ────────────────────────────────

    #[test]
    fn canonical_organ_key_collapses_spelling_variants() {
        let cases = [
            ("AAA", "aaa"),
            ("aaa", "aaa"),
            ("arifOS", "arifos"),
            ("arifos", "arifos"),
            ("A-FORGE", "aforge"),
            ("aforge", "aforge"),
            ("WELL", "well"),
            ("well", "well"),
            ("arifFlow", "arifflow"),
            ("TEST_ORGAN", "testorgan"),
        ];
        for (raw, want) in cases {
            assert_eq!(canonical_organ_key(raw), want, "raw={}", raw);
        }
    }

    fn organ_step(organ: &str, step: StepType, n: u64) -> FlowReceipt {
        stamp(
            FlowReceipt::new_first(
                "connectivity-test",
                "s",
                step,
                EpistemicLabel::Observation,
                n,
            )
            .with_routed_organ(organ),
        )
    }

    #[test]
    fn connectivity_reads_spelling_variants_as_one_organ_not_two() {
        let dir = tempfile::tempdir().unwrap();
        let parent = organ_step("AAA", StepType::Execute, 0);
        let (pid, ph) = (
            parent.receipt_id.to_string(),
            parent.jcs_body_hash.clone().unwrap(),
        );
        let child = stamp(
            FlowReceipt::new_first(
                "connectivity-test",
                "s",
                StepType::Verify,
                EpistemicLabel::Observation,
                1,
            )
            .with_routed_organ("aaa")
            .with_causal_parents(vec![pid], vec![ph]),
        );
        let c = &fq_graph(&write_ledger(dir.path(), &[parent, child])).connectivity;
        assert_eq!(c.edges_total, 1);
        assert_eq!(
            c.same_organ_edges, 1,
            "AAA→aaa is one organ, not a crossing"
        );
        assert_eq!(c.cross_organ_edges, 0);
        assert_eq!(c.organ_labels_raw, 2, "two spellings observed");
        assert_eq!(c.organ_keys_canonical, 1, "one organ underneath");
        assert_eq!(c.organ_components, 1);
    }

    #[test]
    fn connectivity_counts_cross_organ_edges_and_names_the_flow() {
        let dir = tempfile::tempdir().unwrap();
        let aaa = organ_step("AAA", StepType::Execute, 0);
        let (aid, ah) = (
            aaa.receipt_id.to_string(),
            aaa.jcs_body_hash.clone().unwrap(),
        );
        let flow = stamp(
            FlowReceipt::new_first(
                "connectivity-test",
                "s",
                StepType::Verify,
                EpistemicLabel::Observation,
                1,
            )
            .with_routed_organ("arifFlow")
            .with_causal_parents(vec![aid], vec![ah]),
        );
        let c = &fq_graph(&write_ledger(dir.path(), &[aaa, flow])).connectivity;
        assert_eq!(c.cross_organ_edges, 1);
        assert_eq!(c.same_organ_edges, 0);
        assert_eq!(c.top_cross_organ_pairs, vec!["aaa → arifflow".to_string()]);
        assert_eq!(c.organ_components, 1, "the edge joins the two organs");
    }

    #[test]
    fn connectivity_refuses_to_classify_untagged_or_dangling_edges() {
        let dir = tempfile::tempdir().unwrap();
        let tagged = organ_step("WELL", StepType::Execute, 0);
        // child carries no routed_organ → the edge cannot be organ-classified
        let mut untagged_child = FlowReceipt::new_first(
            "connectivity-test",
            "s",
            StepType::Verify,
            EpistemicLabel::Observation,
            1,
        );
        untagged_child.parent_receipt_ids = vec![tagged.receipt_id.to_string()];
        // parent id is not in the ledger window → likewise unclassifiable
        let mut dangling = FlowReceipt::new_first(
            "connectivity-test",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            2,
        );
        dangling.routed_organ = Some("CHRON".into());
        dangling.parent_receipt_ids = vec!["00000000-0000-0000-0000-000000000000".into()];
        let ledger = write_ledger(
            dir.path(),
            &[tagged, stamp(untagged_child), stamp(dangling)],
        );
        let c = &fq_graph(&ledger).connectivity;
        assert_eq!(c.edges_total, 2);
        assert_eq!(c.unclassifiable_edges, 2);
        assert_eq!(c.cross_organ_edges, 0);
        assert_eq!(c.organ_keys_canonical, 2);
        assert_eq!(
            c.organ_components, 2,
            "WELL and CHRON remain separate islands — an isolated organ must still be counted"
        );
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
    fn fq_g_counts_and_latencies() {
        let dir = tempfile::tempdir().unwrap();
        let a = stamp(FlowReceipt::new_first(
            "a",
            "s",
            StepType::Execute,
            EpistemicLabel::Observation,
            0,
        ));
        let aid = a.receipt_id.to_string();
        let ah = a.jcs_body_hash.clone().unwrap();
        let mut pol = FlowReceipt::new_first(
            "a",
            "s",
            StepType::Execute,
            EpistemicLabel::Specification,
            0,
        )
        .with_causal_parents(vec![aid], vec![ah]);
        pol.payload = Some(serde_json::json!({"scar_binding": {
            "policy_slug": "p", "scar_id": "s", "enforcement_surface": "t",
            "enforcement_ref": "r", "policy_text": "x"}}));
        pol.created_at = a.created_at + chrono::Duration::milliseconds(3_600_000);
        let pol = stamp(pol);
        let pid = pol.receipt_id.to_string();
        let ph = pol.jcs_body_hash.clone().unwrap();
        let mut inv = FlowReceipt::new_first(
            "a",
            "s",
            StepType::Verify,
            EpistemicLabel::Interpretation,
            0,
        )
        .with_causal_parents(vec![pid], vec![ph]);
        inv.created_at = pol.created_at + chrono::Duration::milliseconds(7_200_000);
        let mut inv = stamp(inv);
        inv.payload = Some(serde_json::json!({"consequence": {
            "claim_slug": "x", "observed_outcome": "y", "evidence": "z",
            "outcome_class": "recovery"}}));
        let inv = stamp(inv);
        let ledger = write_ledger(dir.path(), &[a, pol, inv]);
        let rep = fq_graph(&ledger);
        assert_eq!(rep.policies_emitted, 1);
        assert_eq!(rep.invoices_received, 1);
        assert_eq!(rep.invoice_yield, 1.0);
        assert_eq!(rep.scar_to_policy_ms, vec![3_600_000]);
        assert_eq!(rep.policy_to_invoice_ms, vec![7_200_000]);
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
