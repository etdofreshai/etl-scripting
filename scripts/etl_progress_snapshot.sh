#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="/opt/data/workspace/etl-scripting"
STATE_DIR="$ROOT_DIR/.worker"
SUMMARY_FILE="$STATE_DIR/last_summary.txt"
STATUS_FILE="$STATE_DIR/last_status.env"
WATCHDOG_STATUS_FILE="$STATE_DIR/watchdog_status.env"
SESSION_FILE="$STATE_DIR/session_id"

printf 'ETL continuous worker progress\n'
printf 'repo=%s\n' "$ROOT_DIR"
printf 'generated_at=%s\n' "$(date --iso-8601=seconds)"

if [[ -f "$SESSION_FILE" ]]; then
  printf 'session_id=%s\n' "$(cat "$SESSION_FILE")"
fi

if [[ -f "$WATCHDOG_STATUS_FILE" ]]; then
  printf '\n[watchdog]\n'
  cat "$WATCHDOG_STATUS_FILE"
fi

if [[ -f "$STATUS_FILE" ]]; then
  printf '\n[last_status]\n'
  cat "$STATUS_FILE"
fi

if [[ -f "$SUMMARY_FILE" ]]; then
  printf '\n[last_summary]\n'
  cat "$SUMMARY_FILE"
fi
