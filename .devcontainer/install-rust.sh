#!/usr/bin/env bash
# Install the pinned Rust toolchain (RUST_VERSION in lib.sh) via rustup for the
# current user and make it the default.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

if ! command -v rustup >/dev/null 2>&1; then
  log "Installing rustup"
  rustup_init="$(mktemp)"
  trap 'rm -f "$rustup_init"' EXIT
  curl "${CURL_OPTIONS[@]}" https://sh.rustup.rs --output "$rustup_init"
  sh "$rustup_init" -y --profile minimal --default-toolchain none
  # shellcheck source=/dev/null
  source "$HOME/.cargo/env"
fi

log "Installing Rust $RUST_VERSION"
rustup toolchain install "$RUST_VERSION" --profile minimal --component rustfmt,clippy
rustup default "$RUST_VERSION"
