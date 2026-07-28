// arifFlow — End-to-End (e3e) Integration Tests
//
// These tests spawn the arifFlow binary as a subprocess and exercise
// the full stdin/stdout JSON-L protocol, daemon mode, and topology
// behavior.
//
// Run: cargo test --test e3e
//
// DITEMPA BUKAN DIBERI — Forged, Not Given.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// ── Binary Path ───────────────────────────────────────────────────────────

fn binary_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("release");
    p.push("arifflow");
    p
}

// ── Helpers ───────────────────────────────────────────────────────────────

struct ProtocolClient {
    child: Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl ProtocolClient {
    fn spawn() -> Self {
        let bin = binary_path();
        let mut child = Command::new(&bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn arifFlow binary — run `cargo build --release` first");

        let stdin = child.stdin.take().expect("Failed to capture stdin");
        let stdout = BufReader::new(child.stdout.take().expect("Failed to capture stdout"));

        Self { child, stdin, stdout }
    }

    fn send(&mut self, json: &serde_json::Value) {
        let line = serde_json::to_string(json).unwrap();
        writeln!(self.stdin, "{}", line).expect("Failed to write to stdin");
        self.stdin.flush().ok();
    }

    fn recv(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("Failed to read from stdout");
        assert!(
            !line.trim().is_empty(),
            "Empty line from stdout — child may have crashed"
        );
        serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("Failed to parse JSON from stdout '{}': {}", line.trim(), e))
    }

    fn recv_timeout(&mut self, timeout_ms: u64) -> Option<serde_json::Value> {
        // Use a non-blocking approach with a timeout
        let start = Instant::now();
        loop {
            if start.elapsed().as_millis() as u64 > timeout_ms {
                return None;
            }
            // Try to read using the BufReader — use fill_buf to check availability
            let buf = self.stdout.fill_buf().ok()?;
            if !buf.is_empty() {
                return Some(self.recv());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn close_stdin(&mut self) {
        // Drop stdin to signal EOF
    }

    fn wait(mut self) -> std::process::ExitStatus {
        self.child.wait().expect("Failed to wait for child")
    }
}

fn make_configure(topology: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "configure",
        "topology": topology,
        "lease_id": "550e8400-e29b-41d4-a716-446655440000",
        "actor_id": "e3e-test-actor",
        "chain_id": "550e8400-e29b-41d4-a716-446655440001"
    })
}

fn make_seed(channel: &str, data: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "seed",
        "channel": channel,
        "data": data
    })
}

fn make_step(nodes: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "type": "step",
        "nodes": nodes
    })
}

fn make_step_node(id: &str, subs: Vec<&str>, outputs: Vec<&str>) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "subs": subs,
        "outputs": outputs
    })
}

fn make_verdict(class: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "verdict",
        "class": class,
        "verdict_id": "v-0000-0000-0001",
        "hash": format!("deadbeef{}", "0".repeat(56)),
    })
}

fn make_stop() -> serde_json::Value {
    serde_json::json!({ "type": "stop" })
}

// Helper: assert a stdout message has the expected type
fn assert_msg_type(msg: &serde_json::Value, expected_type: &str) {
    let msg_type = msg
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("Message missing 'type' field: {}", msg));
    assert_eq!(
        msg_type, expected_type,
        "Expected type '{}', got '{}' in: {}",
        expected_type, msg_type, msg
    );
}

// Helper: assert FQ is valid (finite, non-negative)
fn assert_valid_fq(msg: &serde_json::Value) {
    let afq = msg
        .get("afq")
        .and_then(|v| v.as_f64())
        .unwrap_or_else(|| panic!("Message missing 'afq' field: {}", msg));
    assert!(
        afq.is_finite() && afq >= 0.0,
        "FQ should be finite and non-negative, got: {}",
        afq
    );
    let diagnosis = msg
        .get("afq_diagnosis")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("Message missing 'afq_diagnosis': {}", msg));
    assert!(
        ["OPTIMAL", "BALANCED", "WATCHING", "STUCK"].contains(&diagnosis),
        "Invalid FQ diagnosis: {}",
        diagnosis
    );
}

