#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REGISTRY_DB_DEFAULT="$ROOT_DIR/server/registry/nime-registry.db"
ZONE_DEFAULT="nime.internal"
DNS_BIND_DEFAULT="0.0.0.0:5354"
PORTAL_BIND_DEFAULT="0.0.0.0:8080"
REGISTRY_API_BIND_DEFAULT="0.0.0.0:4172"

log() {
  printf '[nime-server] %s\n' "$*"
}

fail() {
  printf '[nime-server] error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

usage() {
  cat <<'EOF'
Usage:
  scripts/nime-server-linux.sh install-deps
  scripts/nime-server-linux.sh build
  scripts/nime-server-linux.sh backup-state --innernet-db PATH [--registry-db PATH] [--output-dir PATH]
  scripts/nime-server-linux.sh restore-state --backup-dir PATH --innernet-db PATH [--registry-db PATH]
  scripts/nime-server-linux.sh bootstrap-registry --portal-ip IP [--portal-port PORT] [--owner OWNER] [--db PATH]
  scripts/nime-server-linux.sh run-dns --innernet-db PATH [--registry-api HOST:PORT] [--zone ZONE] [--bind HOST:PORT] [--reload-secs N]
  scripts/nime-server-linux.sh run-portal [--registry-api HOST:PORT] [--zone ZONE] [--bind HOST:PORT]
  scripts/nime-server-linux.sh run-registry-api [--db PATH] [--bind HOST:PORT]
  scripts/nime-server-linux.sh write-units --innernet-db PATH [--registry-db PATH] [--zone ZONE] [--dns-bind HOST:PORT] [--portal-bind HOST:PORT] [--registry-api-bind HOST:PORT] [--output-dir PATH]
  scripts/nime-server-linux.sh install-units --innernet-db PATH [--registry-db PATH] [--zone ZONE] [--dns-bind HOST:PORT] [--portal-bind HOST:PORT] [--registry-api-bind HOST:PORT] [--output-dir PATH]
  scripts/nime-server-linux.sh help

Examples:
  scripts/nime-server-linux.sh install-deps
  scripts/nime-server-linux.sh build
  scripts/nime-server-linux.sh backup-state --innernet-db /var/lib/innernet-server/nimenet.db
  scripts/nime-server-linux.sh restore-state --backup-dir backups/20260412-120000 --innernet-db /var/lib/innernet-server/nimenet.db
  scripts/nime-server-linux.sh bootstrap-registry --portal-ip 10.42.0.1 --owner arthur
  scripts/nime-server-linux.sh run-dns --innernet-db /var/lib/innernet-server/nime.db
  scripts/nime-server-linux.sh run-portal
  scripts/nime-server-linux.sh run-registry-api --bind 0.0.0.0:4172
  scripts/nime-server-linux.sh write-units --innernet-db /var/lib/innernet-server/nimenet.db
  scripts/nime-server-linux.sh install-units --innernet-db /var/lib/innernet-server/nimenet.db
EOF
}

detect_pkg_manager() {
  if command -v apt-get >/dev/null 2>&1; then
    echo apt
  elif command -v dnf >/dev/null 2>&1; then
    echo dnf
  elif command -v pacman >/dev/null 2>&1; then
    echo pacman
  else
    fail "unsupported package manager; install dependencies manually"
  fi
}

install_deps() {
  local manager
  manager="$(detect_pkg_manager)"
  log "detected package manager: $manager"

  case "$manager" in
    apt)
      sudo apt-get update
      sudo apt-get install -y \
        build-essential \
        pkg-config \
        clang \
        libclang-dev \
        libsqlite3-dev \
        sqlite3 \
        curl \
        ca-certificates \
        wireguard-tools
      ;;
    dnf)
      sudo dnf install -y \
        gcc \
        gcc-c++ \
        make \
        pkgconf-pkg-config \
        clang \
        clang-devel \
        sqlite-devel \
        sqlite \
        curl \
        ca-certificates \
        wireguard-tools
      ;;
    pacman)
      sudo pacman -Sy --noconfirm \
        base-devel \
        pkgconf \
        clang \
        sqlite \
        curl \
        ca-certificates \
        wireguard-tools
      ;;
  esac

  log "dependency installation complete"
  log "if Rust is missing, install it with: curl https://sh.rustup.rs -sSf | sh"
}

build_all() {
  need_cmd cargo
  log "building Nime workspace"
  (cd "$ROOT_DIR" && cargo build --release)
  log "building innernet-server from vendored source"
  (cd "$ROOT_DIR/server/innernet" && cargo build --release --bin innernet-server --bin innernet)
  log "build complete"
  log "nime binaries: $ROOT_DIR/target/release/"
  log "innernet binaries: $ROOT_DIR/server/innernet/target/release/"
}

