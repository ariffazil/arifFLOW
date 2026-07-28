// arifFlow — binary entry point for the governed parallel execution engine
//
// Two modes:
//   1) stdin/stdout JSON-L protocol (default) — for A-FORGE adapter / pipe usage
//   2) --daemon mode — TCP listener on ARIFLOW_PORT (default 7073) with health endpoint
//
// Reads JSON-L topology commands from stdin, routes to SuperStepScheduler,
// writes checkpoint/verdict envelopes to stdout.
//
// Protocol:
//   stdin:  {"type":"configure","topology":"fan_out","lease_id":"...",...}
//   stdin:  {"type":"seed","channel":"input","data":"..."}
//   stdin:  {"type":"step","nodes":[...]}
//   stdout: {"type":"need_verdict","step":0,"state_root":"...","lease_id":"...","chain_id":"..."}
//   stdin:  {"type":"verdict","class":"SEAL","verdict_id":"...","hash":"..."}
//   stdout: {"type":"step_result","step":0,"verdict":"SEAL","state_root":"...","deltas":{...}}
//   stdin:  {"type":"stop"}
//   stdout: {"type":"cooling","total_steps":3,"final_root":"...","leases_closed":1}
//
// Daemon mode:
//   GET /health → {"status":"ok","fq":{...},"receipts":N,"uptime_ms":...}
//   POST /flow  → JSON-L command (same as stdin protocol)
//
// DITEMPA BUKAN DIBERI — arifOS = law, arifFlow = flow, A-FORGE = hands

use arifflow::channel::ChannelMode;
use arifflow::receipt::FlowQuotient;
use arifflow::receipt::{FlowReceipt, ReceiptStore};
use arifflow::scheduler::{FlowNode, SuperStepScheduler, TopologyKind, VerdictClass};
use chrono;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Instant;

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
                            afq: result.fq.quotient,
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
                    "CANDIDATE" => VerdictClass::CANDIDATE,
                    "UNJUDGED" => VerdictClass::UNJUDGED,
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

// ── Cooling State Machine (GAP-M4/M6) ─────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum CoolingPhase {
    Active,
    Cooling,
    Notify,
    Sovereign,
}

impl CoolingPhase {
    fn label(&self) -> &'static str {
        match self {
            CoolingPhase::Active => "Active",
            CoolingPhase::Cooling => "Cooling",
            CoolingPhase::Notify => "Notify",
            CoolingPhase::Sovereign => "Sovereign",
        }
    }
}

#[derive(Debug, Clone)]
struct CoolingState {
    phase: CoolingPhase,
    stuck_since: Option<chrono::DateTime<chrono::Utc>>,
    phase_start: Option<chrono::DateTime<chrono::Utc>>,
}

impl CoolingState {
    fn new() -> Self {
        Self {
            phase: CoolingPhase::Active,
            stuck_since: None,
            phase_start: None,
        }
    }

