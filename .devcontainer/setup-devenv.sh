#!/usr/bin/env bash
# Set up whatever is needed for interactive development. Codex CLI, Claude Code, any other
# dev stuff that's not needed in CI.
set -euo pipefail

# We reuse the devcontainer across CI and in development. Bail out early if we detect
# that we're running within CI.
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  exit 0
fi

REPOSITORY_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${REPOSITORY_ROOT}"
just update-submodules

curl --fail --silent --show-error --location \
  https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh

curl --fail --silent --show-error --location \
  https://claude.ai/install.sh | CI=1 bash