backup_state() {
  local innernet_db=""
  local registry_db="$REGISTRY_DB_DEFAULT"
  local output_dir="${ROOT_DIR}/backups/$(date +%Y%m%d-%H%M%S)"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --innernet-db)
        innernet_db="${2:-}"
        shift 2
        ;;
      --registry-db)
        registry_db="${2:-}"
        shift 2
        ;;
      --output-dir)
        output_dir="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown backup-state option: $1"
        ;;
    esac
  done

  [[ -n "$innernet_db" ]] || fail "--innernet-db is required"
  [[ -f "$innernet_db" ]] || fail "innernet db not found: $innernet_db"
  [[ -f "$registry_db" ]] || fail "registry db not found: $registry_db"

  mkdir -p "$output_dir"
  cp "$innernet_db" "$output_dir/innernet.db"
  cp "$registry_db" "$output_dir/registry.db"
  log "backup created in $output_dir"
  log "files: innernet.db, registry.db"
}

restore_state() {
  local backup_dir=""
  local innernet_db=""
  local registry_db="$REGISTRY_DB_DEFAULT"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --backup-dir)
        backup_dir="${2:-}"
        shift 2
        ;;
      --innernet-db)
        innernet_db="${2:-}"
        shift 2
        ;;
      --registry-db)
        registry_db="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown restore-state option: $1"
        ;;
    esac
  done

  [[ -n "$backup_dir" ]] || fail "--backup-dir is required"
  [[ -n "$innernet_db" ]] || fail "--innernet-db is required"
  [[ -f "$backup_dir/innernet.db" ]] || fail "backup innernet.db not found in $backup_dir"
  [[ -f "$backup_dir/registry.db" ]] || fail "backup registry.db not found in $backup_dir"

  mkdir -p "$(dirname "$innernet_db")"
  mkdir -p "$(dirname "$registry_db")"
  cp "$backup_dir/innernet.db" "$innernet_db"
  cp "$backup_dir/registry.db" "$registry_db"
  log "restored innernet db to $innernet_db"
  log "restored registry db to $registry_db"
}

bootstrap_registry() {
  need_cmd cargo

  local portal_ip=""
  local portal_port="8080"
  local owner="admin"
  local db_path="$REGISTRY_DB_DEFAULT"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --portal-ip)
        portal_ip="${2:-}"
        shift 2
        ;;
      --portal-port)
        portal_port="${2:-}"
        shift 2
        ;;
      --owner)
        owner="${2:-}"
        shift 2
        ;;
      --db)
        db_path="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown bootstrap-registry option: $1"
        ;;
    esac
  done

  [[ -n "$portal_ip" ]] || fail "--portal-ip is required"

  (cd "$ROOT_DIR" && cargo run -p nime-registry -- \
    bootstrap \
    --db "$db_path" \
    --portal-ip "$portal_ip" \
    --portal-port "$portal_port" \
    --owner "$owner")
}

run_dns() {
  need_cmd cargo

  local innernet_db=""
  local registry_api="$REGISTRY_API_BIND_DEFAULT"
  local zone="$ZONE_DEFAULT"
  local bind="$DNS_BIND_DEFAULT"
  local reload_secs="5"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --innernet-db)
        innernet_db="${2:-}"
        shift 2
        ;;
      --registry-api)
        registry_api="${2:-}"
        shift 2
        ;;
      --zone)
        zone="${2:-}"
        shift 2
        ;;
      --bind)
        bind="${2:-}"
        shift 2
        ;;
      --reload-secs)
        reload_secs="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown run-dns option: $1"
        ;;
    esac
  done

  [[ -n "$innernet_db" ]] || fail "--innernet-db is required"

  (cd "$ROOT_DIR" && cargo run -p nime-dns -- \
    --bind "$bind" \
    --zone "$zone" \
    --innernet-db "$innernet_db" \
    --registry-api "$registry_api" \
    --reload-secs "$reload_secs")
}

run_portal() {
  need_cmd cargo

  local registry_api="$REGISTRY_API_BIND_DEFAULT"
  local zone="$ZONE_DEFAULT"
  local bind="$PORTAL_BIND_DEFAULT"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --registry-api)
        registry_api="${2:-}"
        shift 2
        ;;
      --zone)
        zone="${2:-}"
        shift 2
        ;;
      --bind)
        bind="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown run-portal option: $1"
        ;;
    esac
  done

  (cd "$ROOT_DIR" && cargo run -p nime-portal -- \
    --bind "$bind" \
    --registry-api "$registry_api" \
    --dns-zone "$zone")
}

