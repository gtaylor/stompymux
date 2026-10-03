#!/usr/bin/env bash
# Install architecture-matched tools used by the Rust, Lua, and Hugo workflows.
# Must run as root. Tools already present at the pinned version are skipped, so
# re-running is cheap. Set NODE_PREFIX to install Node somewhere other than
# /usr/local.
set -euo pipefail

# shellcheck source=.devcontainer/lib.sh
source "$(dirname -- "${BASH_SOURCE[0]}")/lib.sh"

JUST_VERSION="${JUST_VERSION:-1.57.0}"
STYLUA_VERSION="${STYLUA_VERSION:-2.5.2}"
NEXTEST_VERSION="${NEXTEST_VERSION:-0.9.146}"
LUA_LANGUAGE_SERVER_VERSION="${LUA_LANGUAGE_SERVER_VERSION:-3.19.1}"
NODE_VERSION="${NODE_VERSION:-24.13.0}"
HUGO_VERSION="${HUGO_VERSION:-0.164.0}"
NODE_PREFIX="${NODE_PREFIX:-/usr/local}"

case "$(uname -m)" in
  x86_64)
    RUST_ARCH=x86_64
    NEXTEST_PLATFORM=linux
    STYLUA_ARCH=x86_64
    LUA_LANGUAGE_SERVER_ARCH=x64
    NODE_ARCH=x64
    HUGO_ARCH=amd64
    ;;
  aarch64)
    RUST_ARCH=aarch64
    NEXTEST_PLATFORM=linux-arm
    STYLUA_ARCH=aarch64
    LUA_LANGUAGE_SERVER_ARCH=arm64
    NODE_ARCH=arm64
    HUGO_ARCH=arm64
    ;;
  *)
    die "unsupported architecture: $(uname -m)"
    ;;
esac

# Succeeds when the binary exists and its version output mentions the version.
has_version() {
  local binary="$1" version="$2"
  [[ -x "$binary" ]] && "$binary" --version 2>/dev/null | grep -qF "$version"
}

export DEBIAN_FRONTEND=noninteractive
apt-get -o Acquire::Retries=5 update
apt-get -o Acquire::Retries=5 install -y --no-install-recommends \
  build-essential ca-certificates curl git golang-go pkg-config python3 \
  ripgrep sqlite3 unzip xz-utils
rm -rf /var/lib/apt/lists/*

download_dir="$(mktemp -d)"
trap 'rm -rf "$download_dir"' EXIT

if has_version /usr/local/bin/just "$JUST_VERSION"; then
  log "just $JUST_VERSION already installed"
else
  log "Installing just $JUST_VERSION"
  curl "${CURL_OPTIONS[@]}" \
    "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${RUST_ARCH}-unknown-linux-musl.tar.gz" \
    --output "$download_dir/just.tar.gz"
  tar -xzf "$download_dir/just.tar.gz" -C /usr/local/bin just
fi

if has_version /usr/local/bin/stylua "$STYLUA_VERSION"; then
  log "StyLua $STYLUA_VERSION already installed"
else
  log "Installing StyLua $STYLUA_VERSION"
  curl "${CURL_OPTIONS[@]}" \
    "https://github.com/JohnnyMorganz/StyLua/releases/download/v${STYLUA_VERSION}/stylua-linux-${STYLUA_ARCH}.zip" \
    --output "$download_dir/stylua.zip"
  unzip -oq "$download_dir/stylua.zip" stylua -d /usr/local/bin
  chmod 0755 /usr/local/bin/stylua
fi

if has_version /usr/local/bin/cargo-nextest "$NEXTEST_VERSION"; then
  log "cargo-nextest $NEXTEST_VERSION already installed"
else
  log "Installing cargo-nextest $NEXTEST_VERSION"
  curl "${CURL_OPTIONS[@]}" \
    "https://get.nexte.st/${NEXTEST_VERSION}/${NEXTEST_PLATFORM}" \
    --output "$download_dir/cargo-nextest.tar.gz"
  tar -xzf "$download_dir/cargo-nextest.tar.gz" -C /usr/local/bin cargo-nextest
fi

lua_language_server_dir="/opt/lua-language-server-${LUA_LANGUAGE_SERVER_VERSION}"
if [[ -x "$lua_language_server_dir/bin/lua-language-server" ]]; then
  log "lua-language-server $LUA_LANGUAGE_SERVER_VERSION already installed"
else
  log "Installing lua-language-server $LUA_LANGUAGE_SERVER_VERSION"
  curl "${CURL_OPTIONS[@]}" \
    "https://github.com/LuaLS/lua-language-server/releases/download/${LUA_LANGUAGE_SERVER_VERSION}/lua-language-server-${LUA_LANGUAGE_SERVER_VERSION}-linux-${LUA_LANGUAGE_SERVER_ARCH}.tar.gz" \
    --output "$download_dir/lua-language-server.tar.gz"
  install -d -m 0755 "$lua_language_server_dir"
  tar -xzf "$download_dir/lua-language-server.tar.gz" -C "$lua_language_server_dir"
  # LuaLS writes generated standard-library metadata here after the vscode UID is remapped.
  chmod 1777 "$lua_language_server_dir/meta"
fi
ln -sf "$lua_language_server_dir/bin/lua-language-server" /usr/local/bin/lua-language-server

if has_version "$NODE_PREFIX/bin/node" "v$NODE_VERSION"; then
  log "Node $NODE_VERSION already installed in $NODE_PREFIX"
else
  log "Installing Node $NODE_VERSION into $NODE_PREFIX"
  curl "${CURL_OPTIONS[@]}" \
    "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODE_ARCH}.tar.xz" \
    --output "$download_dir/node.tar.xz"
  install -d -m 0755 "$NODE_PREFIX"
  tar -xJf "$download_dir/node.tar.xz" --strip-components=1 -C "$NODE_PREFIX"
fi

if [[ -x /usr/local/bin/hugo ]] && /usr/local/bin/hugo version 2>/dev/null | grep -qF "v${HUGO_VERSION}"; then
  log "Hugo $HUGO_VERSION already installed"
else
  log "Installing Hugo $HUGO_VERSION"
  curl "${CURL_OPTIONS[@]}" \
    "https://github.com/gohugoio/hugo/releases/download/v${HUGO_VERSION}/hugo_extended_${HUGO_VERSION}_linux-${HUGO_ARCH}.tar.gz" \
    --output "$download_dir/hugo.tar.gz"
  tar -xzf "$download_dir/hugo.tar.gz" -C /usr/local/bin hugo
fi