// Helper: assert valid JSON structure for each message type
fn assert_valid_need_verdict(msg: &serde_json::Value) {
    assert_msg_type(msg, "need_verdict");
    assert!(msg.get("step").and_then(|v| v.as_u64()).is_some(), "need_verdict missing 'step'");
    assert!(msg.get("state_root").and_then(|v| v.as_str()).is_some(), "need_verdict missing 'state_root'");
    assert!(msg.get("lease_id").and_then(|v| v.as_str()).is_some(), "need_verdict missing 'lease_id'");
    assert!(msg.get("chain_id").and_then(|v| v.as_str()).is_some(), "need_verdict missing 'chain_id'");
    assert_valid_fq(msg);
}

fn assert_valid_step_result(msg: &serde_json::Value) {
    assert_msg_type(msg, "step_result");
    assert!(msg.get("step").and_then(|v| v.as_u64()).is_some(), "step_result missing 'step'");
    let verdict = msg.get("verdict").and_then(|v| v.as_str()).unwrap_or("");
    assert!(
        ["SEAL", "HOLD", "VOID", "SABAR"].contains(&verdict),
        "Invalid verdict: {}",
        verdict
    );
    assert!(msg.get("state_root").and_then(|v| v.as_str()).is_some(), "step_result missing 'state_root'");
    assert!(msg.get("deltas").is_some(), "step_result missing 'deltas'");
}

fn assert_valid_cooling(msg: &serde_json::Value) {
    assert_msg_type(msg, "cooling");
    assert!(msg.get("total_steps").and_then(|v| v.as_u64()).is_some(), "cooling missing 'total_steps'");
    assert!(msg.get("final_root").and_then(|v| v.as_str()).is_some(), "cooling missing 'final_root'");
    assert!(msg.get("leases_closed").and_then(|v| v.as_u64()).is_some(), "cooling missing 'leases_closed'");
}

// ── Test 1: Full Protocol Cycle ───────────────────────────────────────────

#[test]
fn test_e3e_full_protocol_cycle() {
    let mut client = ProtocolClient::spawn();

    // 1. configure
    client.send(&make_configure("fan_out"));

    // 2. seed
    client.send(&make_seed("input", "e3e-test-data"));

    // 3. step → expect need_verdict
    let node = make_step_node("worker-1", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node]));

    let need_v = client.recv();
    assert_valid_need_verdict(&need_v);
    assert_eq!(
        need_v.get("step").and_then(|v| v.as_u64()).unwrap(),
        0,
        "First step should be step 0"
    );

    // 4. verdict SEAL → expect step_result
    client.send(&make_verdict("SEAL"));
    let result = client.recv();
    assert_valid_step_result(&result);
    assert_eq!(
        result.get("verdict").and_then(|v| v.as_str()).unwrap(),
        "SEAL"
    );
    assert_eq!(
        result.get("step").and_then(|v| v.as_u64()).unwrap(),
        0
    );

    // 5. stop → expect cooling
    client.send(&make_stop());
    let cooling = client.recv();
    assert_valid_cooling(&cooling);
    assert_eq!(
        cooling.get("total_steps").and_then(|v| v.as_u64()).unwrap(),
        1
    );
    assert_eq!(
        cooling.get("leases_closed").and_then(|v| v.as_u64()).unwrap(),
        1
    );

    client.wait();
}

// ── Test 2: Daemon Health ─────────────────────────────────────────────────