    fn phase_label(&self) -> &'static str {
        self.phase.label()
    }

    fn t_remaining_s(&self, _start_time: Instant) -> u64 {
        let now = chrono::Utc::now();
        match self.phase {
            CoolingPhase::Cooling => {
                if let Some(ps) = self.phase_start {
                    let elapsed = now.signed_duration_since(ps).num_seconds() as u64;
                    300u64.saturating_sub(elapsed)
                } else {
                    300
                }
            }
            CoolingPhase::Notify => {
                if let Some(ps) = self.phase_start {
                    let elapsed = now.signed_duration_since(ps).num_seconds() as u64;
                    300u64.saturating_sub(elapsed)
                } else {
                    300
                }
            }
            _ => 0,
        }
    }

    fn update_from_fq(&mut self, fq: &FlowQuotient, _start_time: &Instant) {
        let now = chrono::Utc::now();
        if fq.verdict == arifflow::receipt::FlowVerdict::Stuck {
            match self.phase {
                CoolingPhase::Active => {
                    self.phase = CoolingPhase::Cooling;
                    self.stuck_since = Some(now);
                    self.phase_start = Some(now);
                }
                CoolingPhase::Cooling => {
                    if let Some(ps) = self.phase_start {
                        if now.signed_duration_since(ps).num_seconds() as u64 >= 300 {
                            self.phase = CoolingPhase::Notify;
                            self.phase_start = Some(now);
                        }
                    }
                }
                CoolingPhase::Notify => {
                    if let Some(ps) = self.phase_start {
                        if now.signed_duration_since(ps).num_seconds() as u64 >= 300 {
                            self.phase = CoolingPhase::Sovereign;
                            self.phase_start = Some(now);
                        }
                    }
                }
                CoolingPhase::Sovereign => { /* sticks until override */ }
            }
        } else {
            // FQ recovered — auto-resume
            if self.phase != CoolingPhase::Active {
                self.phase = CoolingPhase::Active;
                self.stuck_since = None;
                self.phase_start = None;
            }
        }
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
    cooling_state: &Arc<Mutex<CoolingState>>,
) {
    let mut buf = [0u8; 16384]; // larger buffer for POST bodies
    match stream.read(&mut buf) {
        Ok(n) if n > 0 => {
            let request = String::from_utf8_lossy(&buf[..n]);
            let response = if request.starts_with("GET /health") {
                let store = receipt_store.lock().unwrap();
                let all_receipts = store.all().to_vec();
                // Build timestamped FQ history for trend computation
                let fq_hist_raw: Vec<f64> = store.fq_history().to_vec();
                // For trend, pair each FQ value with a synthetic timestamp (now - index*10s)
                let now = chrono::Utc::now();
                let fq_timestamped: Vec<(f64, chrono::DateTime<chrono::Utc>)> = fq_hist_raw
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        let age_s = ((fq_hist_raw.len() - i) * 10) as i64;
                        (v, now - chrono::Duration::seconds(age_s))
                    })
                    .collect();
                let fq = FlowQuotient::compute_with_trend(&all_receipts, &fq_timestamped, 600);
                let cooling = cooling_state.lock().unwrap().clone();
                let body = serde_json::json!({
                    "status": "ok",
                    "fq": {
                        "quotient": fq.quotient,
                        "verdict": format!("{}", fq.verdict),
                        "verdict_emoji": fq.verdict.emoji(),
                        "execute_count": fq.execute_count,
                        "verify_count": fq.verify_count,
                        "execute_cost_ns": fq.execute_cost_ns,
                        "verify_cost_ns": fq.verify_cost_ns,
                        // GAP-M4: Formula transparency
                        "raw_ratio": if fq.raw_ratio.is_infinite() { serde_json::Value::Null } else { serde_json::json!(fq.raw_ratio) },
                        "is_smoothed": fq.is_smoothed,
                        "alpha": fq.alpha,
                        "window_s": fq.window_s,
                        "cost_clamp_ns": {"min": fq.cost_clamp_ns.0, "max": fq.cost_clamp_ns.1},
                        // GAP-M2: Actor-level FQ
                        "by_actor": fq.by_actor,
                        "worst_actor": fq.worst_actor,
                        "actor_count": fq.actor_count,
                        // GAP-M3: Trend
                        "trend": {
                            "direction": format!("{:?}", fq.trend.direction),
                            "rate_per_min": fq.trend.rate_per_min,
                            "volatility": fq.trend.volatility,
                            "samples": fq.trend.samples,
                            "window_s": fq.trend.window_s,
                        }
                    },
                    "fq_history": fq_hist_raw,
                    "receipts": store.len(),
                    "persist_path": store.persist_path().map(|p| p.display().to_string()),
                    "uptime_ms": start_time.elapsed().as_millis() as u64,
                    // Build identity — verifiable deployment provenance
                    "build": {
                        "version": env!("CARGO_PKG_VERSION"),
                        "git_commit": env!("GIT_COMMIT"),
                        "git_branch": env!("GIT_BRANCH"),
                        "build_timestamp": env!("BUILD_TIMESTAMP"),
                        "build_dirty": env!("BUILD_DIRTY").parse::<bool>().unwrap_or(true),
                    },
                    // GAP-M4/M6: Cooling state
                    "cooling": {
                        "phase": cooling.phase_label(),
                        "stuck_since": cooling.stuck_since.map(|t| t.to_rfc3339()),
                        "t_remaining_s": cooling.t_remaining_s(start_time),
                        "override_allowed": cooling.phase_label() == "Sovereign" || cooling.phase_label() == "Notify",
                    },
                })
                .to_string();
                http_ok(&body)
            } else if request.starts_with("GET /cooling/status") {
                let cooling = cooling_state.lock().unwrap().clone();
                let store = receipt_store.lock().unwrap();
                let fq = store.flow_quotient(20);
                let body = serde_json::json!({
                    "phase": cooling.phase_label(),
                    "stuck_since": cooling.stuck_since.map(|t| t.to_rfc3339()),
                    "t_remaining_s": cooling.t_remaining_s(start_time),
                    "fq": fq.quotient,
                    "fq_verdict": format!("{}", fq.verdict),
                    "override_allowed": true, // Arif can always override
                })
                .to_string();
                http_ok(&body)
            } else if request.starts_with("POST /cooling/override") {
                match extract_body(&request) {
                    Some(raw_json) => {
                        match serde_json::from_str::<serde_json::Value>(raw_json.trim()) {
                            Ok(val) => {
                                let source = val
                                    .get("source")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let signal =
                                    val.get("signal").and_then(|v| v.as_str()).unwrap_or("");
                                // GAP-M6: Authorized sources only
                                let authorized =
                                    matches!(source, "hermes" | "cockpit" | "arifos" | "test");
                                if !authorized {
                                    let body = serde_json::json!({
                                        "status": "forbidden",
                                        "reason": format!("Source '{}' not authorized", source),
                                    })
                                    .to_string();
                                    format!("HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).into_bytes()
                                } else {
                                    let mut cooling = cooling_state.lock().unwrap();
                                    let mut store = receipt_store.lock().unwrap();
                                    match signal {
                                        "jalan_terus" => {
                                            cooling.phase = CoolingPhase::Active;
                                            cooling.stuck_since = None;
                                            // Record SCAR
                                            let receipt = FlowReceipt::new_first(
                                                source,
                                                "cooling-override",
                                                arifflow::receipt::StepType::Cool,
                                                arifflow::receipt::EpistemicLabel::Seal,
                                                0,
                                            )
                                            .with_payload(serde_json::json!({
                                                "override": "jalan_terus", "source": source,
                                                "cooling_decision": "Overridden"
                                            }));
                                            store.push_force(receipt);
                                            let body = serde_json::json!({
                                                "status": "overridden",
                                                "new_fq": 1.0,
                                                "phase": "Active",
                                            })
                                            .to_string();
                                            http_ok(&body)
                                        }
                                        "tunggu" => {
                                            cooling.phase = CoolingPhase::Cooling;
                                            let body = serde_json::json!({
                                                "status": "extended",
                                                "phase": "Cooling",
                                                "t_remaining_s": cooling.t_remaining_s(start_time),
                                            })
                                            .to_string();
                                            http_ok(&body)
                                        }
                                        "verify_metrics" => {
                                            let body = serde_json::json!({
                                                "status": "ack",
                                                "message": "Override test successful",
                                            })
                                            .to_string();
                                            http_ok(&body)
                                        }
                                        _ => {
                                            let body = serde_json::json!({
                                                "status": "invalid",
                                                "reason": format!("Unknown signal: {}", signal),
                                            })
                                            .to_string();
                                            format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).into_bytes()
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let body = serde_json::json!({"status": "invalid", "error": format!("{}", e)}).to_string();
                                format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).into_bytes()
                            }
                        }
                    }
                    None => {
                        let body = r#"{"status":"error","message":"Empty body"}"#;
                        format!("HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).into_bytes()
                    }
                }
            } else if request.starts_with("POST /ingest") {
                match extract_body(&request) {
                    Some(raw_json) => match serde_json::from_str::<FlowReceipt>(raw_json.trim()) {
                        Ok(receipt) => {
                            let mut store = receipt_store.lock().unwrap();
                            store.push_force(receipt);
                            let fq = store.flow_quotient(20);
                            // GAP-M1/M4: Update cooling state based on FQ
                            {
                                let mut cooling = cooling_state.lock().unwrap();
                                cooling.update_from_fq(&fq, &start_time);
                            }
                            let body = serde_json::json!({
                                "status": "ingested",
                                "fq": {
                                    "quotient": fq.quotient,
                                    "verdict": format!("{}", fq.verdict),
                                    "execute_count": fq.execute_count,
                                    "verify_count": fq.verify_count,
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
            } else if request.starts_with("POST /flow") {
                let body = serde_json::json!({
                    "status": "ack",
                    "message": "Flow command received. Use POST /ingest for receipt ingestion.",
                    "endpoints": ["GET /health", "POST /ingest", "POST /flow"]
                })
                .to_string();
                http_ok(&body)
            } else {
                let body = serde_json::json!({
                    "status": "error",
                    "message": "Not found. Use GET /health, POST /ingest, or POST /flow"
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
    // P3-1: File-backed receipt persistence — survives restart
    let persist_dir =
        std::env::var("ARIFLOW_PERSIST_DIR").unwrap_or_else(|_| "/var/lib/arifflow".into());
    let _ = std::fs::create_dir_all(&persist_dir);
    let persist_path = std::path::PathBuf::from(&persist_dir).join("receipts.jsonl");
    let receipt_store = Arc::new(Mutex::new(ReceiptStore::new_with_persistence(
        1000,
        persist_path,
    )));
    let cooling_state = Arc::new(Mutex::new(CoolingState::new()));
    let loaded_count = receipt_store.lock().unwrap().len();
    if loaded_count > 0 {
        eprintln!(
            "[arifFlow] Loaded {} receipts from disk — persistence active",
            loaded_count
        );
    }

    match TcpListener::bind(&addr) {
        Ok(listener) => {
            eprintln!("[arifFlow] Daemon mode — listening on {}", addr);
            eprintln!("[arifFlow] Health: curl http://127.0.0.1:{}/health", port);
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => {
                        let store = receipt_store.clone();
                        let cooling = cooling_state.clone();
                        let start = start_time;
                        std::thread::spawn(move || {
                            handle_client(s, start, &store, &cooling);
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
