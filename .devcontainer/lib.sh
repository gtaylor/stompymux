# Shared helpers sourced by the per-environment setup scripts in .devcontainer/.
# shellcheck shell=bash

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC2034 # Used by the scripts that source this file.
DEVCONTAINER_DIR="$REPOSITORY_ROOT/.devcontainer"

# Tools every environment needs to build, test, and check the project.
REQUIRED_TOOLS=(cargo rustfmt just stylua node npm go hugo)

# Rust toolchain pinned by rust-toolchain.toml at the repository root. The
# devcontainer image is built without the repository, so the version is repeated
# here; check_rust_toolchain fails setup when the two disagree.
RUST_VERSION="1.99.0"

# Tools that improve the editing experience but are not needed by `just checks`.
OPTIONAL_TOOLS=(lua-language-server)

CURL_OPTIONS=(
  --location
  --proto '=https'
  --tlsv1.2
  --fail
  --silent
  --show-error
  --retry 5
  --retry-delay 2
  --retry-all-errors
)

# Prints a progress message.
log() {
  printf '==> %s\n' "$*"
}

# Prints a warning to stderr.
warn() {
  printf 'warning: %s\n' "$*" >&2
}

# Prints an error to stderr and exits.
die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

# Prints each named tool that is not on PATH, one per line.
missing_tools() {
  local tool
  for tool in "$@"; do
    command -v "$tool" >/dev/null 2>&1 || printf '%s\n' "$tool"
  done
}

# Exits with an error naming any required tool that is not on PATH.
require_tools() {
  local missing
  missing="$(missing_tools "${REQUIRED_TOOLS[@]}")"
  if [[ -n "$missing" ]]; then
    die "missing required tools: $(paste -sd ' ' <<<"$missing")"
  fi
  check_rust_toolchain
}

# Exits with an error unless rust-toolchain.toml pins RUST_VERSION and the rustc
# that cargo uses inside the repository is that version.
check_rust_toolchain() {
  local toolchain_file="$REPOSITORY_ROOT/rust-toolchain.toml"
  local pinned
  pinned="$(sed -n 's/^channel = "\(.*\)"$/\1/p' "$toolchain_file")"
  if [[ "$pinned" != "$RUST_VERSION" ]]; then
    die "rust-toolchain.toml pins '$pinned' but .devcontainer/lib.sh expects '$RUST_VERSION'"
  fi

  local active
  active="$(cd -- "$REPOSITORY_ROOT" && rustc --version)"
  if [[ "$active" != "rustc $RUST_VERSION "* ]]; then
    die "expected rustc $RUST_VERSION in the repository, found: $active"
  fi
}

# Warns about optional tools that are not on PATH.
check_optional_tools() {
  local missing
  missing="$(missing_tools "${OPTIONAL_TOOLS[@]}")"
  if [[ -n "$missing" ]]; then
    warn "missing optional tools: $(paste -sd ' ' <<<"$missing")"
  fi
}

# Node prefix for cloud images, whose own Node (and nvm) we want to shadow on PATH.
CLOUD_NODE_PREFIX="$HOME/.local"

# Installs the toolchain on a cloud image running as root, with Node under
# CLOUD_NODE_PREFIX, and puts that Node first on this script's PATH. Callers
# persist the PATH change in whatever way their agent's shells pick up.
install_cloud_toolchain() {
  log "Installing development tools"
  NODE_PREFIX="$CLOUD_NODE_PREFIX" bash "$DEVCONTAINER_DIR/install-tools.sh"
  export PATH="$CLOUD_NODE_PREFIX/bin:$PATH"
  if [[ "$(command -v npm)" != "$CLOUD_NODE_PREFIX/bin/npm" ]]; then
    die "npm resolves to $(command -v npm), expected $CLOUD_NODE_PREFIX/bin/npm"
  fi
}

# Installs the Node dependencies used to build the documentation site.
install_docs_dependencies() {
  log "Installing documentation site dependencies"
  npm --prefix "$REPOSITORY_ROOT/docs" ci --no-audit --no-fund
}

# Installs the Codex CLI unless it is already present.
install_codex() {
  if command -v codex >/dev/null 2>&1; then
    log "Codex already installed"
    return
  fi
  log "Installing Codex"
  curl "${CURL_OPTIONS[@]}" https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh
}

# Installs the Claude Code CLI unless it is already present.
install_claude() {
  if command -v claude >/dev/null 2>&1; then
    log "Claude Code already installed"
    return
  fi
  log "Installing Claude Code"
  curl "${CURL_OPTIONS[@]}" https://claude.ai/install.sh | CI=1 bash
}
