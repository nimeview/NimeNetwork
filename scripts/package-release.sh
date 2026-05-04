#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR_DEFAULT="$ROOT_DIR/dist"
VERSION_DEFAULT="0.1.0"

log() {
  printf '[package-release] %s\n' "$*"
}

fail() {
  printf '[package-release] error: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

usage() {
  cat <<'EOF'
Usage:
  scripts/package-release.sh [--dist-dir PATH] [--version VERSION] [--skip-build]

What it does:
  - builds release binaries
  - builds vendored innernet binaries
  - creates client and server release directories
  - packs them into .tar.gz archives

Output:
  dist/nime-client-<version>-<host>.tar.gz
  dist/nime-server-<version>-<host>.tar.gz
EOF
}

host_triple() {
  rustc -vV | awk '/^host:/ {print $2}'
}

copy_file() {
  local source="$1"
  local destination="$2"
  [[ -f "$source" ]] || fail "file not found: $source"
  cp "$source" "$destination"
}

write_client_install_script() {
  local path="$1"
  cat >"$path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"

mkdir -p "$BIN_DIR"
cp nime-client "$BIN_DIR/nime-client"
cp innernet "$BIN_DIR/innernet"

printf 'Installed binaries into %s\n' "$BIN_DIR"
printf 'Next: run nime-client init ... and then nime-client setup --invite ...\n'
EOF
  chmod +x "$path"
}

write_server_install_script() {
  local path="$1"
  cat >"$path" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"

mkdir -p "$BIN_DIR"
cp nime-dns "$BIN_DIR/nime-dns"
cp nime-portal "$BIN_DIR/nime-portal"
cp nime-registry "$BIN_DIR/nime-registry"
cp innernet-server "$BIN_DIR/innernet-server"

printf 'Installed binaries into %s\n' "$BIN_DIR"
printf 'Next: use scripts/nime-server-linux.sh install-units ... from the repo or copy unit files manually.\n'
EOF
  chmod +x "$path"
}

main() {
  need_cmd cargo
  need_cmd rustc
  need_cmd tar

  local dist_dir="$DIST_DIR_DEFAULT"
  local version="$VERSION_DEFAULT"
  local skip_build="false"

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dist-dir)
        dist_dir="${2:-}"
        shift 2
        ;;
      --version)
        version="${2:-}"
        shift 2
        ;;
      --skip-build)
        skip_build="true"
        shift
        ;;
      --help|-h)
        usage
        exit 0
        ;;
      *)
        fail "unknown option: $1"
        ;;
    esac
  done

  local host
  host="$(host_triple)"
  local client_dir="$dist_dir/nime-client-$version-$host"
  local server_dir="$dist_dir/nime-server-$version-$host"

  rm -rf "$client_dir" "$server_dir"
  mkdir -p "$client_dir" "$server_dir"

  if [[ "$skip_build" != "true" ]]; then
    log "building release workspace"
    (cd "$ROOT_DIR" && cargo build --release)
    log "building vendored innernet release binaries"
    (cd "$ROOT_DIR/server/innernet" && cargo build --release --bin innernet --bin innernet-server)
  fi

  log "assembling client package"
  copy_file "$ROOT_DIR/target/release/nime-client" "$client_dir/"
  copy_file "$ROOT_DIR/server/innernet/target/release/innernet" "$client_dir/"
  copy_file "$ROOT_DIR/docs/USER_GUIDE.md" "$client_dir/"
  copy_file "$ROOT_DIR/docs/QUICKSTART.md" "$client_dir/"
  printf '%s\n' "$version" >"$client_dir/VERSION"
  printf '%s\n' "$host" >"$client_dir/HOST"
  write_client_install_script "$client_dir/install-client.sh"

  log "assembling server package"
  copy_file "$ROOT_DIR/target/release/nime-dns" "$server_dir/"
  copy_file "$ROOT_DIR/target/release/nime-portal" "$server_dir/"
  copy_file "$ROOT_DIR/target/release/nime-registry" "$server_dir/"
  copy_file "$ROOT_DIR/server/innernet/target/release/innernet-server" "$server_dir/"
  copy_file "$ROOT_DIR/scripts/nime-server-linux.sh" "$server_dir/"
  copy_file "$ROOT_DIR/docs/ADMIN_GUIDE.md" "$server_dir/"
  copy_file "$ROOT_DIR/RUNBOOK.md" "$server_dir/"
  printf '%s\n' "$version" >"$server_dir/VERSION"
  printf '%s\n' "$host" >"$server_dir/HOST"
  write_server_install_script "$server_dir/install-server.sh"

  log "packing tar.gz archives"
  (cd "$dist_dir" && tar -czf "$(basename "$client_dir").tar.gz" "$(basename "$client_dir")")
  (cd "$dist_dir" && tar -czf "$(basename "$server_dir").tar.gz" "$(basename "$server_dir")")

  log "created:"
  log "  $client_dir.tar.gz"
  log "  $server_dir.tar.gz"
}

main "$@"
