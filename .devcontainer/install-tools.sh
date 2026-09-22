#!/usr/bin/env bash
# Install architecture-matched tools used by the Rust, Lua, and Hugo workflows.
set -euo pipefail

JUST_VERSION="${JUST_VERSION:-1.57.0}"
STYLUA_VERSION="${STYLUA_VERSION:-2.5.2}"
LUA_LANGUAGE_SERVER_VERSION="${LUA_LANGUAGE_SERVER_VERSION:-3.19.1}"
NODE_VERSION="${NODE_VERSION:-24.13.0}"
HUGO_VERSION="${HUGO_VERSION:-0.164.0}"

case "$(uname -m)" in
  x86_64)
    RUST_ARCH=x86_64
    STYLUA_ARCH=x86_64
    LUA_LANGUAGE_SERVER_ARCH=x64
    NODE_ARCH=x64
    HUGO_ARCH=amd64
    ;;
  aarch64)
    RUST_ARCH=aarch64
    STYLUA_ARCH=aarch64
    LUA_LANGUAGE_SERVER_ARCH=arm64
    NODE_ARCH=arm64
    HUGO_ARCH=arm64
    ;;
  *)
    echo "Unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

CURL_OPTIONS=(
  --location
  --proto '=https'
  --tlsv1.2
  --fail
  --silent
  --show-error
  --retry 5
  --retry-delay 2
  --retry-all-errors
)

export DEBIAN_FRONTEND=noninteractive
apt-get -o Acquire::Retries=5 update
apt-get -o Acquire::Retries=5 install -y --no-install-recommends \
  build-essential ca-certificates curl git golang-go pkg-config python3 \
  ripgrep sqlite3 unzip xz-utils

download_dir="$(mktemp -d)"
trap 'rm -rf "$download_dir"' EXIT

curl "${CURL_OPTIONS[@]}" \
  "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${RUST_ARCH}-unknown-linux-musl.tar.gz" \
  --output "$download_dir/just.tar.gz"
tar -xzf "$download_dir/just.tar.gz" -C /usr/local/bin just

curl "${CURL_OPTIONS[@]}" \
  "https://github.com/JohnnyMorganz/StyLua/releases/download/v${STYLUA_VERSION}/stylua-linux-${STYLUA_ARCH}.zip" \
  --output "$download_dir/stylua.zip"
unzip -oq "$download_dir/stylua.zip" stylua -d /usr/local/bin
chmod 0755 /usr/local/bin/stylua

lua_language_server_dir="/opt/lua-language-server-${LUA_LANGUAGE_SERVER_VERSION}"
curl "${CURL_OPTIONS[@]}" \
  "https://github.com/LuaLS/lua-language-server/releases/download/${LUA_LANGUAGE_SERVER_VERSION}/lua-language-server-${LUA_LANGUAGE_SERVER_VERSION}-linux-${LUA_LANGUAGE_SERVER_ARCH}.tar.gz" \
  --output "$download_dir/lua-language-server.tar.gz"
install -d -m 0755 "$lua_language_server_dir"
tar -xzf "$download_dir/lua-language-server.tar.gz" -C "$lua_language_server_dir"
# LuaLS writes generated standard-library metadata here after the vscode UID is remapped.
chmod 1777 "$lua_language_server_dir/meta"
ln -s "$lua_language_server_dir/bin/lua-language-server" /usr/local/bin/lua-language-server

curl "${CURL_OPTIONS[@]}" \
  "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${NODE_ARCH}.tar.xz" \
  --output "$download_dir/node.tar.xz"
tar -xJf "$download_dir/node.tar.xz" --strip-components=1 -C /usr/local

curl "${CURL_OPTIONS[@]}" \
  "https://github.com/gohugoio/hugo/releases/download/v${HUGO_VERSION}/hugo_extended_${HUGO_VERSION}_linux-${HUGO_ARCH}.tar.gz" \
  --output "$download_dir/hugo.tar.gz"
tar -xzf "$download_dir/hugo.tar.gz" -C /usr/local/bin hugo

rm -rf /var/lib/apt/lists/*
