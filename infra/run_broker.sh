#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

LAN_IP="$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo "unknown")"

echo "LESS broker starting..."
echo "LAN IP (use as MQTT_BROKER_HOST): ${LAN_IP}"
echo "  MQTT:      1883"
echo "  WebSocket: 9001"

exec mosquitto -c "${SCRIPT_DIR}/mosquitto.conf"
