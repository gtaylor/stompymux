#!/usr/bin/env bash
# Set up a Codex cloud environment. Run this as the environment's setup script
# (and maintenance script) in the Codex environment settings; Codex cloud has no
# repository-level startup hook, and its agent phase has no internet access by
# default, so everything must be installed here. Containers use the
# codex-universal image, running as root, which manages Rust with rustup and
# Node with nvm. Node goes under ~/.local so it takes precedence over nvm's.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

install_cloud_toolchain

log "Installing Rust"
bash "$DEVCONTAINER_DIR/install-rust.sh"
# shellcheck source=/dev/null
source "$HOME/.cargo/env"

# Exports made here do not reach the agent phase, but ~/.bashrc does. It runs
# after /etc/profile sources nvm, so our Node stays first.
bashrc_marker="# stompymux codex-cloud environment"
if ! grep -qxF "$bashrc_marker" "$HOME/.bashrc" 2>/dev/null; then
  cat >>"$HOME/.bashrc" <<BASHRC

$bashrc_marker
. "\$HOME/.cargo/env"
export PATH="$CLOUD_NODE_PREFIX/bin:\$PATH"
BASHRC
fi

require_tools
check_optional_tools

install_docs_dependencies
install_claude

log "Codex cloud environment ready"
