#!/usr/bin/env bash
# Set up a Claude Code cloud session. These run as root on the stock Ubuntu cloud
# image rather than in the devcontainer, which already provides Rust, Go, and the
# claude CLI. The remaining tools are installed here; Node goes under ~/.local so
# it takes precedence over the image's own Node on PATH.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

node_prefix="$HOME/.local"

log "Installing development tools"
NODE_PREFIX="$node_prefix" bash "$DEVCONTAINER_DIR/install-tools.sh"

# The image's /etc/profile.d/nodejs.sh prepends its own Node for login shells;
# this drop-in sorts after it so ours stays first.
echo "export PATH=\"$node_prefix/bin:\$PATH\"" >/etc/profile.d/zz-stompymux-node.sh

# SessionStart hooks can persist environment changes for the rest of the session.
if [[ -n "${CLAUDE_ENV_FILE:-}" ]]; then
  echo "export PATH=\"$node_prefix/bin:\$PATH\"" >>"$CLAUDE_ENV_FILE"
fi
export PATH="$node_prefix/bin:$PATH"

if [[ "$(command -v npm)" != "$node_prefix/bin/npm" ]]; then
  die "npm resolves to $(command -v npm), expected $node_prefix/bin/npm"
fi

require_tools
check_optional_tools

install_docs_dependencies
install_codex

log "Cloud environment ready"
