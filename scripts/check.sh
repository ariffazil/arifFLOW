#!/usr/bin/env bash
# arifFlow Invariant Gate — pre-execution check
# Usage: source check.sh && arifflow_check "my-actor-id"
#        or:  bash check.sh my-actor-id
#
# Returns 0 if allowed, 1 if blocked (HOLD/THROTTLE/VOID).
# Agents MUST call this before any mutation action.
#
# DITEMPA BUKAN DIBERI

ARIFLOW_URL="${ARIFLOW_URL:-http://127.0.0.1:7073}"

arifflow_check() {
    local actor_id="${1:-333-AGI}"
    local result
    result=$(curl -sf -X POST "${ARIFLOW_URL}/check" \
        -H 'Content-Type: application/json' \
        -d "{\"actor_id\":\"${actor_id}\"}" 2>/dev/null)

    if [ $? -ne 0 ]; then
        echo "[arifFlow] CHECK FAILED: cannot reach ${ARIFLOW_URL}/check"
        return 2  # network error — proceed with caution
    fi

    local allowed
    allowed=$(echo "$result" | python3 -c "import json,sys; print(json.load(sys.stdin).get('allowed',False))" 2>/dev/null)

    if [ "$allowed" = "True" ]; then
        echo "[arifFlow] CHECK PASSED: ${actor_id} allowed"
        return 0
    else
        local reason
        reason=$(echo "$result" | python3 -c "import json,sys; print(json.load(sys.stdin).get('reason','unknown'))" 2>/dev/null)
        local action
        action=$(echo "$result" | python3 -c "import json,sys; print(json.load(sys.stdin).get('action','unknown'))" 2>/dev/null)
        echo "[arifFlow] 🔴 HOLD: ${actor_id} blocked — ${action}: ${reason}"
        return 1
    fi
}

arifflow_release() {
    local actor_id="${1:-333-AGI}"
    curl -sf -X POST "${ARIFLOW_URL}/release" \
        -H 'Content-Type: application/json' \
        -d "{\"actor_id\":\"${actor_id}\"}" > /dev/null 2>&1
    echo "[arifFlow] RELEASED: ${actor_id}"
}

# If called directly (not sourced), run check
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
    arifflow_check "${1:-333-AGI}"
fi