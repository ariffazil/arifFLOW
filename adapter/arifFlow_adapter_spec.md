# arifFlow_adapter.py — Minimal Implementation Spec

> **Status:** Phase 2 forge target  
> **Sovereign:** Arif (F13)  
> **Constitutional constraint:** 888-HOLD on production deploy until FFI + timeout + crash recovery proven  
> **Target runtime:** Python 3.13+ (A-FORGE environment)

---

## 1. Architecture

```
Hermes / Agent
    │
    │  { topology, lanes, config } via stdin
    ▼
┌─────────────────────────────────────────────────┐
│           arifFlow_adapter.py                   │
│                                                  │
│  ┌──────────────┐    ┌──────────────────┐       │
│  │ Rust process  │◄───│  arif_judge      │       │
│  │ (arifFlow     │    │  FFI (Rust→MCP)  │       │
│  │  binary)      │    └──────────────────┘       │
│  │              │    ┌──────────────────┐       │
│  │ stdin: config│───►│  A-FORGE ACT     │       │
│  │ stdout:      │    │  FFI (Rust→MCP)  │       │
│  │  checkpoint  │    └──────────────────┘       │
│  └──────┬───────┘    ┌──────────────────┐       │
│         │───────────►│  VAULT999 sealer  │       │
│         │            └──────────────────┘       │
│         │            ┌──────────────────┐       │
│         │───────────►│  Kabarkan tracer  │       │
│         │            └──────────────────┘       │
└─────────────────────────────────────────────────┘
    │
    ▼
  stdout: { verdict, checkpoint, cooling_receipt }
```

The adapter is a **process supervisor**. It:
1. Spawns the Rust `ariflow` binary as a subprocess
2. Sends topology definitions via stdin (JSON-L)
3. Receives checkpoint envelopes via stdout (JSON-L)
4. Calls real arifOS `arif_judge` per super-step
5. Calls A-FORGE ACT phases per node execution
6. Writes VAULT999 receipts
7. Emits Kabarkan events
8. Handles crash recovery

---

## 2. Message Format (JSON-L)

### 2.1 Adapter → Rust (stdin)

```jsonl
{"type": "configure", "topology": "fan_out", "lease_id": "uuid", "actor_id": "arif", "chain_id": "uuid"}
{"type": "seed", "channel": "input", "data": "...base64 or json..."}
{"type": "step", "nodes": [{"id": "geo", "subs": ["input"], "outputs": ["ch_geo"]}], "verdict": {"class": "SEAL", "verdict_id": "uuid", "hash": "hex"}}
{"type": "step", "nodes": [...]}
{"type": "stop"}
```

### 2.2 Rust → Adapter (stdout)

```jsonl
{"type": "checkpoint", "step": 0, "state_root": "hex", "channels": {"ch_geo": "hex"}, "previous_hash": "hex"}
{"type": "need_verdict", "step": 0, "state_root": "hex", "lease_id": "uuid", "chain_id": "uuid"}
{"type": "step_result", "step": 0, "verdict": "SEAL", "deltas": {"ch_out": "..."}}
{"type": "divergence", "step": 0, "expected": "hex", "actual": "hex", "nodes": ["geo", "wealth"]}
{"type": "cooling", "total_steps": 5, "final_root": "hex", "leases_closed": 1}
{"type": "error", "code": "LEASE_EXPIRED", "message": "..."}
```

### 2.3 Adapter → arifOS (MCP)

```python
# Python side — calling real arifOS MCP
import json, subprocess, time, uuid

def call_arif_judge(state_root: str, lease_id: str, chain_id: str) -> dict:
    """Call arifOS 888-JUDGE via MCP."""
    import requests
    resp = requests.post(
        "http://localhost:8088/mcp",
        json={
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "arif_judge",
                "arguments": {
                    "mode": "intercept",
                    "session_token": lease_id,
                    "evidence": [{"state_root": state_root}],
                    "intent": f"arifFlow super-step verdict for chain {chain_id}",
                }
            },
            "id": str(uuid.uuid4())
        },
        timeout=10
    )
    result = resp.json()
    verdict_class = result.get("result", {}).get("content", [{}])[0].get("text", "{}")
    verdict_data = json.loads(verdict_class)
    return verdict_data  # {verdict: "SEAL"|"HOLD"|"VOID", verdict_id: "...", ...}
```

---

## 3. Core Functions

