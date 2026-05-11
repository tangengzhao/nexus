#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

CONFIG_PATH="${HSB_CONFIG:-config/hsb.toml}"
LOG_DIR="${HSB_LOG_DIR:-logs}"
PID_FILE="${HSB_PID_FILE:-$LOG_DIR/hsb-server.pid}"
LOG_FILE="${HSB_LOG_FILE:-$LOG_DIR/hsb-server.log}"
ROUTE_PREFIX="${HSB_ROUTE_PREFIX:-}"
ROUTE_PREFIX="/${ROUTE_PREFIX#/}"
ROUTE_PREFIX="${ROUTE_PREFIX%/}"
if [[ "$ROUTE_PREFIX" == "/" ]]; then
  ROUTE_PREFIX=""
fi

mkdir -p "$LOG_DIR" data

if [[ -f "$PID_FILE" ]]; then
  old_pid="$(cat "$PID_FILE")"
  if [[ -n "$old_pid" ]] && kill -0 "$old_pid" 2>/dev/null; then
    echo "hsb-server is already running: pid=$old_pid"
    exit 0
  fi
  rm -f "$PID_FILE"
fi

start_container() {
  local name="$1"
  shift
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker not found; skip container $name"
    return 0
  fi
  if docker ps -a --format '{{.Names}}' | grep -qx "$name"; then
    docker start "$name" >/dev/null
  else
    docker run -d --name "$name" "$@" >/dev/null
  fi
}

start_container hsb-nats -p 4222:4222 nats:2.10-alpine -js
start_container hsb-rabbitmq -p 5672:5672 rabbitmq:3.13-management-alpine
if [[ "${HSB_KAFKA_ENABLED:-false}" == "true" ]]; then
  start_container hsb-redpanda -p 9092:9092 docker.redpanda.com/redpandadata/redpanda:v24.3.6 \
    redpanda start --overprovisioned --smp 1 --memory 512M --reserve-memory 0M \
    --node-id 0 --check=false --kafka-addr 0.0.0.0:9092 --advertise-kafka-addr 127.0.0.1:9092

  if command -v docker >/dev/null 2>&1; then
    docker exec hsb-redpanda rpk topic create hsb.events.v1 >/dev/null 2>&1 || true
  fi
fi

echo "starting hsb-server with $CONFIG_PATH"
nohup cargo run -- start --config "$CONFIG_PATH" >>"$LOG_FILE" 2>&1 &
echo "$!" >"$PID_FILE"
echo "hsb-server pid=$(cat "$PID_FILE")"
echo "log: $LOG_FILE"
echo "ui: http://127.0.0.1:8080${ROUTE_PREFIX}/ui/"