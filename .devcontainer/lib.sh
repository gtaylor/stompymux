# Shared helpers sourced by the per-environment setup scripts in .devcontainer/.
# shellcheck shell=bash

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC2034 # Used by the scripts that source this file.
DEVCONTAINER_DIR="$REPOSITORY_ROOT/.devcontainer"

# Tools every environment needs to build, test, and check the project.
REQUIRED_TOOLS=(cargo rustfmt just stylua node npm go hugo)

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
}

# Warns about optional tools that are not on PATH.
check_optional_tools() {
  local missing
  missing="$(missing_tools "${OPTIONAL_TOOLS[@]}")"
  if [[ -n "$missing" ]]; then
    warn "missing optional tools: $(paste -sd ' ' <<<"$missing")"
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
