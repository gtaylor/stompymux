#!/usr/bin/env bash
# Prepare interactive development tools after the workspace is mounted.
set -euo pipefail

if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  exit 0
fi

repository_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repository_root"

npm --prefix docs ci

curl --fail --silent --show-error --location \
  https://chatgpt.com/codex/install.sh | CODEX_NON_INTERACTIVE=1 sh

curl --fail --silent --show-error --location \
  https://claude.ai/install.sh | CI=1 bash
