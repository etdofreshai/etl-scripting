#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="/opt/data/workspace/etl-scripting"
STATE_DIR="$ROOT_DIR/.worker"
LOG_DIR="$STATE_DIR/logs"
WATCHDOG_PID_FILE="$STATE_DIR/watchdog.pid"
WORKER_PID_FILE="$STATE_DIR/worker.pid"
HEARTBEAT_FILE="$STATE_DIR/heartbeat.txt"
WATCHDOG_STATUS_FILE="$STATE_DIR/watchdog_status.env"
RESTART_COUNT_FILE="$STATE_DIR/restart_count"
LOG_FILE="$LOG_DIR/watchdog.log"
WORKER_SCRIPT="$ROOT_DIR/scripts/etl_continuous_worker.sh"
STALE_AFTER_SECONDS=2400
CHECK_INTERVAL_SECONDS=20

mkdir -p "$STATE_DIR" "$LOG_DIR"

if [[ ! -f "$RESTART_COUNT_FILE" ]]; then
  echo 0 > "$RESTART_COUNT_FILE"
fi

if [[ -f "$WATCHDOG_PID_FILE" ]]; then
  existing_pid="$(cat "$WATCHDOG_PID_FILE" 2>/dev/null || true)"
  if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
    echo "watchdog already running with pid $existing_pid"
    exit 0
  fi
fi

echo $$ > "$WATCHDOG_PID_FILE"
trap 'rm -f "$WATCHDOG_PID_FILE"' EXIT

write_status() {
  local worker_state="$1"
  local heartbeat_age="$2"
  local last_reason="$3"
  cat > "$WATCHDOG_STATUS_FILE" <<EOF
last_check_at=$(date --iso-8601=seconds)
watchdog_pid=$$
worker_state=$worker_state
heartbeat_age_seconds=$heartbeat_age
stale_after_seconds=$STALE_AFTER_SECONDS
restart_count=$(cat "$RESTART_COUNT_FILE")
last_restart_reason=$last_reason
EOF
}

start_worker() {
  local reason="$1"
  echo "[$(date --iso-8601=seconds)] starting worker from watchdog (reason: $reason)" | tee -a "$LOG_FILE"
  local restart_count
  restart_count="$(cat "$RESTART_COUNT_FILE")"
  restart_count="$((restart_count + 1))"
  echo "$restart_count" > "$RESTART_COUNT_FILE"
  nohup "$WORKER_SCRIPT" >> "$LOG_FILE" 2>&1 &
  write_status "starting" 0 "$reason"
}

worker_running() {
  if [[ ! -f "$WORKER_PID_FILE" ]]; then
    return 1
  fi
  pid="$(cat "$WORKER_PID_FILE" 2>/dev/null || true)"
  [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

heartbeat_age_seconds() {
  if [[ ! -f "$HEARTBEAT_FILE" ]]; then
    echo 999999
    return
  fi
  local now heartbeat_mtime
  now="$(date +%s)"
  heartbeat_mtime="$(stat -c %Y "$HEARTBEAT_FILE" 2>/dev/null || echo 0)"
  echo "$((now - heartbeat_mtime))"
}

echo "[$(date --iso-8601=seconds)] watchdog online" | tee -a "$LOG_FILE"
write_status "booting" 0 "startup"

while true; do
  if ! worker_running; then
    write_status "missing" 0 "worker_missing"
    start_worker "worker_missing"
    sleep 5
  else
    heartbeat_age="$(heartbeat_age_seconds)"
    if [[ "$heartbeat_age" -gt "$STALE_AFTER_SECONDS" ]]; then
      pid="$(cat "$WORKER_PID_FILE" 2>/dev/null || true)"
      echo "[$(date --iso-8601=seconds)] worker heartbeat stale (${heartbeat_age}s); restarting pid ${pid:-unknown}" | tee -a "$LOG_FILE"
      if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" || true
        sleep 3
        if kill -0 "$pid" 2>/dev/null; then
          kill -9 "$pid" || true
        fi
      fi
      start_worker "heartbeat_stale_${heartbeat_age}s"
      sleep 5
    else
      write_status "healthy" "$heartbeat_age" "none"
    fi
  fi

  sleep "$CHECK_INTERVAL_SECONDS"
done
