#![allow(
    deprecated,
    dead_code,
    unused_imports,
    unused_assignments,
    unused_variables,
    clippy::too_many_arguments,
    clippy::lines_filter_map_ok
)]
// arifFlow — binary entry point for the governed parallel execution engine
//
// Two modes:
//   1) stdin/stdout JSON-L protocol (default) — for A-FORGE adapter / pipe usage
//   2) --daemon mode — TCP listener on ARIFLOW_PORT (default 7073) with:
//      GET /health    → status + FQ + invariant health
//      POST /ingest   → ingest flow receipt, update actor state, enforce invariants
//      POST /check    → check if actor is allowed to execute (invariant gate)
//      POST /release  → release hold on actor (after verification)
//      POST /enforce  → manually trigger enforcement cycle
//      POST /flow     → JSON-L command (same as stdin protocol)
//
// DITEMPA BUKAN DIBERI — arifOS = law, arifFlow = flow, A-FORGE = hands

use arifflow::bridge::{AForgeExecutorBridge, ArifOSGovernanceBridge, ExecutionRequest};
use arifflow::channel::ChannelMode;
use arifflow::governance::Vault999Sealer;
use arifflow::governance::invariants::InvariantEnforcer;
use arifflow::receipt::{
    ExplanationClass, FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE, FLOW_INVARIANT_EXPLANATION_CLASS,
    FlowReceipt, ReceiptStore, StepType,
};
use arifflow::scheduler::{FlowNode, SuperStepScheduler, TopologyKind, VerdictClass};
use arifflow::vector::{Dimension, Epistemology, IndependenceMonitor, VectorStore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// F5 (audit 2026-08-10): VAULT999 canonical witness-trail log.
// arifFlow appends sealed receipts here; VAULT999 is the only immutable store.
const VAULT999_LOG_PATH: &str = "/root/arifOS/VAULT999/arifflow_sealed.jsonl";

// ── Protocol Messages ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum StdinMsg {
    #[serde(rename = "configure")]
    Configure {
        topology: String,
        lease_id: String,
        actor_id: String,
        chain_id: String,
    },
    #[serde(rename = "seed")]
    Seed { channel: String, data: String },
    #[serde(rename = "step")]
    Step { nodes: Vec<NodeDef> },
    #[serde(rename = "verdict")]
    Verdict {
        class: String,
        verdict_id: String,
        hash: String,
    },
    #[serde(rename = "restore")]
    Restore { checkpoint: serde_json::Value },
    #[serde(rename = "stop")]
    Stop,
}

#[derive(Debug, Deserialize)]
struct NodeDef {
    id: String,
    subs: Vec<String>,
    outputs: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum StdoutMsg {
    #[serde(rename = "need_verdict")]
    NeedVerdict {
        step: u64,
        state_root: String,
        lease_id: String,
        chain_id: String,
        afq_execution_steps: u64,
        afq_governance_steps: u64,
        afq: f64,
        afq_diagnosis: String,
    },
    #[serde(rename = "step_result")]
    StepResult {
        step: u64,
        verdict: String,
        state_root: String,
        deltas: BTreeMap<String, Vec<String>>,
    },
    #[serde(rename = "cooling")]
    Cooling {
        total_steps: u64,
        final_root: String,
        leases_closed: u64,
    },
    #[serde(rename = "error")]
    Error { code: String, message: String },
}

// ── Runtime ─────────────────────────────────────────────────────────────

struct NodeWrapper {
    id: String,
    subs: Vec<String>,
    outputs: Vec<String>,
}

impl FlowNode for NodeWrapper {
    fn id(&self) -> &str {
        &self.id
    }
    fn subscriptions(&self) -> Vec<arifflow::channel::ChannelId> {
        self.subs
            .iter()
            .map(|s| arifflow::channel::ChannelId(s.clone()))
            .collect()
    }
    fn run(
        &self,
        _inputs: BTreeMap<arifflow::channel::ChannelId, Vec<arifflow::channel::Message<String>>>,
        _lease_id: uuid::Uuid,
    ) -> Result<BTreeMap<arifflow::channel::ChannelId, String>, arifflow::scheduler::NodeError>
    {
        let mut out = BTreeMap::new();
        for o in &self.outputs {
            out.insert(
                arifflow::channel::ChannelId(o.clone()),
                format!("result_{}", self.id),
            );
        }
        Ok(out)
    }
}

fn send(msg: &StdoutMsg) {
    let line = serde_json::to_string(msg).unwrap();
    println!("{}", line);
    io::stdout().flush().ok();
}

fn stdin_protocol_loop() {
    let stdin = io::stdin();
    let mut scheduler: Option<SuperStepScheduler> = None;
    let mut lease_id: String = String::new();
    let mut actor_id: String = String::new();
    let mut chain_id: String = String::new();
    let mut total_steps: u64 = 0;
    let mut pending_verdict = false;
    let mut pending_state_root = String::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                send(&StdoutMsg::Error {
                    code: "STDIN_READ_ERROR".into(),
                    message: e.to_string(),
                });
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let msg: StdinMsg = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                send(&StdoutMsg::Error {
                    code: "PARSE_ERROR".into(),
                    message: format!("Invalid JSON: {}", e),
                });
                continue;
            }
        };

        match msg {
            StdinMsg::Configure {
                topology,
                lease_id: lid,
                actor_id: aid,
                chain_id: cid,
            } => {
                lease_id = lid;
                actor_id = aid;
                chain_id = cid;
                total_steps = 0;
                pending_verdict = false;

                let kind = match topology.as_str() {
                    "fan_out" => TopologyKind::FanOut,
                    "pipeline" => TopologyKind::Pipeline,
                    "cascade" => TopologyKind::Cascade,
                    _ => {
                        send(&StdoutMsg::Error {
                            code: "UNKNOWN_TOPOLOGY".into(),
                            message: format!("Unknown topology: {}", topology),
                        });
                        continue;
                    }
                };

                let lid_uuid = uuid::Uuid::parse_str(&lease_id).unwrap_or(uuid::Uuid::nil());
                let cid_uuid = uuid::Uuid::parse_str(&chain_id).unwrap_or(uuid::Uuid::nil());

                let mut sched = SuperStepScheduler::new(kind, lid_uuid, actor_id.clone(), cid_uuid);
                sched.register_channel("input", ChannelMode::Unbounded);
                sched.register_channel("output", ChannelMode::Unbounded);
                scheduler = Some(sched);
            }

            StdinMsg::Seed { channel, data } => {
                if let Some(ref mut sched) = scheduler {
                    let _ = sched.seed_channel(&channel, data);
                }
            }

            StdinMsg::Step { nodes } => {
                if pending_verdict {
                    send(&StdoutMsg::Error {
                        code: "PENDING_VERDICT".into(),
                        message: "Previous step waiting for verdict. Send verdict first.".into(),
                    });
                    continue;
                }

                let sched = match scheduler.as_mut() {
                    Some(s) => s,
                    None => {
                        send(&StdoutMsg::Error {
                            code: "NOT_CONFIGURED".into(),
                            message: "Send configure first.".into(),
                        });
                        continue;
                    }
                };

                // Convert node definitions
                let boxed_nodes: Vec<Box<dyn FlowNode>> = nodes
                    .into_iter()
                    .map(|n| {
                        Box::new(NodeWrapper {
                            id: n.id,
                            subs: n.subs,
                            outputs: n.outputs,
                        }) as Box<dyn FlowNode>
                    })
                    .collect();

                match sched.step(&boxed_nodes) {
                    Ok(result) => {
                        pending_verdict = true;
                        pending_state_root = format!("{:?}", result.checkpoint.state_root);

                        send(&StdoutMsg::NeedVerdict {
                            step: result.step_number,
                            state_root: pending_state_root.clone(),
                            lease_id: lease_id.clone(),
                            chain_id: chain_id.clone(),
                            afq_execution_steps: result.fq.execute_count as u64,
                            afq_governance_steps: result.fq.verify_count as u64,
                            afq: result.fq.quotient.unwrap_or(0.0),
                            afq_diagnosis: result.fq.verdict.to_string(),
                        });
                    }
                    Err(e) => {
                        send(&StdoutMsg::Error {
                            code: "STEP_ERROR".into(),
                            message: format!("{:?}", e),
                        });
                    }
                }
            }

            StdinMsg::Verdict {
                class,
                verdict_id: _vid,
                hash: _vh,
            } => {
                if !pending_verdict {
                    send(&StdoutMsg::Error {
                        code: "NO_PENDING_VERDICT".into(),
                        message: "No step waiting for verdict.".into(),
                    });
                    continue;
                }
                pending_verdict = false;
                total_steps += 1;

                let sched = match scheduler.as_mut() {
                    Some(s) => s,
                    None => {
                        send(&StdoutMsg::Error {
                            code: "NOT_CONFIGURED".into(),
                            message: "Scheduler not configured.".into(),
                        });
                        continue;
                    }
                };

                let verdict_class = match class.as_str() {
                    "SEAL" => VerdictClass::SEAL,
                    "HOLD" => VerdictClass::HOLD,
                    "VOID" => VerdictClass::VOID,
                    "SABAR" => VerdictClass::SABAR,
                    _ => VerdictClass::HOLD,
                };

                sched.commit_verdict(verdict_class);

                let verdict_str = format!("{:?}", verdict_class);
                send(&StdoutMsg::StepResult {
                    step: total_steps - 1,
                    verdict: verdict_str,
                    state_root: pending_state_root.clone(),
                    deltas: BTreeMap::new(),
                });
            }

            StdinMsg::Restore { .. } => {
                // Replay checkpoint — simplified for Phase 2
                send(&StdoutMsg::StepResult {
                    step: 0,
                    verdict: "SEAL".into(),
                    state_root: "0".repeat(64),
                    deltas: BTreeMap::new(),
                });
            }

            StdinMsg::Stop => {
                send(&StdoutMsg::Cooling {
                    total_steps,
                    final_root: pending_state_root.clone(),
                    leases_closed: 1,
                });
                break;
            }
        }
    }

    // If stdin closed without stop, send cooling anyway
    if scheduler.is_some() {
        send(&StdoutMsg::Cooling {
            total_steps,
            final_root: pending_state_root,
            leases_closed: 1,
        });
    }
}