### 3.1 `ArifFlowAdapter`

```python
class ArifFlowAdapter:
    """
    Supervises a Rust arifFlow subprocess.
    Manages stdin/stdout JSON-L protocol.
    Routes verdict calls to arifOS.
    Routes node execution to A-FORGE.
    """
    
    def __init__(self, binary_path: str = "/root/arifFlow/target/release/ariflow"):
        self.process: subprocess.Popen | None = None
        self.lease_id: str | None = None
        self.chain_id: str | None = None
        self.current_topology: str | None = None
        self.checkpoints: list[dict] = []
        self.pending_verdicts: dict[int, dict] = {}  # step_index -> checkpoint
    
    def spawn(self, topology: str, actor_id: str) -> str:
        """Spawn Rust process. Returns lease_id."""
        self.lease_id = str(uuid.uuid4())
        self.chain_id = str(uuid.uuid4())
        self.current_topology = topology
        
        self.process = subprocess.Popen(
            [binary_path],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        # Send configuration
        self._send({
            "type": "configure",
            "topology": topology,
            "lease_id": self.lease_id,
            "actor_id": actor_id,
            "chain_id": self.chain_id
        })
        
        return self.lease_id
    
    def seed_channel(self, channel: str, data: str):
        """Seed initial data into a channel."""
        self._send({
            "type": "seed",
            "channel": channel,
            "data": data
        })
    
    def run_step(self, nodes: list[dict]):
        """
        Execute one super-step.
        1. Send nodes to Rust (stdin)
        2. Receive 'need_verdict' from Rust (stdout)
        3. Call arifOS 888-JUDGE
        4. Send verdict back to Rust (stdin)
        5. Receive 'step_result' from Rust (stdout)
        6. Emit Kabarkan event
        7. If SEAL: write VAULT999 micro-receipt
        """
        # Step 1: dispatch to Rust
        self._send({"type": "step", "nodes": nodes})
        
        # Step 2: wait for need_verdict
        msg = self._recv()
        if msg.get("type") != "need_verdict":
            raise RuntimeError(f"Expected need_verdict, got: {msg.get('type')}")
        
        # Step 3: call arifOS
        verdict = self._call_arif_judge(msg["state_root"])
        
        # Step 4: send verdict to Rust
        self._send({
            "type": "verdict",
            "class": verdict["verdict"],
            "verdict_id": verdict["verdict_id"],
            "hash": verdict.get("hash", "0" * 64)
        })
        
        # Step 5: receive result
        result = self._recv()
        
        # Step 6: Kabarkan
        self._emit_kabarkan("super_step_completed", {
            "step": result.get("step"),
            "verdict": verdict["verdict"],
            "state_root": result.get("state_root")
        })
        
        # Step 7: VAULT999 micro-seal (if SEAL)
        if verdict["verdict"] == "SEAL":
            self._vault_seal(result)
        
        return result
    
    def close(self):
        """Send stop, wait for cooling receipt, close process."""
        self._send({"type": "stop"})
        cooling = self._recv()
        self.process.wait(timeout=5)
        return cooling
    
    def restore_from_checkpoint(self, checkpoint: dict) -> bool:
        """
        Crash recovery path.
        1. Re-verify authority: call arifOS validate_checkpoint(chain_id, verdict_id)
        2. If valid: re-spawn Rust, replay checkpoints
        3. If invalid: emit breach, refuse to resume
        """
        validation = self._call_validate_checkpoint(
            checkpoint["chain_id"],
            checkpoint.get("verdict_id", "")
        )
        if not validation.get("allowed", False):
            self._emit_kabarkan("breach", {
                "reason": "checkpoint_invalid",
                "chain_id": checkpoint["chain_id"]
            })
            return False
        
        # Re-spawn and replay
        self.spawn(self.current_topology, checkpoint.get("actor_id", "unknown"))
        for cp in self.checkpoints:
            self._send({"type": "restore", "checkpoint": cp})
        return True
    
    # --- Internal ---
    
    def _send(self, msg: dict):
        """Write JSON-L to Rust stdin."""
        line = json.dumps(msg) + "\n"
        self.process.stdin.write(line)
        self.process.stdin.flush()
    
    def _recv(self, timeout: float = 10.0) -> dict:
        """Read JSON-L from Rust stdout with timeout."""
        import select
        if select.select([self.process.stdout], [], [], timeout)[0]:
            line = self.process.stdout.readline()
            return json.loads(line)
        raise TimeoutError(f"No response from arifFlow after {timeout}s")
    
    def _call_arif_judge(self, state_root: str) -> dict:
        """Call arifOS 888-JUDGE. Retry with backoff if timeout."""
        max_retries = 3
        for attempt in range(max_retries):
            try:
                return call_arif_judge(state_root, self.lease_id, self.chain_id)
            except (requests.Timeout, ConnectionError) as e:
                if attempt == max_retries - 1:
                    return {"verdict": "HOLD", "verdict_id": "", "hash": "0"*64}
                time.sleep(2 ** attempt)  # backoff
    
    def _call_validate_checkpoint(self, chain_id: str, verdict_id: str) -> dict:
        """Call arifOS checkpoint validation."""
        # TODO: wire to real arif_judge validate mode
        return {"allowed": True}
    
    def _vault_seal(self, result: dict):
        """Write per-step envelope to VAULT999."""
        # TODO: wire to VAULT999 Python module
        pass
    
    def _emit_kabarkan(self, event_type: str, data: dict):
        """Emit trace event to Kabarkan."""
        # TODO: wire to Kabarkan HTTP endpoint
        pass
```

