#!/usr/bin/env bash
set -euo pipefail

CMAKE_VERSION="${CMAKE_VERSION:-4.4.2}"
JUST_VERSION="${JUST_VERSION:-1.57.0}"
STYLUA_VERSION="${STYLUA_VERSION:-2.5.2}"

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

if ((EUID == 0)); then
  SUDO=()
else
  SUDO=(sudo)
fi

"${SUDO[@]}" apt-get update
"${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  ca-certificates curl

curl "${CURL_OPTIONS[@]}" \
  https://apt.llvm.org/llvm-snapshot.gpg.key \
  --output /tmp/apt.llvm.org.asc
"${SUDO[@]}" install -m 0644 \
  /tmp/apt.llvm.org.asc /etc/apt/trusted.gpg.d/apt.llvm.org.asc
echo 'deb http://apt.llvm.org/noble/ llvm-toolchain-noble-22 main' \
  | "${SUDO[@]}" tee /etc/apt/sources.list.d/llvm.list >/dev/null

"${SUDO[@]}" apt-get update
"${SUDO[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  build-essential gcc-14 g++-14 clang-22 clang-format-22 clang-tidy-22 \
  clang-tools-22 libclang-22-dev libclang-rt-22-dev llvm-22-dev clangd-22 \
  libsqlite3-dev ripgrep sqlite3 unzip xxd

CMAKE_ARCHIVE="cmake-${CMAKE_VERSION}-linux-x86_64.tar.gz"
curl "${CURL_OPTIONS[@]}" \
  "https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/${CMAKE_ARCHIVE}" \
  --output "/tmp/${CMAKE_ARCHIVE}"
"${SUDO[@]}" tar --extract --gzip --file "/tmp/${CMAKE_ARCHIVE}" \
  --strip-components=1 --directory /usr/local

curl "${CURL_OPTIONS[@]}" \
  https://just.systems/install.sh --output /tmp/just-install.sh
"${SUDO[@]}" bash /tmp/just-install.sh --to /usr/local/bin --tag "${JUST_VERSION}"

STYLUA_ARCHIVE="stylua-linux-x86_64.zip"
curl "${CURL_OPTIONS[@]}" \
  "https://github.com/JohnnyMorganz/StyLua/releases/download/v${STYLUA_VERSION}/${STYLUA_ARCHIVE}" \
  --output "/tmp/${STYLUA_ARCHIVE}"
"${SUDO[@]}" unzip -oq "/tmp/${STYLUA_ARCHIVE}" -d /usr/local/bin

rm -f \
  /tmp/apt.llvm.org.asc \
  "/tmp/${CMAKE_ARCHIVE}" \
  /tmp/just-install.sh \
  "/tmp/${STYLUA_ARCHIVE}"
"${SUDO[@]}" rm -rf /var/lib/apt/lists/*
