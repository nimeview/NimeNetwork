#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${ROOT_DIR}/services/logs"
PID_DIR="${ROOT_DIR}/services/pids"

mkdir -p "$LOG_DIR" "$PID_DIR" \
  "$ROOT_DIR/services/data/wiki" \
  "$ROOT_DIR/services/data/storage" \
  "$ROOT_DIR/services/data/ledger"

usage() {
  cat <<'EOF'
Usage:
  services/run-all.sh start
  services/run-all.sh stop
  services/run-all.sh restart
  services/run-all.sh status

Ports by default:
  wiki    -> 0.0.0.0:8181
  storage -> 0.0.0.0:8282
  ledger  -> 0.0.0.0:8383
EOF
}

start_one() {
  local name="$1"
  local command="$2"
  local pid_file="${PID_DIR}/${name}.pid"
  local log_file="${LOG_DIR}/${name}.log"

  if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "[services] ${name} already running"
    return
  fi

  (
    cd "$ROOT_DIR"
    nohup bash -lc "$command" >"$log_file" 2>&1 &
    echo $! >"$pid_file"
  )
  echo "[services] started ${name}"
}

stop_one() {
  local name="$1"
  local pid_file="${PID_DIR}/${name}.pid"

  if [[ ! -f "$pid_file" ]]; then
    echo "[services] ${name} is not running"
    return
  fi

  local pid
  pid="$(cat "$pid_file")"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid"
    echo "[services] stopped ${name}"
  else
    echo "[services] ${name} pid file was stale"
  fi
  rm -f "$pid_file"
}

status_one() {
  local name="$1"
  local pid_file="${PID_DIR}/${name}.pid"
  if [[ -f "$pid_file" ]] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
    echo "[services] ${name}: running (pid $(cat "$pid_file"))"
  else
    echo "[services] ${name}: stopped"
  fi
}

case "${1:-help}" in
  start)
    start_one "wiki" "cargo run -p nime-wiki -- --bind 0.0.0.0:8181 --data-dir services/data/wiki"
    start_one "storage" "cargo run -p nime-storage -- --bind 0.0.0.0:8282 --data-dir services/data/storage"
    start_one "ledger" "cargo run -p nime-ledger -- --bind 0.0.0.0:8383 --state-file services/data/ledger/state.json"
    ;;
  stop)
    stop_one "wiki"
    stop_one "storage"
    stop_one "ledger"
    ;;
  restart)
    "$0" stop
    "$0" start
    ;;
  status)
    status_one "wiki"
    status_one "storage"
    status_one "ledger"
    ;;
  help|--help|-h)
    usage
    ;;
  *)
    usage
    exit 1
    ;;
esac
