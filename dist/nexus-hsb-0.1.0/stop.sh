#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT_DIR"

LOG_DIR="${HSB_LOG_DIR:-logs}"
PID_FILE="${HSB_PID_FILE:-$LOG_DIR/hsb-server.pid}"

if [[ -f "$PID_FILE" ]]; then
  pid="$(cat "$PID_FILE")"
  if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
    echo "stopping hsb-server pid=$pid"
    kill "$pid"
    for _ in {1..30}; do
      if ! kill -0 "$pid" 2>/dev/null; then
        break
      fi
      sleep 1
    done
    if kill -0 "$pid" 2>/dev/null; then
      echo "hsb-server did not stop in time; sending SIGKILL"
      kill -9 "$pid" 2>/dev/null || true
    fi
  fi
  rm -f "$PID_FILE"
else
  pkill -f 'target/debug/hsb-server start|target/release/hsb-server start' 2>/dev/null || true
fi

if [[ "${STOP_HSB_RESOURCES:-0}" == "1" ]] && command -v docker >/dev/null 2>&1; then
  docker stop hsb-redpanda hsb-rabbitmq hsb-nats >/dev/null 2>&1 || true
fi

echo "hsb-server stopped"