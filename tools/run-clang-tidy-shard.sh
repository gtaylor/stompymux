#!/usr/bin/env bash
set -euo pipefail

if (($# != 5)); then
  echo "usage: $0 BUILD_DIR SHARD SHARD_COUNT RUN_CLANG_TIDY CLANG_TIDY" >&2
  exit 2
fi

BUILD_DIR=$1
SHARD=$2
SHARD_COUNT=$3
RUN_CLANG_TIDY=$4
CLANG_TIDY=$5

if [[ ! ${SHARD} =~ ^[0-9]+$ || ! ${SHARD_COUNT} =~ ^[1-9][0-9]*$ ]] ||
  ((SHARD >= SHARD_COUNT)); then
  echo "shard must be in the range 0..shard_count-1" >&2
  exit 2
fi

mapfile -d '' -t SOURCES < <(
  find src/mux src/btech -type f -name '*.c' -print0 | sort -z
)

SELECTED=()
for ((INDEX = SHARD; INDEX < ${#SOURCES[@]}; INDEX += SHARD_COUNT)); do
  SELECTED+=(".*/${SOURCES[INDEX]}$")
done

if ((${#SELECTED[@]} == 0)); then
  echo "clang-tidy shard ${SHARD}/${SHARD_COUNT} selected no sources" >&2
  exit 2
fi

OUTPUT=$(mktemp)
trap 'rm -f "${OUTPUT}"' EXIT
STATUS=0
"${RUN_CLANG_TIDY}" \
  -clang-tidy-binary "${CLANG_TIDY}" \
  -quiet \
  -p "${BUILD_DIR}" \
  -j "$(nproc)" \
  -checks='readability-implicit-bool-conversion' \
  -warnings-as-errors='*,-readability-implicit-bool-conversion' \
  "${SELECTED[@]}" >"${OUTPUT}" 2>&1 || STATUS=$?

if ((STATUS != 0)); then
  cat "${OUTPUT}" >&2
  exit "${STATUS}"
fi

if rg -n -- "-> 'bool'" "${OUTPUT}"; then
  echo 'Implicit conversion into bool found; make the conversion explicit.' >&2
  exit 1
fi
