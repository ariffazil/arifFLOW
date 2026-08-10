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

use arifflow::channel::ChannelMode;
use arifflow::governance::invariants::{EnforcerAction, FqThresholds, InvariantEnforcer};
use arifflow::receipt::{FlowReceipt, ReceiptStore};
use arifflow::scheduler::{FlowNode, SuperStepScheduler, TopologyKind, VerdictClass};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
    persist_path: &PathBuf,
    persist_mutex: &Arc<Mutex<()>>,
) {
    let mut buf = [0u8; 16384];
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            let request = String::from_utf8_lossy(&buf[..n]);
            let response = if request.starts_with("GET /health") {
                let store = receipt_store.lock().unwrap();
                let enf = enforcer.lock().unwrap();
                let fq = store.flow_quotient(100);
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
                let body = serde_json::json!({
                    "status": "ok",
                    "fq": {
                        "quotient": fq.quotient,
                        "verdict": format!("{}", fq.verdict),
                        "execute_count": fq.execute_count,
                        "verify_count": fq.verify_count,
                    },
                    "provenance": {
                        "formula_version": "qg.v0.2",
                        "formula_hash": "sha256:arifflow-fq-v2.1-2026-08-05",
                        "window_start_utc": start_time.elapsed().as_secs().to_string(),
                        "window_duration_s": 0,
                    },
                    "invariants": {
                        "cycle_count": enf.cycle_count,
                        "hold_count": enf.hold_count,
                        "throttle_count": enf.throttle_count,
                        "restricted_actors": restricted,
                    },
                    "receipts": store.len(),
                    "uptime_ms": start_time.elapsed().as_millis() as u64,
                })
                .to_string();
                http_ok(&body)
            } else if request.starts_with("POST /ingest") {
                match extract_body(&request) {
                    Some(raw_json) => match serde_json::from_str::<FlowReceipt>(raw_json.trim()) {
                        Ok(receipt) => {
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
                                    {
                                        if let Ok(line) = serde_json::to_string(&receipt) {
                                            let _ = writeln!(file, "{}", line);
                                        }
                                    }
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
                            // Ingest into invariant enforcer
                            enf.ingest(&receipt);
                            let fq = store.flow_quotient(20);
                            let body = serde_json::json!({
                                "status": "ingested",
                                "actor": receipt.actor_id,
                                "step_type": format!("{}", receipt.step_type),
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
            } else if request.starts_with("POST /check") {
                // ── INVARIANT GATE: Check if actor is allowed to execute ──
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct CheckRequest {
                            actor_id: String,
                        }
                        match serde_json::from_str::<CheckRequest>(raw_json.trim()) {
                            Ok(req) => {
                                let enf = enforcer.lock().unwrap();
                                let (allowed, reason, action) = enf.check_actor(&req.actor_id);
                                let body = serde_json::json!({
                                    "actor": req.actor_id,
                                    "allowed": allowed,
                                    "reason": reason,
                                    "action": format!("{:?}", action),
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
                // FIX 5 (audit 2026-08-10): require requester_id, deny self-release,
                // and restrict to F13 SOVEREIGN ("arif") or external verifier.
                // NOTE: plaintext requester_id is defense-in-depth, not a substitute
                // for SCT-token-based cryptographic identity (deferred to F13 design).
                match extract_body(&request) {
                    Some(raw_json) => {
                        #[derive(Deserialize)]
                        struct ReleaseRequest {
                            actor_id: String,
                            requester_id: String,
                        }
                        match serde_json::from_str::<ReleaseRequest>(raw_json.trim()) {
                            Ok(req) => {
                                // ── Self-release denied: an actor cannot release
                                // its own hold without external verification. ──
                                if req.requester_id == req.actor_id {
                                    let body = serde_json::json!({
                                        "status": "forbidden",
                                        "reason": "self-release denied — external verification required",
                                        "actor": req.actor_id,
                                        "requester": req.requester_id,
                                    })
                                    .to_string();
                                    let _ = stream.write_all(
                                        &format!(
                                            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(), body
                                        )
                                        .into_bytes(),
                                    );
                                    return;
                                }
                                // ── Only F13 SOVEREIGN ("arif") or an external
                                // verifier may authorize a release. ──
                                if req.requester_id != "arif" {
                                    let body = serde_json::json!({
                                        "status": "forbidden",
                                        "reason": "release requires F13 sovereign or external verifier",
                                        "actor": req.actor_id,
                                        "requester": req.requester_id,
                                    })
                                    .to_string();
                                    let _ = stream.write_all(
                                        &format!(
                                            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                            body.len(), body
                                        )
                                        .into_bytes(),
                                    );
                                    return;
                                }
                                let mut enf = enforcer.lock().unwrap();
                                enf.release_hold(&req.actor_id);
                                let body = serde_json::json!({
                                    "status": "released",
                                    "actor": req.actor_id,
                                    "released_by": req.requester_id,
                                });
                                let _ = stream.write_all(&http_ok(&body.to_string()));
                            }
                            Err(e) => {
                                let body = serde_json::json!({"status": "invalid", "error": format!("{}", e)}).to_string();
                                let _ = stream.write_all(
                                    &format!(
                                        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                        body.len(), body
                                    )
                                    .into_bytes(),
                                );
                            }
                        }
                    }
                    None => {
                        let body = r#"{"status":"error","message":"Empty body. Send {\"actor_id\":\"...\",\"requester_id\":\"...\"}"}"#;
                        let _ = stream.write_all(
                            &format!(
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                body.len(), body
                            )
                            .into_bytes(),
                        );
                    }
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
                    "message": "Flow command received. Endpoints: GET /health, POST /ingest, POST /check, POST /release, POST /enforce, POST /flow",
                    "endpoints": ["GET /health", "POST /ingest", "POST /check", "POST /release", "POST /enforce", "POST /flow"]
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

    // ── Daemon-side receipt persistence (audit 2026-08-10) ──
    // Load existing receipts on startup, then append new receipts as they arrive.
    let persist_path = PathBuf::from("/var/lib/arifflow/receipts.jsonl");
    let persist_mutex = Arc::new(Mutex::new(()));
    if let Some(parent) = persist_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Load last N receipts from existing file (if any) into in-memory store
    {
        let mut store = receipt_store.lock().unwrap();
        if let Ok(file) = File::open(&persist_path) {
            let reader = BufReader::new(file);
            for line in reader.lines().flatten() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<FlowReceipt>(&line) {
                    Ok(receipt) => {
                        store.push_force(receipt);
                    }
                    Err(e) => {
                        eprintln!("[arifFlow] Skip malformed receipt line: {}", e);
                    }
                }
                if store.len() >= 1000 {
                    break;
                }
            }
        }
        if store.len() > 0 {
            eprintln!(
                "[arifFlow] Loaded {} receipts from {}",
                store.len(),
                persist_path.display()
            );
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
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(enf_interval));
        let mut enf = match enforcer_clone.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let report = enf.enforce();
        if report.blocking_count > 0 || report.warn_count > 0 {
            eprintln!(
                "[arifFlow] auto-enforce: status={:?} blocking={} warns={}",
                report.overall_status, report.blocking_count, report.warn_count
            );
        }
    });

    match TcpListener::bind(&addr) {
        Ok(listener) => {
            eprintln!("[arifFlow] Daemon mode — listening on {}", addr);
            eprintln!("[arifFlow] Health:  curl http://127.0.0.1:{}/health", port);
            eprintln!("[arifFlow] Check:  curl -X POST http://127.0.0.1:{}/check -d '{{\"actor_id\":\"test\"}}'", port);
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
                        let start = start_time;
                        let pp = persist_path.clone();
                        let pm = persist_mutex.clone();
                        std::thread::spawn(move || {
                            handle_client(s, start, &store, &enf, &pp, &pm);
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
