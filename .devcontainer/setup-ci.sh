#!/usr/bin/env bash
# Set up GitHub Actions runs. CI always runs inside the devcontainer image, which
# already bakes in every tool, so this only verifies the image is complete.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

require_tools
log "CI environment ready"
