#!/usr/bin/env bash
# Detect which supported environment we are running in and run its setup script.
#
# Usage: .devcontainer/setup.sh [ci|local|claude-cloud|codex-cloud]
#
# With no argument the environment is detected:
#   ci           - GitHub Actions (GITHUB_ACTIONS=true), always inside the devcontainer.
#   claude-cloud - Claude Code cloud sessions (CLAUDE_CODE_REMOTE=true).
#   local        - Everything else: a developer machine, inside or outside the devcontainer.
# Codex cloud sets no identifying variable, so its setup script passes
# codex-cloud explicitly. STOMPYMUX_ENV may also be set to force an environment.
set -euo pipefail

devcontainer_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

environment="${1:-${STOMPYMUX_ENV:-}}"
if [[ -z "$environment" ]]; then
  if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
    environment=ci
  elif [[ "${CLAUDE_CODE_REMOTE:-}" == "true" ]]; then
    environment=claude-cloud
  else
    environment=local
  fi
fi

case "$environment" in
  ci | local | claude-cloud | codex-cloud) ;;
  *)
    echo "error: unknown environment '$environment' (expected ci, local, claude-cloud, or codex-cloud)" >&2
    exit 2
    ;;
esac

echo "==> Setting up the '$environment' environment"
exec "$devcontainer_dir/setup-$environment.sh"
