#!/usr/bin/env bash

set -euo pipefail

root=${1:?repository root is required}
cd "$root"

status=0

while IFS= read -r -d '' path; do
  lines=$(awk 'END { print NR }' "$path")
  if ((lines > 800)); then
    echo "$path: $lines lines (maximum 800)"
    status=1
  fi
done < <(find src/btech -type f \( -name '*.c' -o -name '*.h' \) -print0)

while IFS= read -r path; do
  base=${path##*/}
  case "$base" in
    CMakeLists.txt|.clang-tidy) continue ;;
  esac
  if [[ "$base" == *.*.* ]]; then
    echo "$path: dotted legacy filename"
    status=1
  fi
  if [[ "$base" == p_*.h ]]; then
    echo "$path: generated-style prototype header"
    status=1
  fi
done < <(find src/btech -type f -print | sort)

while IFS= read -r match; do
  echo "$match: MUX includes a private BTech header"
  status=1
done < <(rg -n '^#include "(registry_|special_object|mech_|map_|autopilot_|btconfig|legacy_macros|mech_macros)' src/mux -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: non-unit source includes the private Mech layout"
  status=1
done < <(rg -n '^#include "mech_internal\.h"' src/btech \
  -g '*.[ch]' -g '!src/btech/unit/**' || true)

if find src/btech/src -type f -print -quit 2>/dev/null | grep -q .; then
  echo "src/btech/src: legacy nested source tree is not allowed"
  status=1
fi

exit "$status"
