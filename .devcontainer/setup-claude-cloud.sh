#!/usr/bin/env bash
# Set up a Claude Code cloud session. These run as root on the stock Ubuntu cloud
# image rather than in the devcontainer. The image already provides rustup, Go,
# and the claude CLI. The remaining tools and the pinned Rust toolchain are
# installed here; Node goes under ~/.local so it takes precedence over the
# image's own Node on PATH.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

install_cloud_toolchain

log "Installing Rust"
bash "$DEVCONTAINER_DIR/install-rust.sh"

# The image's /etc/profile.d/nodejs.sh prepends its own Node for login shells;
# this drop-in sorts after it so ours stays first.
echo "export PATH=\"$CLOUD_NODE_PREFIX/bin:\$PATH\"" >/etc/profile.d/zz-stompymux-node.sh

# SessionStart hooks can persist environment changes for the rest of the session.
if [[ -n "${CLAUDE_ENV_FILE:-}" ]]; then
  echo "export PATH=\"$CLOUD_NODE_PREFIX/bin:\$PATH\"" >>"$CLAUDE_ENV_FILE"
fi

require_tools
check_optional_tools

install_docs_dependencies
install_codex
warm_build_cache

log "Claude Code cloud environment ready"