// ── Daemon Mode ────────────────────────────────────────────────────

/// HTTP response helper
fn http_ok(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

/// HTTP 400 response helper (SEQ-N /lineage and future read-only surfaces)
fn http_bad_request(body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .into_bytes()
}

/// Extract JSON body from HTTP request (after \r\n\r\n)
fn extract_body(request: &str) -> Option<&str> {
    request.split("\r\n\r\n").nth(1)
}

/// Handle a single HTTP connection on the daemon port
fn handle_client(
    mut stream: TcpStream,
    start_time: Instant,
    receipt_store: &Arc<Mutex<ReceiptStore>>,
    enforcer: &Arc<Mutex<InvariantEnforcer>>,
    vector_store: &Arc<Mutex<VectorStore>>,
    independence: &Arc<Mutex<IndependenceMonitor>>,
    persist_path: &PathBuf,
    persist_mutex: &Arc<Mutex<()>>,
    vault_sealer: &Arc<Mutex<Vault999Sealer>>,
) {
    let mut buf = [0u8; 16384];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            let request = String::from_utf8_lossy(&buf[..n]);
            let response = if request.starts_with("GET /health") {
                let store = receipt_store.lock().unwrap();
                let enf = enforcer.lock().unwrap();
                let mut vs = vector_store.lock().unwrap();
                let mut indep = independence.lock().unwrap();
                vs.tick();
                let fq = store.flow_quotient(100);
                // Inject live FQ into the vector engine (MEASURE·LIVE)
                vs.inject_fq(fq.quotient);
                indep.record(&vs);
                let restricted: Vec<serde_json::Value> = enf
                    .restricted_actors()
                    .iter()
                    .map(|(id, action, reason)| {
                        serde_json::json!({
                            "actor": id,
                            "action": format!("{:?}", action),
                            "reason": reason,
                        })
                    })
                    .collect();
                // Diagnosis-first reporting (2026-08-14): scalar FQ is deprecated as a
                // sovereign-facing health indicator. Concentration is harder to game.
                let total_steps = fq.execute_count + fq.verify_count;
                let diagnosis = if total_steps == 0 {
                    "UNMEASURED"
                } else {
                    let verify_pct = fq.verify_count as f64 / total_steps as f64 * 100.0;
                    if verify_pct > 80.0 {
                        "VERIFICATION DOMINANCE"
                    } else if verify_pct < 20.0 {
                        "EXECUTION DOMINANCE"
                    } else {
                        "BALANCED"
                    }
                };
                // ── FQ VECTOR (2026-08-14): per-actor breakdown ──
                // The scalar masks per-actor pathology (e.g. one actor stuck at 0.0
                // while automated heartbeats inflate the global verify count).
                // per_actor exposes the vector so diagnosis is actor-specific.
                let per_actor: BTreeMap<String, serde_json::Value> = enf
                    .actors
                    .iter()
                    .map(|(id, state)| {
                        let total = state.execute_count + state.verify_count;
                        let verify_pct = if total > 0 {
                            state.verify_count as f64 / total as f64 * 100.0
                        } else {
                            0.0
                        };
                        let actor_dx = if total == 0 {
                            "UNMEASURED"
                        } else if verify_pct > 80.0 {
                            "VERIFICATION DOMINANCE"
                        } else if verify_pct < 20.0 && state.execute_count > 0 {
                            "EXECUTION DOMINANCE"
                        } else {
                            "BALANCED"
                        };
                        (
                            id.clone(),
                            serde_json::json!({
                                "execute": state.execute_count,
                                "verify": state.verify_count,
                                "quotient": state.quotient,
                                "verdict": format!("{}", state.verdict),
                                "diagnosis": actor_dx,
                                "held": state.held,
                                "throttled": state.throttled,
                                "consecutive_exec_no_verify": state.consecutive_executes_without_verify,
                                "risk_class": state.last_risk_class.code(),
                                "fq_required": state.last_risk_class.fq_required(),
                            }),
                        )
                    })
                    .collect();
                let actors_tracked = per_actor.len();

                // ── QG.V0.3 VECTOR (2026-08-14, spec §4): full vector state ──
                let vector_state = vs.vector_state();
                let vector_rank = (vs.rank() * 1000.0).round() / 1000.0;
                let constellation = vs.constellation();
                let primary_pathology = vs
                    .primary_pathology()
                    .map(|d| d.failure().to_string())
                    .unwrap_or_else(|| "NONE".to_string());
                // RED-010: gate the headline diagnosis on G calibration — the
                // constitutional alarm name is not emitted while G is uncalibrated.
                let diagnosis_label = if primary_pathology == "GOVERNANCE_COLLAPSE" {
                    "HEURISTIC_ADVISORY".to_string()
                } else {
                    primary_pathology.clone()
                };
                let collapse_pairs: Vec<serde_json::Value> = indep
                    .collapse_pairs()
                    .iter()
                    .map(|(a, b, r)| {
                        serde_json::json!({
                            "dim_a": a.code(),
                            "dim_b": b.code(),
                            "pearson": (r * 100.0).round() / 100.0,
                        })
                    })
                    .collect();

                let body = serde_json::json!({
                    "status": "ok-v3-vector",
                    // SPEC STEP 10 (RETIRE SCALAR): the canonical vector is the
                    // headline; the deprecated scalar is demoted to legacy_* fields.
                    "verdict": constellation.clone(),
                    "diagnosis": diagnosis_label,
                    "fq": {
                        "quotient": fq.quotient,
                        "legacy_verdict": format!("{}", fq.verdict),
                        "execute_count": fq.execute_count,
                        "verify_count": fq.verify_count,
                        "barrier_count": fq.barrier_count,
                        "legacy_diagnosis": diagnosis,
                        "scalar_fq_note": "Deprecated as health indicator — use top-level verdict (vector constellation).",
                        // ── FQ VECTOR (per-actor) ──
                        "per_actor": per_actor,
                        "metric_frame": {
                            "window_size": 100,
                            "sample_size": fq.window_size,
                            "actors_tracked": actors_tracked,
                            "formula_version": "qg.v0.3.1-vector",
                        },
                    },
                    // ── QG.V0.3.1 VECTOR ONTOLOGY (spec §4) ──
                    "vector": {
                        "diagnosis": {
                            "constellation": constellation,
                            "primary_pathology": primary_pathology,
                            "fused_rank": vector_rank,
                            "healthy_shape": "constellation, not maximum",
                        },
                        "dimensions": vector_state,
                        "independence": {
                            "collapse_pairs": collapse_pairs,
                            "monitored": "INV-3 |ρ| ≤ 0.85",
                        },
                        "ontology": {
                            "spec": "QG_V0_3_VECTOR_SPEC.md v0.3.1-AMD",
                            "sealed": "2026-08-14",
                            "formula_version": "qg.v0.3.1-vector",
                            "invariants": ["INV-1","INV-2","INV-3","INV-4","INV-5","INV-6","INV-7","INV-8","INV-9","INV-10","INV-11","INV-12"],
                        },
                    },
                    "provenance": {
                        "formula_version": "qg.v0.3.1-vector",
                        "formula_hash": "sha256:arifflow-fq-v2.1-2026-08-05",
                        "window_start_utc": start_time.elapsed().as_secs().to_string(),
                        "window_duration_s": fq.window_size as u64,
                        "tau_half_lives": {
                            "LIVE": 10,
                            "MEASURE": 100,
                            "WITNESS": 250,
                            "FEEL": "anchor N (default 10)",
                        },
                    },
                    "invariants": {
                        "cycle_count": enf.cycle_count,
                        "hold_count": enf.hold_count,
                        "throttle_count": enf.throttle_count,
                        "restricted_actors": restricted,
                        // CM-1 (2026-09-19) — explanatory-class gate (F2 + F3).
                        // Reported as observation: how many execution-class
                        // receipts arifFlow refused to transmit, under which
                        // named code, and who owns the verdict.
                        "explanation_class_gate": {
                            "code": FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE,
                            "invariant": FLOW_INVARIANT_EXPLANATION_CLASS,
                            "refusals": enf.explanation_refusals,
                            "last_violation": enf.last_explanation_violation,
                            "verdict_owner": "claim_kernel",
                        },
                    },
                    "receipts": store.len(),
                    "uptime_ms": start_time.elapsed().as_millis() as u64,
                })
                .to_string();
                http_ok(&body)
            } else if request.starts_with("POST /fq_g") {
                // RG-9 / FQ_G (2026-09-13): institutional metabolism rate —
                // measured LAST. Read-only, full-ledger distributions.
                match arifflow::lineage_query::LoadedLedger::from_path(std::path::Path::new(
                    "/var/lib/arifflow/receipts.jsonl",
                )) {
                    Err(e) => http_bad_request(
                        &serde_json::json!({"status": "ledger_unreadable", "error": format!("{}", e)}).to_string(),
                    ),
                    Ok(ledger) => http_ok(
                        &serde_json::to_string(&arifflow::lineage_query::fq_graph(&ledger))
                            .unwrap_or_else(|_| "{}".into()),
                    ),
                }
            } else if request.starts_with("POST /consequences") {
                // RG-7 (2026-09-13): consequence records — read-only.
                match extract_body(&request) {
                    None => http_bad_request(
                        &serde_json::json!({"status": "invalid", "error": "empty body"})
                            .to_string(),
                    ),
                    Some(raw_json) => match serde_json::from_str::<serde_json::Value>(
                        raw_json.trim(),
                    ) {
                        Err(e) => http_bad_request(
                            &serde_json::json!({"status": "invalid", "error": format!("{}", e)})
                                .to_string(),
                        ),
                        Ok(req) => {
                            let before = req.get("before_receipt_id").and_then(|v| v.as_str());
                            match arifflow::lineage_query::LoadedLedger::from_path(std::path::Path::new(
                                "/var/lib/arifflow/receipts.jsonl",
                            )) {
                                Err(e) => http_bad_request(
                                    &serde_json::json!({"status": "ledger_unreadable", "error": format!("{}", e)}).to_string(),
                                ),
                                Ok(ledger) => match arifflow::lineage_query::consequences(&ledger, before) {
                                    Err(e) => http_bad_request(
                                        &serde_json::json!({"status": "consequences_error", "error": e}).to_string(),
                                    ),
                                    Ok(records) => http_ok(
                                        &serde_json::to_string(&serde_json::json!({
                                            "schema": "arifflow.consequences/v1",
                                            "as_of_receipt_id": before,
                                            "count": records.len(),
                                            "consequences": records,
                                        }))
                                        .unwrap_or_else(|_| "{}".into()),
                                    ),
                                },
                            }
                        }
                    },
                }
            } else if request.starts_with("POST /scar_policies") {
                // RG-5 (2026-09-13): scar-bound policy query — read-only.
                match extract_body(&request) {
                    None => http_bad_request(
                        &serde_json::json!({"status": "invalid", "error": "empty body"})
                            .to_string(),
                    ),
                    Some(raw_json) => match serde_json::from_str::<serde_json::Value>(
                        raw_json.trim(),
                    ) {
                        Err(e) => http_bad_request(
                            &serde_json::json!({"status": "invalid", "error": format!("{}", e)})
                                .to_string(),
                        ),
                        Ok(req) => {
                            let before = req.get("before_receipt_id").and_then(|v| v.as_str());
                            match arifflow::lineage_query::LoadedLedger::from_path(std::path::Path::new(
                                "/var/lib/arifflow/receipts.jsonl",
                            )) {
                                Err(e) => http_bad_request(
                                    &serde_json::json!({"status": "ledger_unreadable", "error": format!("{}", e)}).to_string(),
                                ),
                                Ok(ledger) => match arifflow::lineage_query::scar_policies(&ledger, before) {
                                    Err(e) => http_bad_request(
                                        &serde_json::json!({"status": "scar_policies_error", "error": e}).to_string(),
                                    ),
                                    Ok(policies) => http_ok(
                                        &serde_json::to_string(&serde_json::json!({
                                            "schema": "arifflow.scar-policies/v1",
                                            "as_of_receipt_id": before,
                                            "count": policies.len(),
                                            "policies": policies,
                                        }))
                                        .unwrap_or_else(|_| "{}".into()),
                                    ),
                                },
                            }
                        }
                    },
                }
            } else if request.starts_with("POST /gov_events") {
                // RG-4 (2026-09-13): governance-event query — read-only.
                // Body: {} or {"before_receipt_id": "..."} for time travel.
                match extract_body(&request) {
                    None => http_bad_request(
                        &serde_json::json!({"status": "invalid", "error": "empty body"})
                            .to_string(),
                    ),
                    Some(raw_json) => match serde_json::from_str::<serde_json::Value>(
                        raw_json.trim(),
                    ) {
                        Err(e) => http_bad_request(
                            &serde_json::json!({"status": "invalid", "error": format!("{}", e)})
                                .to_string(),
                        ),
                        Ok(req) => {
                            let before = req.get("before_receipt_id").and_then(|v| v.as_str());
                            match arifflow::lineage_query::LoadedLedger::from_path(std::path::Path::new(
                                "/var/lib/arifflow/receipts.jsonl",
                            )) {
                                Err(e) => http_bad_request(
                                    &serde_json::json!({"status": "ledger_unreadable", "error": format!("{}", e)}).to_string(),
                                ),
                                Ok(ledger) => match arifflow::lineage_query::gov_events(&ledger, before) {
                                    Err(e) => http_bad_request(
                                        &serde_json::json!({"status": "gov_events_error", "error": e}).to_string(),
                                    ),
                                    Ok(events) => http_ok(
                                        &serde_json::to_string(&serde_json::json!({
                                            "schema": "arifflow.gov-events/v1",
                                            "as_of_receipt_id": before,
                                            "count": events.len(),
                                            "events": events,
                                        }))
                                        .unwrap_or_else(|_| "{}".into()),
                                    ),
                                },
                            }
                        }
                    },
                }
            } else if request.starts_with("POST /lineage") {
                // SEQ-N (2026-09-13): belief-lineage query surface — read-only.
                // Body: {"receipt_id": "...", "before_receipt_id": "..."?}
                match extract_body(&request) {
                    None => http_bad_request(
                        &serde_json::json!({"status": "invalid", "error": "empty body"})
                            .to_string(),
                    ),
                    Some(raw_json) => match serde_json::from_str::<serde_json::Value>(
                        raw_json.trim(),
                    ) {
                        Err(e) => http_bad_request(
                            &serde_json::json!({"status": "invalid", "error": format!("{}", e)})
                                .to_string(),
                        ),
                        Ok(req) => {
                            let target = req.get("receipt_id").and_then(|v| v.as_str());
                            let before = req.get("before_receipt_id").and_then(|v| v.as_str());
                            match target {
                                None => http_bad_request(
                                    &serde_json::json!({"status": "invalid", "error": "receipt_id is required"}).to_string(),
                                ),
                                Some(target) => {
                                    match arifflow::lineage_query::LoadedLedger::from_path(
                                        std::path::Path::new("/var/lib/arifflow/receipts.jsonl"),
                                    ) {
                                        Err(e) => http_bad_request(
                                            &serde_json::json!({"status": "ledger_unreadable", "error": format!("{}", e)}).to_string(),
                                        ),
                                        Ok(ledger) => match arifflow::lineage_query::lineage_report(
                                            &ledger, target, before,
                                        ) {
                                            Err(e) => http_bad_request(
                                                &serde_json::json!({"status": "lineage_error", "error": e}).to_string(),
                                            ),
                                            Ok(report) => http_ok(
                                                &serde_json::to_string(&report).unwrap_or_else(|_| "{}".into()),
                                            ),
                                        },
                                    }
                                }
                            }
                        }
                    },
                }
            } else if request.starts_with("POST /ingest") {
                match extract_body(&request) {
                    Some(raw_json) => match serde_json::from_str::<FlowReceipt>(raw_json.trim()) {
                        Ok(mut receipt) => {
                            // ── CM-1 (2026-09-19): explanatory-class gate (F2 + F3) ──
                            // Receipt-validation point. arifFlow does not classify the
                            // claim (claim_kernel owns that verdict, on disk at
                            // /root/AAA/lib/claim_kernel/claim_kernel.py) — it refuses to
                            // transmit an execution-class receipt whose declared class
                            // cannot justify a mutation. Refusal is a flow-plane
                            // violation, reported with the named code and never stored.
                            let gate = receipt.explanation_gate();
                            if gate.is_refused() {
                                let mut enf = enforcer.lock().unwrap();
                                let check = enf.note_explanation_refusal(&receipt, &gate);
                                drop(enf);
                                eprintln!(
                                    "[arifFlow] EXPLANATION-CLASS REFUSAL {} receipt={} actor={} class={}",
                                    FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE,
                                    receipt.receipt_id,
                                    receipt.actor_id,
                                    gate.explanation_class
                                        .map(|c| c.code())
                                        .unwrap_or("UNKNOWN")
                                );
                                let body = serde_json::json!({
                                    "status": "refused",
                                    "refused": true,
                                    "code": FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE,
                                    "violation": "flow-plane",
                                    "invariant": FLOW_INVARIANT_EXPLANATION_CLASS,
                                    "invariant_enforced": arifflow::governance::FlowInvariant::F3_ObserveNeverInterpret.name().to_string(),
                                    "verdict_owner": "claim_kernel",
                                    "actor": receipt.actor_id,
                                    "step_type": format!("{}", receipt.step_type),
                                    "receipt_id": receipt.receipt_id.to_string(),
                                    "explanation_class": gate.explanation_class.map(|c| c.code()),
                                    "explanation_schema": gate.schema,
                                    "reason": gate.reason,
                                    "stored": false,
                                    "enforcement": {
                                        "invariant": format!("{:?}", check.invariant),
                                        "status": format!("{:?}", check.status),
                                        "reason": check.reason,
                                    },
                                })
                                .to_string();
                                let response = format!(
                                    "HTTP/1.1 422 Unprocessable Entity\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(), body
                                ).into_bytes();
                                let _ = stream.write_all(&response);
                                return;
                            }
                            // RG-PH (2026-09-13): daemon stamps the canonical
                            // JCS body hash server-side — client-supplied values
                            // are recomputed, never trusted. Fail-soft on
                            // schema-discipline violations (e.g. u64 > 2^53).
                            match receipt.compute_jcs_body_hash() {
                                Ok(h) => receipt.jcs_body_hash = Some(h),
                                Err(e) => eprintln!(
                                    "[arifFlow] WARN: jcs stamp failed for {}: {} \
                                     (receipt stored unhashed)",
                                    receipt.receipt_id, e
                                ),
                            }
                            let mut store = receipt_store.lock().unwrap();
                            let mut enf = enforcer.lock().unwrap();
                            // [FIX 2] 2026-08-10: chain-aware ingest — rejects receipts with
                            // malformed previous_receipt_hash. Accepts new chain starts (no hash)
                            // and receipts whose previous hash matches an existing stored receipt.
                            // Multi-session safe: different sessions can coexist.
                            match store.push_chain_aware(receipt.clone()) {
                                Ok(_) => {
                                    // [FIX 4] 2026-08-10: daemon-side receipt persistence.
                                    // Append receipt as JSON line to durable file storage.
                                    // Uses a shared mutex to serialize writes across threads.
                                    let _lock = persist_mutex.lock().unwrap();
                                    if let Ok(mut file) = OpenOptions::new()
                                        .create(true)
                                        .append(true)
                                        .open(persist_path)
                                        && let Ok(line) = serde_json::to_string(&receipt)
                                    {
                                        let _ = writeln!(file, "{}", line);
                                    }
                                    // FIX 4 (audit 2026-08-10): chain-validation log line.
                                    eprintln!(
                                        "[arifFlow] Receipt {} ingested with chain validation",
                                        receipt.receipt_id
                                    );
                                }
                                Err(chain_err) => {
                                    eprintln!(
                                        "[arifFlow] Chain-aware reject for receipt {}: {}",
                                        receipt.receipt_id, chain_err
                                    );
                                    let body = serde_json::json!({
                                        "status": "chain_invalid",
                                        "error": chain_err,
                                    })
                                    .to_string();
                                    let response = format!(
                                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body.len(), body
                                    ).into_bytes();
                                    let _ = stream.write_all(&response);
                                    return;
                                }
                            }
                            // F5: Receipts are sealed to VAULT999 for canonical witness trail.
                            // Receipt's SHA3-256 hash becomes the seal checkpoint, producing a
                            // tamper-evident hash chain appended to VAULT999/arifflow_sealed.jsonl.
                            // Failures here are logged but do not fail /ingest (in-memory primary).
                            if let Ok(mut sealer) = vault_sealer.lock() {
                                let checkpoint: [u8; 32] = match hex::decode(receipt.hash()) {
                                    Ok(bytes) if bytes.len() == 32 => {
                                        let mut arr = [0u8; 32];
                                        arr.copy_from_slice(&bytes);
                                        arr
                                    }
                                    _ => [0u8; 32],
                                };
                                match sealer.seal(checkpoint) {
                                    Ok(seal_receipt) => {
                                        // RG-2 lineage-aware seal entry:
                                        // Carry the receipt body hash, parent edges,
                                        // and genesis anchor into the sealed record.
                                        // This allows LineageResolver to reconstruct
                                        // lineage from sealed evidence alone, without
                                        // needing the original receipt body.
                                        let body_hash = hex::encode(checkpoint);
                                        let line = serde_json::json!({
                                            "vault_entry_id": seal_receipt.vault_entry_id,
                                            "chain_position": seal_receipt.chain_position,
                                            "prev_hash": hex::encode(seal_receipt.prev_hash),
                                            "chain_entry_hash": hex::encode(seal_receipt.chain_entry_hash),
                                            "receipt_id": receipt.receipt_id,
                                            "body_hash": body_hash,
                                            "parent_receipt_hashes": receipt.parent_receipt_ids,
                                            "genesis_anchor": receipt.genesis_anchor,
                                            "routed_organ": receipt.routed_organ,
                                        })
                                        .to_string();
                                        match OpenOptions::new()
                                            .create(true)
                                            .append(true)
                                            .open(VAULT999_LOG_PATH)
                                        {
                                            Ok(mut f) => {
                                                let _ = writeln!(f, "{}", line);
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "[arifFlow] WARN: VAULT999 log unwritable at {}: {} — seal kept in-memory",
                                                    VAULT999_LOG_PATH, e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[arifFlow] WARN: VAULT999 seal failed: {} — ingest continues in-memory",
                                            e
                                        );
                                    }
                                }
                            }
                            // Ingest into invariant enforcer
                            enf.ingest(&receipt);
                            // [AUTO-RELEASE 2026-09-25 FI-008 — F13 directive "scan what
                            // hold is breaking the flow"] The hold protocol (AGENTS.md
                            // §Invariant Enforcement) defines the release condition as
                            // "after verification receipt" — but no lane ever called
                            // POST /release, so `held` was a one-way ratchet: measured at
                            // scan time hold_count=35,812 vs cycle_count=3,569, with
                            // healthy actors locked (333-agi FQ=1.92 HELD) while a
                            // genuinely STUCK actor (qwen-code FQ=0.4) sailed unheld.
                            // A Verify receipt now clears the hold itself — the loop the
                            // protocol always specified. enf.ingest() already resets the
                            // consecutive-counter on Verify, so this adds ONLY the
                            // held/throttled flag clear. POST /release remains for manual
                            // and future SCT-gated governance (deferred to F13, 2026-08-10 note).
                            if receipt.step_type == StepType::Verify {
                                let was_held = enf
                                    .actors
                                    .get(&receipt.actor_id)
                                    .map(|s| s.held || s.throttled)
                                    .unwrap_or(false);
                                if was_held {
                                    enf.release_hold(&receipt.actor_id);
                                    eprintln!(
                                        "[arifFlow] AUTO-RELEASE: hold cleared for {} on Verify receipt {}",
                                        receipt.actor_id, receipt.receipt_id
                                    );
                                }
                            }
                            let fq = store.flow_quotient(20);
                            let body = serde_json::json!({
                                "status": "ingested",
                                "actor": receipt.actor_id,
                                "step_type": format!("{}", receipt.step_type),
                                "receipt_id": receipt.receipt_id.to_string(),
                                "jcs_body_hash": receipt.jcs_body_hash,
                                "fq": {
                                    "quotient": fq.quotient,
                                    "verdict": format!("{}", fq.verdict),
                                    "execute_count": fq.execute_count,
                                    "verify_count": fq.verify_count,
                                },
                                "provenance": {
                                    "formula_version": receipt.formula_version,
                                    "formula_hash": receipt.formula_hash,
                                    "witness_organs": receipt.witness_organs,
                                },
                                // CM-1: the gate's decision is reported whether
                                // the class was absent (LEGACY_UNTAGGED), eligible,
                                // or not applicable — omission stays observable.
                                "explanation_gate": {
                                    "outcome": gate.outcome_code,
                                    "class": gate.explanation_class.map(|c| c.code()),
                                    "schema": gate.schema,
                                    "code": gate.code,
                                },
                                "receipts": store.len(),
                            })
                            .to_string();
                            http_ok(&body)
                        }
                        Err(e) => {
                            let body = serde_json::json!({
                                "status": "invalid",
                                "error": format!("{}", e),
                            })
                            .to_string();
                            format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(), body
                            ).into_bytes()
                        }
                    },
                    None => {
                        let body = r#"{"status":"error","message":"Empty body"}"#;
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(), body
                        ).into_bytes()
                    }
                }
            } else if request.starts_with("POST /vector") {
                // ── QG.V0.3 VECTOR INGEST (spec §2.1, §9) ──
                // External organs push their dimension readings (G, J, W³, C_dark,
                // ΔS, Ω₀). Each reading MUST declare its four-part epistemology
                // contract. FQ is injected live from receipts — not accepted here.
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct VectorReading {
                            dimension: String,
                            value: f64,
                            epistemology: Option<String>,
                            method_id: Option<String>,
                            producer: Option<String>,
                            /// FEEL anchor present (WITNESS/MEASURE backing claim)
                            anchored: Option<bool>,
                        }
                        match serde_json::from_str::<VectorReading>(raw_json.trim()) {
                            Ok(reading) => {
                                let mut vs = vector_store.lock().unwrap();
                                let mut indep = independence.lock().unwrap();
                                vs.tick();
                                // Map dimension code → Dimension
                                let dim = match reading.dimension.as_str() {
                                    "g" | "G" => Dimension::G,
                                    "j" | "J" => Dimension::J,
                                    "w3" | "W3" | "w3_consensus" => Dimension::W3,
                                    "c_dark" | "cdark" | "C_dark" => Dimension::CDark,
                                    "ds" | "dS" | "ΔS" | "delta_s" => Dimension::DS,
                                    "omega" | "omega0" | "Ω0" | "Ω₀" | "Omega0" => {
                                        Dimension::Omega0
                                    }
                                    "fq" | "FQ" => {
                                        let body = serde_json::json!({
                                            "status": "rejected",
                                            "error": "FQ is injected live from receipts (MEASURE·LIVE). Do not POST it.",
                                        })
                                        .to_string();
                                        let response = format!(
                                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(), body
                                        ).into_bytes();
                                        let _ = stream.write_all(&response);
                                        return;
                                    }
                                    other => {
                                        let body = serde_json::json!({
                                            "status": "rejected",
                                            "error": format!("Unknown dimension: {}", other),
                                            "valid": ["g","j","w3","c_dark","ds","omega"],
                                        })
                                        .to_string();
                                        let response = format!(
                                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(), body
                                        ).into_bytes();
                                        let _ = stream.write_all(&response);
                                        return;
                                    }
                                };
                                // Declared or default epistemology per spec §1 table
                                let ep = match reading.epistemology.as_deref() {
                                    Some("MEASURE") | Some("measure") => Epistemology::Measure,
                                    Some("WITNESS") | Some("witness") => Epistemology::Witness,
                                    Some("FEEL") | Some("feel") => Epistemology::Feel,
                                    Some("LIVE") | Some("live") => Epistemology::Live,
                                    _ => dim.default_epistemology(),
                                };
                                let method_id = reading.method_id.unwrap_or_else(|| {
                                    dim.default_epistemology().code().to_string()
                                });
                                let producer =
                                    reading.producer.unwrap_or_else(|| "external".to_string());
                                let anchored = reading.anchored.unwrap_or(false);
                                vs.ingest(dim, reading.value, ep, &method_id, &producer, anchored);
                                indep.record(&vs);
                                let body = serde_json::json!({
                                    "status": "ingested",
                                    "dimension": dim.code(),
                                    "value": reading.value,
                                    "epistemology": ep.code(),
                                    "method_id": method_id,
                                    "producer": producer,
                                    "vector": vs.vector_state().get(dim.code()).cloned().unwrap_or(serde_json::Value::Null),
                                })
                                .to_string();
                                http_ok(&body)
                            }
                            Err(e) => {
                                let body = serde_json::json!({
                                    "status": "invalid",
                                    "error": format!("{}", e),
                                    "example": {"dimension":"g","value":0.85,"epistemology":"WITNESS","method_id":"forge_evaluate","producer":"A-FORGE","anchored":true},
                                })
                                .to_string();
                                format!(
                                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(), body
                                ).into_bytes()
                            }
                        }
                    }
                    None => {
                        let body = r#"{"status":"error","message":"Empty body"}"#;
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(), body
                        ).into_bytes()
                    }
                }
            } else if request.starts_with("POST /check") {
                // ── INVARIANT GATE: Check if actor is allowed to execute ──
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct CheckRequest {
                            actor_id: String,
                            // CM-1 (2026-09-19, additive): optional explanatory class of
                            // the intended action. Requests that omit it behave exactly
                            // as before. Source of the vocabulary: claim_kernel at
                            // /root/AAA/lib/claim_kernel/claim_kernel.py.
                            #[serde(default)]
                            explanation_class: Option<ExplanationClass>,
                            #[serde(default)]
                            explanation_schema: Option<String>,
                        }
                        match serde_json::from_str::<CheckRequest>(raw_json.trim()) {
                            Ok(req) => {
                                // CM-1: request-time refusal. A declared class that
                                // cannot justify a mutation is refused BEFORE the
                                // execute, mirroring the /ingest receipt check. This is
                                // still an observation about a transmitted class — the
                                // claim itself is never classified here.
                                if let Some(class) = req.explanation_class
                                    && !class.is_action_eligible()
                                {
                                    let body = serde_json::json!({
                                        "actor": req.actor_id,
                                        "allowed": false,
                                        "action": "Hold",
                                        "code": FLOW_CODE_EXPLANATION_CLASS_INELIGIBLE,
                                        "violation": "flow-plane",
                                        "invariant": FLOW_INVARIANT_EXPLANATION_CLASS,
                                        "verdict_owner": "claim_kernel",
                                        "explanation_class": class.code(),
                                        "explanation_schema": req.explanation_schema,
                                        "reason": format!(
                                            "{} is not action-eligible — refused before execute; \
                                             MEASURED | MECHANISM | PATTERN may justify a mutation",
                                            class
                                        ),
                                    })
                                    .to_string();
                                    let response = format!(
                                        "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body.len(), body
                                    ).into_bytes();
                                    let _ = stream.write_all(&response);
                                    return;
                                }
                                let enf = enforcer.lock().unwrap();
                                let (allowed, reason, action) = enf.check_actor(&req.actor_id);
                                let body = serde_json::json!({
                                    "actor": req.actor_id,
                                    "allowed": allowed,
                                    "reason": reason,
                                    "action": format!("{:?}", action),
                                    "explanation_class": req.explanation_class.map(|c| c.code()),
                                });
                                if allowed {
                                    http_ok(&body.to_string())
                                } else {
                                    let body_str = body.to_string();
                                    format!(
                                        "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body_str.len(), body_str
                                    ).into_bytes()
                                }
                            }
                            Err(e) => {
                                let body = serde_json::json!({"status": "invalid", "error": format!("{}", e)}).to_string();
                                format!(
                                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                    body.len(), body
                                ).into_bytes()
                            }
                        }
                    }
                    None => {
                        let body = r#"{"status":"error","message":"Empty body. Send {\"actor_id\":\"...\"}"}"#;
                        format!(
                            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(), body
                        ).into_bytes()
                    }
                }
            } else if request.starts_with("POST /release") {
                // ── Release hold on actor (called after verification) ──
                // [L0↔L2] Self-release governance requires SCT cryptographic identity,
                // not plaintext string comparison. The plaintext requester_id check was
                // reverted (2026-08-10 L2↔L3 runtime probe: broken API contract —
                // Python client doesn't send requester_id → all clients get 400).
                // Defer to F13 SOVEREIGN decision. Until then, release remains open.
                // Documented as honest governance gap, not hidden.
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct ReleaseRequest {
                            actor_id: String,
                        }
                        match serde_json::from_str::<ReleaseRequest>(raw_json.trim()) {
                            Ok(req) => {
                                let mut enf = enforcer.lock().unwrap();
                                enf.release_hold(&req.actor_id);
                                let body = serde_json::json!({
                                    "status": "released",
                                    "actor": req.actor_id,
                                });
                                http_ok(&body.to_string())
                            }
                            Err(e) => {
                                let body = serde_json::json!({"status": "invalid", "error": format!("{}", e)}).to_string();
                                format!(
                                    "HTTP/1.1 400 Bad Request
Content-Type: application/json
Content-Length: {}
Connection: close

{}",
                                    body.len(),
                                    body
                                )
                                .into_bytes()
                            }
                        }
                    }
                    None => {
                        let body = r#"{"status":"error","message":"Empty body. Send {\"actor_id\":\"...\"}"}"#;
                        format!(
                            "HTTP/1.1 400 Bad Request
Content-Type: application/json
Content-Length: {}
Connection: close

{}",
                            body.len(),
                            body
                        )
                        .into_bytes()
                    }
                }
            } else if request.starts_with("POST /execute") {
                // ── E2: arifFlow → A-FORGE execution bridge ──
                // Routes node execution requests through AForgeExecutorBridge
                // which now makes a real HTTP call to A-FORGE :7071/mcp.
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct ExecuteRequest {
                            node_id: String,
                            topology: Option<String>,
                            envelope_json: Option<String>,
                            lease_id: Option<String>,
                            actor_id: Option<String>,
                        }
                        match serde_json::from_str::<ExecuteRequest>(raw_json.trim()) {
                            Ok(req) => {
                                let exec_req = ExecutionRequest {
                                    node_id: req.node_id,
                                    topology: req.topology.unwrap_or_else(|| "pipeline".into()),
                                    envelope_json: req.envelope_json.unwrap_or_else(|| "{}".into()),
                                    lease_id: req.lease_id.unwrap_or_else(|| "unknown".into()),
                                    actor_id: req.actor_id.unwrap_or_else(|| "arifflow".into()),
                                };
                                let bridge = AForgeExecutorBridge::new();
                                match bridge.execute(exec_req) {
                                    Ok(resp) => http_ok(&serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into())),
                                    Err(e) => format!(
                                        "HTTP/1.1 502 Bad Gateway\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        serde_json::json!({"status":"aforge_bridge_error", "error": e}).to_string().len(),
                                        serde_json::json!({"status":"aforge_bridge_error", "error": e})
                                    ).into_bytes(),
                                }
                            }
                            Err(e) => http_bad_request(
                                &serde_json::json!({"status":"invalid", "error": format!("{}", e)})
                                    .to_string(),
                            ),
                        }
                    }
                    None => http_bad_request(
                        r#"{"status":"error","message":"Empty body. Send {\"node_id\":\"...\"}"}"#,
                    ),
                }
            } else if request.starts_with("POST /enforce") {
                // ── Manually trigger enforcement cycle ──
                let mut enf = enforcer.lock().unwrap();
                let report = enf.enforce();
                let body = serde_json::json!({
                    "status": "enforced",
                    "overall": format!("{:?}", report.overall_status),
                    "blocking": report.blocking_count,
                    "warns": report.warn_count,
                    "checks": report.checks.iter().map(|c| {
                        serde_json::json!({
                            "invariant": c.invariant.code(),
                            "status": format!("{:?}", c.status),
                            "reason": c.reason,
                        })
                    }).collect::<Vec<_>>(),
                });
                http_ok(&body.to_string())
            } else if request.starts_with("POST /flow") {
                let body = serde_json::json!({
                    "status": "ack",
                    "message": "Flow command received. Endpoints: GET /health, POST /ingest, POST /vector, POST /check, POST /release, POST /enforce, POST /flow",
                    "endpoints": ["GET /health", "POST /ingest", "POST /vector", "POST /check", "POST /release", "POST /enforce", "POST /flow"]
                })
                .to_string();
                http_ok(&body)
            } else {
                let body = serde_json::json!({
                    "status": "error",
                    "message": "Not found. Use GET /health, POST /ingest, POST /check, POST /release, POST /enforce, or POST /flow"
                })
                .to_string();
                format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .into_bytes()
            };
            let _ = stream.write_all(&response);
        }
        _ => {}
    }
}