---

## 4. Entry Point

```python
#!/usr/bin/env python3
"""arifFlow adapter — bridge between Rust core and arifOS federation."""

import argparse, sys
from ariflow_adapter import ArifFlowAdapter

def main():
    parser = argparse.ArgumentParser(description="arifFlow adapter")
    parser.add_argument("--topology", required=True, choices=["fan_out", "pipeline", "cascade"])
    parser.add_argument("--actor", default="333-AGI")
    parser.add_argument("--binary", default="/root/arifFlow/target/release/ariflow")
    parser.add_argument("--seed", type=json.loads, default=None, help='{"channel":"data"}')
    args = parser.parse_args()
    
    adapter = ArifFlowAdapter(binary_path=args.binary)
    lease_id = adapter.spawn(args.topology, args.actor)
    print(f"Lease: {lease_id}", file=sys.stderr)
    
    if args.seed:
        for ch, data in args.seed.items():
            adapter.seed_channel(ch, data)
    
    try:
        while True:
            # Read node definitions from stdin
            line = sys.stdin.readline()
            if not line:
                break
            nodes = json.loads(line)
            result = adapter.run_step(nodes)
            print(json.dumps(result), flush=True)
    finally:
        cooling = adapter.close()
        print(json.dumps({"cooling": cooling}), file=sys.stderr)

if __name__ == "__main__":
    main()
```

---

## 5. F13 Compliance Checklist

| Check | Implementation | Status |
|-------|---------------|--------|
| 888-JUDGE per super-step | `_call_arif_judge()` before every commit | ✅ Spec'd |
| HOLD discards deltas | Rust core already does this | ✅ Phase 1 |
| Lease required for all execution | `spawn()` generates lease, Rust rejects nil | ✅ Phase 1 |
| Verdict timeout → safe HOLD | `_call_arif_judge()` retry with backoff → HOLD fallback | ✅ Spec'd |
| Crash recovery re-verifies authority | `restore_from_checkpoint()` validates via arifOS before resume | ✅ Spec'd |
| Cooling receipt on every run | `close()` receives cooling envelope | ✅ Spec'd |
| No orphaned channels | Rust `Channel::close()` on VOID | ✅ Phase 1 |
| Kabarkan trace per super-step | `_emit_kabarkan()` in `run_step()` | ✅ Spec'd |
| VAULT999 per-step micro-seal | `_vault_seal()` in `run_step()` | ✅ Spec'd |

---

## 6. Open Questions (for Arif to decide)

| Question | Options | Impact |
|----------|---------|--------|
| Rust binary in release or debug? | Release = faster, debug = better error messages | Release for prod, debug for dev |
| JSON-L or protobuf for Rust↔Python? | JSON-L = easy debug, protobuf = faster | JSON-L for Phase 2, upgrade if bottleneck |
| Blocking or async Python adapter? | Blocking = simpler, async = better concurrency | Blocking for Phase 2, async Phase 3 |
| Adapter lives where? | `A-FORGE/domain/orchestration/` or `arifFlow/adapter/` | A-FORGE dir — makes more sense |

---

*DITEMPA BUKAN DIBERI — Phase 2 target. Belum production. 888-HOLD pada deploy.*
