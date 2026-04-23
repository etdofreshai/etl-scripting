#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="/opt/data/workspace/etl-scripting"
PROMPT_FILE="$ROOT_DIR/scripts/etl_worker_prompt.txt"
STATE_DIR="$ROOT_DIR/.worker"
LOG_DIR="$STATE_DIR/logs"
PID_FILE="$STATE_DIR/worker.pid"
SESSION_FILE="$STATE_DIR/session_id"
LOG_FILE="$LOG_DIR/worker.log"
HEARTBEAT_FILE="$STATE_DIR/heartbeat.txt"
SUMMARY_FILE="$STATE_DIR/last_summary.txt"
OUTPUT_FILE="$STATE_DIR/last_run.out"
ITERATION_FILE="$STATE_DIR/iteration_count"
STATUS_FILE="$STATE_DIR/last_status.env"
HERMES_BIN="/opt/data/home/.local/bin/hermes"

mkdir -p "$STATE_DIR" "$LOG_DIR"

if [[ -f "$PID_FILE" ]]; then
  existing_pid="$(cat "$PID_FILE" 2>/dev/null || true)"
  if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
    echo "worker already running with pid $existing_pid"
    exit 0
  fi
fi

echo $$ > "$PID_FILE"
trap 'rm -f "$PID_FILE"' EXIT

export PATH="/opt/data/home/.local/bin:$PATH"
if [[ -f /opt/data/.env ]]; then
  set -a
  source /opt/data/.env
  set +a
fi

cd "$ROOT_DIR"

echo "[$(date --iso-8601=seconds)] starting etl continuous worker" | tee -a "$LOG_FILE"

if [[ ! -f "$ITERATION_FILE" ]]; then
  echo 0 > "$ITERATION_FILE"
fi

while true; do
  iteration="$(cat "$ITERATION_FILE")"
  iteration="$((iteration + 1))"
  echo "$iteration" > "$ITERATION_FILE"
  printf 'state=running\niteration=%s\nstarted_at=%s\n' "$iteration" "$(date --iso-8601=seconds)" > "$STATUS_FILE"
  date --iso-8601=seconds > "$HEARTBEAT_FILE"

  prompt="$(cat "$PROMPT_FILE")"
  cmd=("$HERMES_BIN")

  if [[ -f "$SESSION_FILE" ]]; then
    session_id="$(cat "$SESSION_FILE")"
    if [[ -n "$session_id" ]]; then
      cmd+=(--resume "$session_id")
    fi
  fi

  cmd+=(chat -q "$prompt" -Q --yolo -t terminal,file,skills,session_search,todo,search,web)

  {
    printf '\n[%s] iteration %s starting\n' "$(date --iso-8601=seconds)" "$iteration"
    printf 'command:'
    printf ' %q' "${cmd[@]}"
    printf '\n'
  } >> "$LOG_FILE"

  set +e
  timeout 25m "${cmd[@]}" > "$OUTPUT_FILE" 2>&1
  status=$?
  set -e

  cat "$OUTPUT_FILE" >> "$LOG_FILE"
  printf '\n' >> "$LOG_FILE"

  new_session_id="$(grep -Eo 'session_id: [A-Za-z0-9_:-]+' "$OUTPUT_FILE" | awk '{print $2}' | tail -1 || true)"
  if [[ -n "$new_session_id" ]]; then
    echo "$new_session_id" > "$SESSION_FILE"
  fi

  {
    echo "iteration=$iteration"
    echo "finished_at=$(date --iso-8601=seconds)"
    echo "exit_code=$status"
    if [[ -n "${new_session_id:-}" ]]; then
      echo "session_id=$new_session_id"
    elif [[ -f "$SESSION_FILE" ]]; then
      echo "session_id=$(cat "$SESSION_FILE")"
    fi
  } >> "$STATUS_FILE"

  awk 'found{print} /^what changed$/{found=1; print}' "$OUTPUT_FILE" > "$SUMMARY_FILE" || true
  if [[ ! -s "$SUMMARY_FILE" ]]; then
    tail -n 80 "$OUTPUT_FILE" > "$SUMMARY_FILE" || true
  fi

  date --iso-8601=seconds > "$HEARTBEAT_FILE"

  if [[ $status -ne 0 ]]; then
    echo "[$(date --iso-8601=seconds)] worker iteration $iteration exited with status $status" | tee -a "$LOG_FILE"
    sleep 20
  else
    sleep 10
  fi
done