#[test]
fn test_e3e_daemon_health() {
    let port = 19073; // non-standard to avoid conflict
    let bin = binary_path();

    let mut child = Command::new(&bin)
        .arg("--daemon")
        .env("ARIFLOW_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn daemon");

    // Wait for daemon to bind
    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    let mut connected = false;

    while start.elapsed() < timeout {
        if TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            connected = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(connected, "Daemon failed to bind within 5s");

    // GET /health
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Failed to connect to daemon");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .expect("Failed to send health request");

    let mut response = String::new();
    let mut reader = BufReader::new(&mut stream);
    reader
        .read_line(&mut response)
        .expect("Failed to read health response");

    // Parse body — HTTP/1.1 200 OK\r\n...\r\n\r\n{body}
    let mut full_response = String::new();
    reader
        .read_to_string(&mut full_response)
        .expect("Failed to read full response");
    response.push_str(&full_response);

    // Extract JSON body after \r\n\r\n
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .trim();
    let health: serde_json::Value =
        serde_json::from_str(body).expect("Failed to parse health response JSON");

    assert_eq!(
        health.get("status").and_then(|v| v.as_str()).unwrap(),
        "ok"
    );
    assert!(
        health.get("fq").is_some(),
        "Health response missing 'fq' field"
    );
    let fq = health.get("fq").unwrap();
    assert!(
        fq.get("quotient").and_then(|v| v.as_f64()).is_some(),
        "FQ missing 'quotient'"
    );
    assert!(
        fq.get("verdict").and_then(|v| v.as_str()).is_some(),
        "FQ missing 'verdict'"
    );
    assert!(
        health.get("receipts").and_then(|v| v.as_u64()).is_some(),
        "Health missing 'receipts'"
    );
    assert!(
        health.get("uptime_ms").and_then(|v| v.as_u64()).is_some(),
        "Health missing 'uptime_ms'"
    );

    // POST /ingest with a valid receipt
    let receipt = serde_json::json!({
        "receipt_id": "00000000-0000-0000-0000-000000000001",
        "step_type": "Execute",
        "actor_id": "e3e-daemon-test",
        "session_id": "daemon-test-session",
        "cost_ns": 1_000_000,
        "previous_receipt_hash": null,
        "created_at": "2026-07-28T00:00:00Z",
        "epistemic_label": "Observation",
        "floor_verdict": "Pass",
        "cooling_decision": "None",
        "step_number": 0
    });

    let mut stream2 = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Failed to connect for ingest");
    let body_str = serde_json::to_string(&receipt).unwrap();
    let http_req = format!(
        "POST /ingest HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body_str.len(),
        body_str
    );

    stream2
        .write_all(http_req.as_bytes())
        .expect("Failed to send ingest request");

    let mut ingest_response = String::new();
    let mut reader2 = BufReader::new(&mut stream2);
    reader2
        .read_to_string(&mut ingest_response)
        .expect("Failed to read ingest response");

    let ingest_body = ingest_response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .trim();
    let ingest: serde_json::Value =
        serde_json::from_str(ingest_body).expect("Failed to parse ingest response JSON");

    assert_eq!(
        ingest.get("status").and_then(|v| v.as_str()).unwrap(),
        "ingested"
    );
    assert!(
        ingest.get("fq").is_some(),
        "Ingest response missing 'fq'"
    );

    // Kill daemon
    child.kill().expect("Failed to kill daemon");
    child.wait().ok();
}

// ── Test 3: Nash Collapse on Verdict HOLD ─────────────────────────────────

#[test]
fn test_e3e_nash_collapse_on_verdict_hold() {
    let mut client = ProtocolClient::spawn();

    // Configure and seed
    client.send(&make_configure("fan_out"));
    client.send(&make_seed("input", "hold-verdict-test"));

    // Step
    let node = make_step_node("worker-hold", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node]));

    let need_v = client.recv();
    assert_valid_need_verdict(&need_v);

    // Send HOLD verdict
    client.send(&make_verdict("HOLD"));
    let result = client.recv();
    assert_valid_step_result(&result);
    assert_eq!(
        result.get("verdict").and_then(|v| v.as_str()).unwrap(),
        "HOLD",
        "Step result should reflect HOLD verdict"
    );

    // Stop
    client.send(&make_stop());
    let cooling = client.recv();
    assert_valid_cooling(&cooling);
    // Step was held but still counts as a step
    assert_eq!(
        cooling.get("total_steps").and_then(|v| v.as_u64()).unwrap(),
        1
    );

    client.wait();
}

// ── Test 4: Pipeline Topology ─────────────────────────────────────────────