run_registry_api() {
  need_cmd cargo

  local registry_db="$REGISTRY_DB_DEFAULT"
  local bind="$REGISTRY_API_BIND_DEFAULT"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --db|--registry-db)
        registry_db="${2:-}"
        shift 2
        ;;
      --bind)
        bind="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown run-registry-api option: $1"
        ;;
    esac
  done

  (cd "$ROOT_DIR" && cargo run -p nime-registry -- \
    serve \
    --db "$registry_db" \
    --bind "$bind")
}

write_units() {
  local innernet_db=""
  local registry_db="$REGISTRY_DB_DEFAULT"
  local zone="$ZONE_DEFAULT"
  local dns_bind="$DNS_BIND_DEFAULT"
  local portal_bind="$PORTAL_BIND_DEFAULT"
  local registry_api_bind="$REGISTRY_API_BIND_DEFAULT"
  local output_dir="$ROOT_DIR/deploy/systemd"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --innernet-db)
        innernet_db="${2:-}"
        shift 2
        ;;
      --registry-db)
        registry_db="${2:-}"
        shift 2
        ;;
      --zone)
        zone="${2:-}"
        shift 2
        ;;
      --dns-bind)
        dns_bind="${2:-}"
        shift 2
        ;;
      --portal-bind)
        portal_bind="${2:-}"
        shift 2
        ;;
      --registry-api-bind)
        registry_api_bind="${2:-}"
        shift 2
        ;;
      --output-dir)
        output_dir="${2:-}"
        shift 2
        ;;
      *)
        fail "unknown write-units option: $1"
        ;;
    esac
  done

  [[ -n "$innernet_db" ]] || fail "--innernet-db is required"

  mkdir -p "$output_dir"

  cat >"$output_dir/nime-dns.service" <<EOF
[Unit]
Description=Nime DNS
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$ROOT_DIR
ExecStart=$ROOT_DIR/target/release/nime-dns --bind $dns_bind --zone $zone --innernet-db $innernet_db --registry-api $registry_api_bind --reload-secs 5
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

  cat >"$output_dir/nime-portal.service" <<EOF
[Unit]
Description=Nime Portal
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$ROOT_DIR
ExecStart=$ROOT_DIR/target/release/nime-portal --bind $portal_bind --registry-api $registry_api_bind --dns-zone $zone
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

  cat >"$output_dir/nime-registry-api.service" <<EOF
[Unit]
Description=Nime Registry API
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$ROOT_DIR
ExecStart=$ROOT_DIR/target/release/nime-registry serve --db $registry_db --bind $registry_api_bind
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

  log "wrote systemd units to $output_dir"
  log "generated: nime-dns.service, nime-portal.service, nime-registry-api.service"
}

install_units() {
  need_cmd sudo
  need_cmd systemctl

  local args=("$@")
  local output_dir="$ROOT_DIR/deploy/systemd"
  local next_is_output_dir="false"

  for arg in "${args[@]}"; do
    if [[ "$next_is_output_dir" == "true" ]]; then
      output_dir="$arg"
      next_is_output_dir="false"
      continue
    fi
    if [[ "$arg" == "--output-dir" ]]; then
      next_is_output_dir="true"
    fi
  done

  write_units "${args[@]}"

  sudo cp "$output_dir"/nime-dns.service /etc/systemd/system/
  sudo cp "$output_dir"/nime-portal.service /etc/systemd/system/
  sudo cp "$output_dir"/nime-registry-api.service /etc/systemd/system/
  sudo systemctl daemon-reload
  sudo systemctl enable --now nime-dns.service
  sudo systemctl enable --now nime-portal.service
  sudo systemctl enable --now nime-registry-api.service
  log "installed and started nime-dns, nime-portal and nime-registry-api"
}

main() {
  local subcommand="${1:-help}"
  shift || true

  case "$subcommand" in
    install-deps) install_deps "$@" ;;
    build) build_all "$@" ;;
    backup-state) backup_state "$@" ;;
    restore-state) restore_state "$@" ;;
    bootstrap-registry) bootstrap_registry "$@" ;;
    run-dns) run_dns "$@" ;;
    run-portal) run_portal "$@" ;;
    run-registry-api) run_registry_api "$@" ;;
    write-units) write_units "$@" ;;
    install-units) install_units "$@" ;;
    help|-h|--help) usage ;;
    *) fail "unknown subcommand: $subcommand" ;;
  esac
}

main "$@"
