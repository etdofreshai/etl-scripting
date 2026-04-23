#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="/opt/data/workspace/etl-scripting"
STATE_DIR="$ROOT_DIR/.worker"
WORKER_PID_FILE="$STATE_DIR/worker.pid"
WATCHDOG_PID_FILE="$STATE_DIR/watchdog.pid"
SESSION_FILE="$STATE_DIR/session_id"
LOG_FILE="$STATE_DIR/logs/worker.log"
WATCHDOG_LOG_FILE="$STATE_DIR/logs/watchdog.log"
HEARTBEAT_FILE="$STATE_DIR/heartbeat.txt"
SUMMARY_FILE="$STATE_DIR/last_summary.txt"
STATUS_FILE="$STATE_DIR/last_status.env"
WATCHDOG_STATUS_FILE="$STATE_DIR/watchdog_status.env"

print_proc_status() {
  local label="$1"
  local path="$2"
  if [[ -f "$path" ]]; then
    local pid
    pid="$(cat "$path" 2>/dev/null || true)"
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      echo "$label=running pid=$pid"
      return
    fi
    echo "$label=stale-pid"
    return
  fi
  echo "$label=stopped"
}

print_proc_status watchdog "$WATCHDOG_PID_FILE"
print_proc_status worker "$WORKER_PID_FILE"

if [[ -f "$SESSION_FILE" ]]; then
  echo "session_id=$(cat "$SESSION_FILE")"
fi

if [[ -f "$HEARTBEAT_FILE" ]]; then
  heartbeat_value="$(cat "$HEARTBEAT_FILE")"
  heartbeat_age="$(( $(date +%s) - $(date -d "$heartbeat_value" +%s) ))"
  echo "heartbeat=$heartbeat_value"
  echo "heartbeat_age_seconds=$heartbeat_age"
fi

if [[ -f "$WATCHDOG_STATUS_FILE" ]]; then
  echo "--- watchdog status ---"
  cat "$WATCHDOG_STATUS_FILE"
fi

if [[ -f "$STATUS_FILE" ]]; then
  echo "--- last status ---"
  cat "$STATUS_FILE"
fi

if [[ -f "$SUMMARY_FILE" ]]; then
  echo "--- last summary ---"
  cat "$SUMMARY_FILE"
fi

if [[ -f "$LOG_FILE" ]]; then
  echo "--- worker log tail ---"
  tail -n 20 "$LOG_FILE"
fi

if [[ -f "$WATCHDOG_LOG_FILE" ]]; then
  echo "--- watchdog log tail ---"
  tail -n 20 "$WATCHDOG_LOG_FILE"
fi

printf '\n--- git status ---%s' "\n"
cd "$ROOT_DIR"
git status --short --branch
printf '\n--- latest commit ---%s' "\n"
git log -1 --oneline