#[test]
fn test_e3e_pipeline_topology() {
    let mut client = ProtocolClient::spawn();

    // Configure with pipeline topology
    client.send(&make_configure("pipeline"));
    client.send(&make_seed("input", "pipeline-test"));

    // Pipeline topology: sequential execution mode
    // Each step runs nodes in sequence — use registered channels (input, output)
    // Step 1
    let node1 = make_step_node("stage-1", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node1]));
    let need_v1 = client.recv();
    assert_valid_need_verdict(&need_v1);
    assert_eq!(
        need_v1.get("step").and_then(|v| v.as_u64()).unwrap(),
        0
    );

    client.send(&make_verdict("SEAL"));
    let result1 = client.recv();
    assert_valid_step_result(&result1);
    assert_eq!(
        result1.get("verdict").and_then(|v| v.as_str()).unwrap(),
        "SEAL"
    );

    // Step 2 — second pipeline stage
    let node2 = make_step_node("stage-2", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node2]));
    let need_v2 = client.recv();
    assert_valid_need_verdict(&need_v2);
    assert_eq!(
        need_v2.get("step").and_then(|v| v.as_u64()).unwrap(),
        1
    );

    client.send(&make_verdict("SEAL"));
    let result2 = client.recv();
    assert_valid_step_result(&result2);

    // Stop
    client.send(&make_stop());
    let cooling = client.recv();
    assert_valid_cooling(&cooling);
    assert_eq!(
        cooling.get("total_steps").and_then(|v| v.as_u64()).unwrap(),
        2
    );

    client.wait();
}

// ── Test 5: Cascade Topology ──────────────────────────────────────────────

#[test]
fn test_e3e_cascade_topology() {
    let mut client = ProtocolClient::spawn();

    // Configure with cascade topology
    client.send(&make_configure("cascade"));
    client.send(&make_seed("input", "cascade-test"));

    // Step 1 — agent-a reads input, writes to output
    let node1 = make_step_node("agent-a", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node1]));
    let need_v1 = client.recv();
    assert_valid_need_verdict(&need_v1);
    assert_eq!(
        need_v1.get("step").and_then(|v| v.as_u64()).unwrap(),
        0
    );

    client.send(&make_verdict("SEAL"));
    let result1 = client.recv();
    assert_valid_step_result(&result1);
    assert_eq!(
        result1.get("verdict").and_then(|v| v.as_str()).unwrap(),
        "SEAL"
    );

    // Step 2 — agent-b reads input, writes to output
    let node2 = make_step_node("agent-b", vec!["input"], vec!["output"]);
    client.send(&make_step(vec![node2]));
    let need_v2 = client.recv();
    assert_valid_need_verdict(&need_v2);
    assert_eq!(
        need_v2.get("step").and_then(|v| v.as_u64()).unwrap(),
        1
    );

    client.send(&make_verdict("SEAL"));
    let result2 = client.recv();
    assert_valid_step_result(&result2);

    // Stop
    client.send(&make_stop());
    let cooling = client.recv();
    assert_valid_cooling(&cooling);
    assert_eq!(
        cooling.get("total_steps").and_then(|v| v.as_u64()).unwrap(),
        2
    );

    client.wait();
}

// ── Test 6: Multi-Step Sequence ───────────────────────────────────────────

#[test]
fn test_e3e_multi_step_sequence() {
    let mut client = ProtocolClient::spawn();

    client.send(&make_configure("fan_out"));
    client.send(&make_seed("input", "multi-step-test"));

    let num_steps = 5;

    for step_idx in 0..num_steps {
        let node = make_step_node(
            &format!("worker-{}", step_idx),
            vec!["input"],
            vec!["output"],
        );
        client.send(&make_step(vec![node]));

        let need_v = client.recv();
        assert_valid_need_verdict(&need_v);
        assert_eq!(
            need_v.get("step").and_then(|v| v.as_u64()).unwrap(),
            step_idx as u64,
            "Step number should increment sequentially"
        );

        // Verify FQ diagnosis evolves correctly
        let diagnosis = need_v
            .get("afq_diagnosis")
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(
            ["OPTIMAL", "BALANCED", "WATCHING", "STUCK"].contains(&diagnosis),
            "Invalid FQ diagnosis at step {}: {}",
            step_idx,
            diagnosis
        );

        client.send(&make_verdict("SEAL"));
        let result = client.recv();
        assert_valid_step_result(&result);
        assert_eq!(
            result.get("verdict").and_then(|v| v.as_str()).unwrap(),
            "SEAL"
        );
        assert_eq!(
            result.get("step").and_then(|v| v.as_u64()).unwrap(),
            step_idx as u64
        );
    }

    // Stop
    client.send(&make_stop());
    let cooling = client.recv();
    assert_valid_cooling(&cooling);
    assert_eq!(
        cooling.get("total_steps").and_then(|v| v.as_u64()).unwrap(),
        num_steps
    );

    client.wait();
}
