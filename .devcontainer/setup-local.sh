#!/usr/bin/env bash
# Set up a developer machine, either inside the devcontainer (where the toolchain
# is baked into the image) or directly on the host (where the developer provides
# the toolchain). Installs the docs site dependencies and coding agent CLIs.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

missing="$(missing_tools "${REQUIRED_TOOLS[@]}")"
if [[ -n "$missing" ]]; then
  cat >&2 <<EOF
error: missing required tools: $(paste -sd ' ' <<<"$missing")

Either open this repository in the devcontainer, or install the toolchain on
this machine. On Debian or Ubuntu the devcontainer installers can do this:

  sudo .devcontainer/install-tools.sh
  .devcontainer/install-rust.sh

Then re-run .devcontainer/setup.sh.
EOF
  exit 1
fi
check_optional_tools

install_docs_dependencies
install_codex
install_claude

log "Local environment ready"
