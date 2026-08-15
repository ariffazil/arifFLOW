#!/usr/bin/env bash
# arifFlow deploy script
# DITEMPA BUKAN DIBERI — Forged, Not Given
set -euo pipefail

APP_NAME="arifflow"
SOURCE_DIR="/root/arifFlow"
TARGET_DIR="/opt/arifflow/bin"
SERVICE_NAME="arifflow.service"
SYSTEMD_UNIT="/etc/systemd/system/${SERVICE_NAME}"

echo "=== arifFlow Deploy ==="

# 1. Build release binary
echo "[1/5] Building release binary..."
cd "${SOURCE_DIR}"
cargo build --release

# 2. Stop existing service
echo "[2/5] Stopping existing service..."
systemctl stop "${SERVICE_NAME}" 2>/dev/null || true

# 3. Deploy binary
echo "[3/5] Deploying to ${TARGET_DIR}..."
mkdir -p "${TARGET_DIR}" /var/log/ariflow /tmp/ariflow
cp target/release/arifflow "${TARGET_DIR}/arifflow"
chmod +x "${TARGET_DIR}/arifflow"

# 4. Install systemd unit
echo "[4/5] Checking systemd unit..."
if [ -f "${SYSTEMD_UNIT}" ]; then
  systemctl daemon-reload
else
  cp deploy/ariflow.service "${SYSTEMD_UNIT}"
  systemctl daemon-reload
fi

# 5. Start service
echo "[5/5] Starting service..."
systemctl enable "${SERVICE_NAME}"
systemctl start "${SERVICE_NAME}"

sleep 2
systemctl status "${SERVICE_NAME}" --no-pager

echo "=== Deploy complete ==="
echo "Health: curl http://127.0.0.1:7073/health"

