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

while IFS= read -r match; do
  echo "$match: non-unit source accesses a private Mech state component"
  status=1
done < <(rg -n -g '*.[ch]' -g '!src/btech/unit/**' \
  -- '->(ud|pd|rd|sd)\b' src/btech || true)

while IFS= read -r match; do
  echo "$match: legacy stagger API name"
  status=1
done < <(rg -n '\b(MarkStaggerDamage|RemoveStaggerDamage|ClearAllStaggerDamage|ClearStaggerDamage|CurrentStaggerDamage|CurrentCountedStaggerDamage)\b' \
  src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: converted boundary accesses Mech layout directly"
  status=1
done < <(rg -n -g '*.[ch]' -- '\bmech->' src/btech/core src/btech/commands \
  src/btech/integration || true)

while IFS= read -r match; do
  echo "$match: converted sensors boundary accesses Mech layout directly"
  status=1
done < <(rg -n -g '*.[ch]' \
  -- '\b(mech|[A-Za-z_][A-Za-z0-9_]*Mech|seer|target|spotter|m|t)->' \
  src/btech/sensors || true)

while IFS= read -r match; do
  echo "$match: converted map UI boundary accesses Mech layout directly"
  status=1
done < <(rg -n \
  -- '\b(mech|tempMech|tmpm|oMech)->(xcode|mynum|mapindex|mapnumber|brief|ID)\b' \
  src/btech/ui/mech_maps.c src/btech/ui/mech_base_entry.c \
  src/btech/ui/mech_lrs_map.c src/btech/ui/mech_tactical_command.c || true)

while IFS= read -r match; do
  echo "$match: converted scripting boundary accesses Mech layout directly"
  status=1
done < <(rg -n -g '*.[ch]' \
  -- '\b(mech|mechA|mechB|target)->(xcode|mynum|mapindex|mapnumber|freq|freqmodes|chantitle)\b' \
  src/btech/scripting || true)

while IFS= read -r match; do
  echo "$match: converted special-object boundary accesses Mech layout directly"
  status=1
done < <(rg -n \
  -- '\bmech->(xcode|mynum|mapindex|mapnumber|brief|ID)\b' \
  src/btech/special/registry_loading.c || true)

while IFS= read -r match; do
  echo "$match: special-object domain depends on the legacy Mech aggregate"
  status=1
done < <(rg -n -g '*.[ch]' '#include "mech\.h"|\b(MechAuto|MechType|Started)\s*\(' \
  src/btech/special || true)

while IFS= read -r match; do
  echo "$match: converted integration or persistence code depends on the legacy Mech aggregate"
  status=1
done < <(rg -n -g '*.[ch]' '#include "mech\.h"|#include "mech_macros\.h"' \
  src/btech/integration src/btech/persistence || true)

while IFS= read -r match; do
  echo "$match: core depends on the legacy Mech aggregate"
  status=1
done < <(rg -n -g '*.[ch]' '#include "mech\.h"|#include "mech_macros\.h"' \
  src/btech/core || true)

while IFS= read -r match; do
  echo "$match: commands depend on the legacy Mech aggregate"
  status=1
done < <(rg -n -g '*.[ch]' '#include "mech\.h"|#include "mech_macros\.h"' \
  src/btech/commands || true)

while IFS= read -r match; do
  echo "$match: economy depends on a legacy Mech interface"
  status=1
done < <(rg -n -g '*.[ch]' \
  '#include "mech\.h"|#include "mech_macros\.h"|#include "mech_utils_internal\.h"|\b(CalcFasaCost|GetPartWeight|MechNumHeatsinksInEngine)\b' \
  src/btech/economy || true)

while IFS= read -r match; do
  echo "$match: converted map module depends on the legacy Mech aggregate"
  status=1
done < <(rg -n '#include "mech\.h"|#include "mech_macros\.h"' \
  src/btech/map/map.c src/btech/map/map_buildings.c \
  src/btech/map/map_dynamic.c src/btech/map/map_obj.c \
  src/btech/map/map_obj_commands.c src/btech/map/map_obj_internal.h \
  src/btech/map/map_terrain.c || true)

if find src/btech/src -type f -print -quit 2>/dev/null | grep -q .; then
  echo "src/btech/src: legacy nested source tree is not allowed"
  status=1
fi

exit "$status"
