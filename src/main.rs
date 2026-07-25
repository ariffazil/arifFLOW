// arifFlow — binary entry point for the governed parallel execution engine
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
// DITEMPA BUKAN DIBERI — arifOS = law, arifFlow = flow, A-FORGE = hands

use ariflow::channel::ChannelMode;
use ariflow::scheduler::{FlowNode, SuperStepScheduler, TopologyKind, VerdictClass};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

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
    fn subscriptions(&self) -> Vec<ariflow::channel::ChannelId> {
        self.subs.iter().map(|s| ariflow::channel::ChannelId(s.clone())).collect()
    }
    fn run(
        &self,
        _inputs: BTreeMap<ariflow::channel::ChannelId, Vec<ariflow::channel::Message<String>>>,
        _lease_id: uuid::Uuid,
    ) -> Result<BTreeMap<ariflow::channel::ChannelId, String>, ariflow::scheduler::NodeError> {
        let mut out = BTreeMap::new();
        for o in &self.outputs {
            out.insert(ariflow::channel::ChannelId(o.clone()), format!("result_{}", self.id));
        }
        Ok(out)
    }
}

fn send(msg: &StdoutMsg) {
    let line = serde_json::to_string(msg).unwrap();
    println!("{}", line);
    io::stdout().flush().ok();
}

fn main() {
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