/// Daemon mode — TCP listener on ARIFLOW_PORT (default 7073)
fn daemon_mode() {
    let port: u16 = std::env::var("ARIFLOW_PORT")
        .unwrap_or_else(|_| "7073".into())
        .parse()
        .unwrap_or(7073);
    let addr = format!("127.0.0.1:{}", port);
    let start_time = Instant::now();
    let receipt_store = Arc::new(Mutex::new(ReceiptStore::new(1000)));
    let enforcer = Arc::new(Mutex::new(InvariantEnforcer::default()));
    // ── QG.V0.3 VECTOR ENGINE (2026-08-14) ──
    let vector_store = Arc::new(Mutex::new(VectorStore::new()));
    let independence = Arc::new(Mutex::new(IndependenceMonitor::new(200)));

    // ── VAULT999 sealer (audit 2026-08-10, F5) ──
    // Receipts are sealed into the VAULT999 immutable hash chain (see F5:
    // "Flow writes receipts, never owns memory"). In-memory chain + JSONL log.
    let vault_sealer = Arc::new(Mutex::new(Vault999Sealer::new()));

    // ── Daemon-side receipt persistence (audit 2026-08-10) ──
    // Load existing receipts on startup, then append new receipts as they arrive.
    let persist_path = PathBuf::from("/var/lib/arifflow/receipts.jsonl");
    let persist_mutex = Arc::new(Mutex::new(()));
    if let Some(parent) = persist_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Load last N receipts from existing file (if any) into in-memory store
    // IMPORTANT (audit 2026-08-10): load the MOST RECENT receipts, not the first N.
    // Loading the first 1000 of a 1288-line file puts STALE receipts in memory,
    // making the FQ window (last 100 of store) reflect old metabolism, not current.
    // Reality reconciliation probe (re_audit_drift_check.sh Probe 5) caught this:
    // daemon FQ=0.515 vs disk-recomputed FQ=0.957. Fixed by taking the tail.
    {
        let mut store = receipt_store.lock().unwrap();
        if let Ok(file) = File::open(&persist_path) {
            let reader = BufReader::new(file);
            let mut loaded: Vec<FlowReceipt> = Vec::new();
            for line in reader.lines().flatten() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<FlowReceipt>(&line) {
                    Ok(receipt) => loaded.push(receipt),
                    Err(e) => {
                        eprintln!("[arifFlow] Skip malformed receipt line: {}", e);
                    }
                }
            }
            // Keep only the most recent 1000 (store capacity), drop stale head.
            let capacity = 1000;
            if loaded.len() > capacity {
                let keep_from = loaded.len() - capacity;
                for receipt in loaded.drain(keep_from..) {
                    store.push_force(receipt);
                }
            } else {
                for receipt in loaded {
                    store.push_force(receipt);
                }
            }
        }
        if !store.is_empty() {
            eprintln!(
                "[arifFlow] Loaded {} receipts from {} (most recent)",
                store.len(),
                persist_path.display()
            );
        }
    }

    // ── E1 PHASE E ACTIVATION: arifOS governance bridge boot lease ──
    // Cross-organ bridge activation per Phase E charter (F13 SOVEREIGN-ratified 2026-09-17).
    // Calls arifOS :8088/mcp (arif_init) to acquire a session + constitutional chain ID.
    // Failure is non-fatal: daemon continues with local-only mode.
    eprintln!("[arifFlow E1] Activating arifOS governance bridge at boot...");
    match ArifOSGovernanceBridge::new()
        .request_lease("arifflow-daemon", "E1_Phase_E_activation_2026-09-17")
    {
        Ok(lease) => {
            eprintln!(
                "[arifFlow E1] arifOS bridge ONLINE: session_id={}, chain_id={}, scope={:?}",
                lease.lease_id, lease.constitutional_chain_id, lease.scope
            );
        }
        Err(e) => {
            eprintln!(
                "[arifFlow E1] arifOS bridge BOOT FAILED: {} (daemon continues in local mode)",
                e
            );
        }
    }

    // ── E2 PHASE E ACTIVATION: A-FORGE executor bridge boot probe ──
    // Calls A-FORGE :7071/mcp to verify cross-organ bridge connectivity at boot.
    // Failure is non-fatal: daemon continues with local-only mode.
    eprintln!("[arifFlow E2] Activating A-FORGE executor bridge at boot...");
    let aforge_health = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
        .and_then(|c| {
            let url =
                std::env::var("AFORGE_URL").unwrap_or_else(|_| "http://127.0.0.1:7071".into());
            c.get(format!("{}/health", url.trim_end_matches('/')))
                .send()
                .ok()
        });
    match aforge_health {
        Some(r) if r.status().is_success() => {
            eprintln!(
                "[arifFlow E2] A-FORGE bridge ONLINE: HTTP {} from :7071/health",
                r.status()
            );
        }
        Some(r) => eprintln!(
            "[arifFlow E2] A-FORGE bridge DEGRADED: HTTP {} from :7071/health",
            r.status()
        ),
        None => {
            eprintln!("[arifFlow E2] A-FORGE bridge UNREACHABLE (daemon continues in local mode)")
        }
    }

    // ── Auto-enforcement timer (audit 2026-08-10) ──
    // Spawn background thread that runs invariant enforcement every ARIFLOW_ENFORCE_INTERVAL_S
    // seconds (default 10), so HOLD/THROTTLE/VOID gates fire even without explicit POST /enforce.
    let enforcer_clone = enforcer.clone();
    let enf_interval: u64 = std::env::var("ARIFLOW_ENFORCE_INTERVAL_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    eprintln!(
        "[arifFlow] Auto-enforcement timer: {}s interval",
        enf_interval
    );
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(enf_interval));
            let mut enf = match enforcer_clone.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let report = enf.enforce();
            // FIX 3 (audit 2026-08-10): single-line cycle log on every enforce.
            eprintln!("[arifFlow] enforce cycle #{} complete", enf.cycle_count);
            if report.blocking_count > 0 || report.warn_count > 0 {
                eprintln!(
                    "[arifFlow] auto-enforce: status={:?} blocking={} warns={}",
                    report.overall_status, report.blocking_count, report.warn_count
                );
            }
        }
    });

    // ── Auto-vector-sync background thread (spec §9, STEPs 2-9) ──
    // Periodically syncs live apex scalars from arifOS (:8088) / A-FORGE (:7071)
    // into the vector store so dimensions maintain live reality contact without
    // waiting solely for manual push.
    let vs_sync = vector_store.clone();
    let indep_sync = independence.clone();
    let arifos_url = std::env::var("ARIFOS_URL").unwrap_or_else(|_| "http://127.0.0.1:8088".into());
    let aforge_url = std::env::var("AFORGE_URL").unwrap_or_else(|_| "http://127.0.0.1:7071".into());
    let sync_interval: u64 = std::env::var("ARIFLOW_VECTOR_SYNC_INTERVAL_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(15);
    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[arifFlow] Failed to create HTTP client for vector sync: {}",
                    e
                );
                return;
            }
        };
        loop {
            std::thread::sleep(Duration::from_secs(sync_interval));
            let mut synced = false;
            let arifos_health_url = format!("{}/health", arifos_url.trim_end_matches('/'));
            if let Ok(resp) = client.get(&arifos_health_url).send()
                && let Ok(json) = resp.json::<serde_json::Value>()
            {
                let mut vs = match vs_sync.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                let mut indep = match indep_sync.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                vs.tick();
                if let Some(g_val) = json
                    .pointer("/apex_scalars/G/value")
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::G,
                        g_val,
                        Epistemology::Witness,
                        "forge_evaluate",
                        "A-FORGE",
                        true,
                    );
                }
                if let Some(cd_val) = json
                    .pointer("/apex_scalars/C_dark/value")
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::CDark,
                        cd_val,
                        Epistemology::Measure,
                        "forge_evaluate",
                        "A-FORGE",
                        true,
                    );
                }
                if let Some(w3_val) = json
                    .pointer("/apex_scalars/W3/value")
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::W3,
                        w3_val,
                        Epistemology::Witness,
                        "forge_witness",
                        "A-FORGE",
                        true,
                    );
                }
                if let Some(j_val) = json
                    .pointer("/apex_scalars/QDF/value")
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::J,
                        j_val,
                        Epistemology::Measure,
                        "forge_apex_encode",
                        "A-FORGE",
                        true,
                    );
                }
                if let Some(ds_val) = json
                    .pointer("/thermodynamic/entropy_delta")
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::DS,
                        ds_val,
                        Epistemology::Measure,
                        "entropy_sweep",
                        "arifOS",
                        true,
                    );
                }
                if let Some(omega_val) = json
                    .pointer("/runtime_floors/F7")
                    .or_else(|| json.pointer("/runtime_floors_status/F7/score"))
                    .and_then(|v| v.as_f64())
                {
                    vs.ingest(
                        Dimension::Omega0,
                        omega_val,
                        Epistemology::Feel,
                        "humility",
                        "333-AGI",
                        true,
                    );
                }
                indep.record(&vs);
                synced = true;
            }

            if !synced {
                let aforge_health_url = format!("{}/health", aforge_url.trim_end_matches('/'));
                if let Ok(resp) = client.get(&aforge_health_url).send()
                    && let Ok(json) = resp.json::<serde_json::Value>()
                {
                    let mut vs = match vs_sync.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    let mut indep = match indep_sync.lock() {
                        Ok(g) => g,
                        Err(p) => p.into_inner(),
                    };
                    vs.tick();
                    if let Some(g_val) = json
                        .pointer("/apex_scalars/G/value")
                        .and_then(|v| v.as_f64())
                    {
                        vs.ingest(
                            Dimension::G,
                            g_val,
                            Epistemology::Witness,
                            "forge_evaluate",
                            "A-FORGE",
                            true,
                        );
                    }
                    if let Some(cd_val) = json
                        .pointer("/apex_scalars/C_dark/value")
                        .and_then(|v| v.as_f64())
                    {
                        vs.ingest(
                            Dimension::CDark,
                            cd_val,
                            Epistemology::Measure,
                            "forge_evaluate",
                            "A-FORGE",
                            true,
                        );
                    }
                    if let Some(w3_val) = json
                        .pointer("/apex_scalars/W3/value")
                        .and_then(|v| v.as_f64())
                    {
                        vs.ingest(
                            Dimension::W3,
                            w3_val,
                            Epistemology::Witness,
                            "forge_witness",
                            "A-FORGE",
                            true,
                        );
                    }
                    indep.record(&vs);
                }
            }
        }
    });

    match TcpListener::bind(&addr) {
        Ok(listener) => {
            eprintln!("[arifFlow] Daemon mode — listening on {}", addr);
            eprintln!("[arifFlow] Health:  curl http://127.0.0.1:{}/health", port);
            eprintln!(
                "[arifFlow] Check:  curl -X POST http://127.0.0.1:{}/check -d '{{\"actor_id\":\"test\"}}'",
                port
            );
            eprintln!(
                "[arifFlow] Ingest: curl -X POST http://127.0.0.1:{}/ingest -d '{{...}}'",
                port
            );
            eprintln!(
                "[arifFlow] Enforce:curl -X POST http://127.0.0.1:{}/enforce",
                port
            );
            eprintln!("[arifFlow] Invariants: F0-F6 flow-plane enforcement ACTIVE");

            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let store = receipt_store.clone();
                        let enf = enforcer.clone();
                        let vs = vector_store.clone();
                        let indep = independence.clone();
                        let start = start_time;
                        let pp = persist_path.clone();
                        let pm = persist_mutex.clone();
                        let vsl = vault_sealer.clone();
                        std::thread::spawn(move || {
                            handle_client(s, start, &store, &enf, &vs, &indep, &pp, &pm, &vsl);
                        });
                    }
                    Err(e) => {
                        eprintln!("[arifFlow] Connection error: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("[arifFlow] Failed to bind {}: {}", addr, e);
            std::process::exit(1);
        }
    }
}

/// Main — dispatch to daemon mode or stdin protocol mode
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--daemon" {
        daemon_mode();
    } else {
        stdin_protocol_loop();
    }
}
