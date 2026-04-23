#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="/opt/data/workspace/etl-scripting"
exec "$ROOT_DIR/scripts/etl_continuous_watchdog.sh"
