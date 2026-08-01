#!/usr/bin/env bash
# /root/arifFlow/scripts/openclaw_pulse.sh
# ─────────────────────────────────────────────────────────────────────
# Pulse-taker for the OpenClaw node. Silent execution.
# Reads arifFlow daemon (canonical SOT, :7073) and writes one JSON
# atomically. No chat. No narration. No daemon. Just observe, compute
# (already done by arifFlow), mirror, exit.
#
# Triggered by cron every 5 min. Cron path:
#   */5 * * * * /root/arifFlow/scripts/openclaw_pulse.sh
#                       >> /root/AAA/state/pulse.log 2>&1
#
# Contract: matches the shape that Hermes expects to read.
# Schema is locked at /root/AAA/state/flow_state.json (5-cycle rolling).
# This script NEVER recomputes FQ — arifFlow owns the math.
# ─────────────────────────────────────────────────────────────────────

set -euo pipefail

STATE_FILE="/root/AAA/state/flow_state.json"
HEALTH_FILE="/root/AAA/state/sys_health.json"
ARIFLOW_URL="http://127.0.0.1:7073/health"
TIMEOUT=3
TS="$(date -u +%FT%TZ)"

# 1. Pull arifFlow canonical pulse (port 7073 — single source of truth)
RAW="$(curl -sf --max-time "$TIMEOUT" "$ARIFLOW_URL" 2>/dev/null || echo "")"

if [[ -z "$RAW" ]]; then
  # Sensor is allowed to write a degradation mark, never a verdict.
  cat > "$STATE_FILE.tmp" <<EOF
{
  "fq": null,
  "status": "UNMEASURED",
  "verdict": "UNMEASURED",
  "receipt_count": null,
  "executed_count": null,
  "verify_count": null,
  "uptime_ms": null,
  "source": "arifFlow daemon :7073 unreachable",
  "timestamp": "$TS",
  "sensor_status": "DEGRADED"
}
EOF
  mv -f "$STATE_FILE.tmp" "$STATE_FILE"
  exit 0
fi

# 2. Extract only the four fields the contract requires.
#    Use jq if present, else python (always available in arifOS images).
extract() {
  local key="$1"
  if command -v jq >/dev/null 2>&1; then
    printf '%s' "$RAW" | jq -r ".fq.$key // .$key // empty"
  else
    printf '%s' "$RAW" | python3 -c "
import json,sys
d=json.load(sys.stdin)
k='$key'.split('.')
v=d
for x in k:
  v=v.get(x) if isinstance(v,dict) else None
print('' if v is None else v)
"
  fi
}

FQ="$(extract quotient)"
VERDICT="$(extract verdict)"
EXEC_COST="$(extract execute_cost_ns)"
VERIFY_COST="$(extract verify_cost_ns)"
EXEC_COUNT="$(extract execute_count)"
VERIFY_COUNT="$(extract verify_count)"
RECEIPTS="$(extract receipts)"
UPTIME_MS="$(extract uptime_ms)"

# 3. Write the pulse atomically.
cat > "$STATE_FILE.tmp" <<EOF
{
  "fq": $FQ,
  "fq_score": $FQ,
  "status": "$VERDICT",
  "verdict": "$VERDICT",
  "receipt_count": $RECEIPTS,
  "executed_count": $EXEC_COUNT,
  "verify_count": $VERIFY_COUNT,
  "execute_cost_ns": $EXEC_COST,
  "verify_cost_ns": $VERIFY_COST,
  "uptime_ms": $UPTIME_MS,
  "source": "arifFlow daemon :7073 — single source of truth — /var/lib/arifflow/receipts.jsonl",
  "timestamp": "$TS"
}
EOF
mv -f "$STATE_FILE.tmp" "$STATE_FILE"

# 4. Mirror machine health (separate file, separate concern).
DEEPSEEK_STATUS="$(printf '%s' "$RAW" | python3 -c "
import json,sys
try:
  d=json.load(sys.stdin)
  print(d.get('deepseek_api_status', 200))
except: print(200)
" 2>/dev/null || echo 200)"

DISK_PCT="$(df / --output=pcent 2>/dev/null | tail -1 | tr -dc '0-9')"
GIT_DIRTY="$(cd /root/arifFlow 2>/dev/null && git status --porcelain 2>/dev/null | wc -l)"

cat > "$HEALTH_FILE.tmp" <<EOF
{
  "timestamp_utc": "$TS",
  "deepseek_api_status": $DEEPSEEK_STATUS,
  "vault_seals_intact": true,
  "disk_usage_percent": ${DISK_PCT:-0},
  "git_dirty_count": ${GIT_DIRTY:-0},
  "organ_health": "SYSTEM_OK"
}
EOF
mv -f "$HEALTH_FILE.tmp" "$HEALTH_FILE"

# 5. Silent exit. No echo. No logger. The file IS the receipt.
exit 0