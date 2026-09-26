#!/usr/bin/env bash
# Install the stable Rust toolchain via rustup for the current (non-root) user.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

if command -v rustup >/dev/null 2>&1; then
  log "rustup already installed; ensuring a toolchain, rustfmt, and clippy are present"
  if ! rustup show active-toolchain >/dev/null 2>&1; then
    rustup default stable
  fi
  rustup component add rustfmt clippy
  exit 0
fi

rustup_init="$(mktemp)"
trap 'rm -f "$rustup_init"' EXIT

curl "${CURL_OPTIONS[@]}" https://sh.rustup.rs --output "$rustup_init"
sh "$rustup_init" -y --profile minimal --component rustfmt,clippy
