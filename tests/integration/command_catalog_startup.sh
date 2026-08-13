#!/usr/bin/env bash

set -euo pipefail

server=$1
missing_config=$(mktemp "${TMPDIR:-/tmp}/stompymux-missing-config.XXXXXX")
rm -f "$missing_config"
stdout_file=$(mktemp)
stderr_file=$(mktemp)
trap 'rm -f "$stdout_file" "$stderr_file"' EXIT

set +e
"$server" "$missing_config" >"$stdout_file" 2>"$stderr_file"
status=$?
set -e

if [[ $status -ne 2 ]]; then
  echo "expected configuration failure status 2, got $status" >&2
  cat "$stderr_file" >&2
  exit 1
fi

if ! grep -Fq "Error reading config file '$missing_config'" "$stderr_file"; then
  echo "server did not reach configuration loading" >&2
  cat "$stderr_file" >&2
  exit 1
fi

if grep -Fq "Unable to create MUX server resources." "$stderr_file"; then
  echo "production command catalog failed to initialize" >&2
  cat "$stderr_file" >&2
  exit 1
fi
