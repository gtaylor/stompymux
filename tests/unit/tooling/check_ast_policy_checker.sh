#!/usr/bin/env bash

set -euo pipefail

checker=${1:?checker path is required}
root=${2:?repository root is required}
fixtures=$root/tests/fixtures/ast_policy_checker
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT

for source in "$fixtures"/*.[ch]; do
  base=${source##*/}
  mkdir -p "$work/src/mux/fixtures"
  cp "$source" "$work/src/mux/fixtures/$base"
done

{
  printf '[\n'
  separator=
  for source in "$work"/src/mux/fixtures/*.c; do
    printf '%s  {"directory":"%s","command":"clang-22 -std=c23 -c %s","file":"%s"}' \
      "$separator" "$work" "$source" "$source"
    separator=$',\n'
  done
  printf '\n]\n'
} >"$work/compile_commands.json"

expect_pass() {
  local policy=$1
  local source=$2
  "$checker" -p "$work" --checks="$policy" \
    "$work/src/mux/fixtures/$source" >/dev/null 2>&1
}

expect_failure() {
  local policy=$1
  local source=$2
  local message=$3
  local output=$work/output
  if "$checker" -p "$work" --checks="$policy" \
      "$work/src/mux/fixtures/$source" >"$output" 2>&1; then
    echo "$policy failed to reject $source" >&2
    return 1
  fi
  if ! rg -q "$message" "$output"; then
    cat "$output" >&2
    echo "$policy emitted the wrong diagnostic for $source" >&2
    return 1
  fi
}

expect_failure status-accessors status_violation.c \
  'Raw bitwise operation on a persisted unit status word'
expect_pass status-accessors status_accepted.c
expect_failure enum-underlying-type enum_violation.c \
  'Named enum without an explicit underlying type found'
expect_pass enum-underlying-type enum_accepted.c
expect_failure checked-suffix-order suffix_violation.c \
  'NUL write precedes a checked suffix of the same pointer'
expect_pass checked-suffix-order suffix_accepted.c
expect_failure boolean-contracts boolean_violation.c \
  'Integer-returning predicate found'
expect_pass boolean-contracts boolean_accepted.c

if "$checker" -p "$work" --checks=unknown \
    "$work/src/mux/fixtures/status_accepted.c" >/dev/null 2>&1; then
  echo 'unknown policy was accepted' >&2
  exit 1
fi
