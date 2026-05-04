#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_DB_DEFAULT="${ROOT_DIR}/server/registry/nime-registry.db"

usage() {
  cat <<'EOF'
Usage:
  services/register-defaults.sh --server-ip 10.42.0.1 [--db PATH] [--owner nime]

Registers:
  wiki.docs     -> :8181
  drive.apps    -> :8282
  ledger.apps   -> :8383
EOF
}

SERVER_IP=""
REGISTRY_DB="$REGISTRY_DB_DEFAULT"
OWNER="nime"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --server-ip)
      SERVER_IP="${2:-}"
      shift 2
      ;;
    --db)
      REGISTRY_DB="${2:-}"
      shift 2
      ;;
    --owner)
      OWNER="${2:-}"
      shift 2
      ;;
    help|--help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "$SERVER_IP" ]]; then
  echo "--server-ip is required" >&2
  usage
  exit 1
fi

cd "$ROOT_DIR"

cargo run -p nime-registry -- add --db "$REGISTRY_DB" --name wiki --group docs --target-ip "$SERVER_IP" --port 8181 --proto http --title "Wiki" --description "Общая внутренняя вики сети" --owner "$OWNER" --tags docs,shared || true
cargo run -p nime-registry -- add --db "$REGISTRY_DB" --name drive --group apps --target-ip "$SERVER_IP" --port 8282 --proto http --title "Drive" --description "Общее файловое хранилище сети" --owner "$OWNER" --tags files,shared || true
cargo run -p nime-registry -- add --db "$REGISTRY_DB" --name ledger --group apps --target-ip "$SERVER_IP" --port 8383 --proto http --title "Ledger" --description "Игровая внутренняя валюта сети без внешней стоимости" --owner "$OWNER" --tags economy,shared || true

echo "registered default services in ${REGISTRY_DB}"
