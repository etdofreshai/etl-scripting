#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="/opt/data/workspace/etl-scripting"
WORKER_PID_FILE="$ROOT_DIR/.worker/worker.pid"
WATCHDOG_PID_FILE="$ROOT_DIR/.worker/watchdog.pid"

stop_pid_file() {
  local label="$1"
  local path="$2"
  if [[ -f "$path" ]]; then
    local pid
    pid="$(cat "$path" 2>/dev/null || true)"
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" || true
      echo "stopped $label pid $pid"
      return 0
    fi
  fi
  echo "$label is not running"
}

stop_pid_file watchdog "$WATCHDOG_PID_FILE"
stop_pid_file worker "$WORKER_PID_FILE"
