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
  echo "$match: map domain depends on the legacy Mech layout"
  status=1
done < <(rg -n -g '*.[ch]' \
  '#include "mech\.h"|#include "mech_macros\.h"|\bmech->' \
  src/btech/map || true)

while IFS= read -r match; do
  echo "$match: character domain accesses Mech identity layout directly"
  status=1
done < <(rg -n -g '*.[ch]' \
  -- '\b[A-Za-z_][A-Za-z0-9_]*->(xcode|mynum|mapindex|mapnumber)\b' \
  src/btech/character || true)

while IFS= read -r match; do
  echo "$match: converted character module uses a legacy Mech state macro"
  status=1
done < <(rg -n \
  '\b(MechPilot|MechPilotStatus|GunPilot|MechPer|MechLX|MechLY|MechX|MechY|MechTeam|MechType|MechMove|MechTons|MechXPMod|MechBV|MechCritStatus|MechMaxSpeed|MechSpeed|MechDesiredSpeed|Destroyed|NoGunXP|SetSect[A-Za-z]*|SetPart[A-Za-z]*)\s*\(' \
  src/btech/character/character_health.c \
  src/btech/character/character_experience.c \
  src/btech/character/character_battle_value.c \
  src/btech/character/pcombat.c || true)

while IFS= read -r match; do
  echo "$match: converted character module includes aggregate Mech layout"
  status=1
done < <(rg -n '#include "mech(_macros)?\.h"' \
  src/btech/character/btechstats_internal.h \
  src/btech/character/btechstats.c \
  src/btech/character/character_health.c \
  src/btech/character/character_experience.c \
  src/btech/character/character_battle_value.c \
  src/btech/character/character_persistence.c \
  src/btech/character/pcombat.c || true)

while IFS= read -r match; do
  echo "$match: legacy unconsciousness export is not allowed"
  status=1
done < <(rg -n '\bProlongUncon\b' src/btech || true)

while IFS= read -r match; do
  echo "$match: legacy personal-combat export is not allowed"
  status=1
done < <(rg -n \
  '\b(pc_to_dam_conversion|dam_to_pc_conversion|armor_effect)\b' \
  src/btech || true)

while IFS= read -r match; do
  echo "$match: character domain depends on aggregate Mech layout"
  status=1
done < <(rg -n -g '*.[ch]' \
  '#include "mech(_macros)?\.h"|\b[A-Za-z_][A-Za-z0-9_]*->(ud|pd|rd|sd)\b' \
  src/btech/character || true)

while IFS= read -r match; do
  echo "$match: legacy failure export is not allowed"
  status=1
done < <(rg -n \
  '\b(GetBrandIndex|GetPartBrandName|CheckGenericFail|CheckWeaponFailed)\b' \
  src/btech || true)

while IFS= read -r match; do
  echo "$match: tactical style renderer depends on aggregate Mech layout"
  status=1
done < <(rg -n '#include "mech(_maps_internal|_macros)?\.h"' \
  src/btech/commands/mech_command_checks.c \
  src/btech/ui/mech_base_entry.c \
  src/btech/ui/mech_broadcast.c \
  src/btech/ui/mech_lrs_map.c \
  src/btech/ui/mech_maps.c \
  src/btech/ui/mech_notify.c \
  src/btech/ui/mech_notify_radio.c \
  src/btech/ui/mech_notify_radio_config.c \
  src/btech/ui/mech_notify_weapon_text.c \
  src/btech/ui/mech_radio_render_internal.h \
  src/btech/ui/mech_status_render_internal.h \
  src/btech/ui/mech_status_templates_internal.h \
  src/btech/ui/mech_status_armor.c \
  src/btech/ui/mech_status_armor_templates.c \
  src/btech/ui/mech_status_parts.c \
  src/btech/ui/mech_status_summary.c \
  src/btech/ui/mech_status_weapons.c \
  src/btech/ui/mech_status_weapons_format.c \
  src/btech/ui/mech_tactical_command.c \
  src/btech/ui/mech_tactical_layout.c \
  src/btech/ui/mech_tactical_map.c \
  src/btech/ui/mech_tactical_style.c \
  src/btech/ui/mech_map_render_internal.h || true)

while IFS= read -r match; do
  echo "$match: converted autopilot radio module depends on aggregate Mech layout"
  status=1
done < <(rg -n \
  '#include "mech(_macros)?\.h"|\bmech->|\b(Mech[A-Z][A-Za-z0-9_]*|GetSect[A-Za-z0-9_]*|GetPart[A-Za-z0-9_]*|SetSect[A-Za-z0-9_]*|SetPart[A-Za-z0-9_]*|Destroyed|Started|Fallen|Jumping|Landed|FlyingT|HasCamo)\(' \
  src/btech/autopilot/autopilot_ai.c \
  src/btech/autopilot/autopilot_autogun.c \
  src/btech/autopilot/autopilot_bases.c \
  src/btech/autopilot/autopilot_chase_target.c \
  src/btech/autopilot/autopilot_commands.c \
  src/btech/autopilot/autopilot_command_dispatch.c \
  src/btech/autopilot/autopilot_core.c \
  src/btech/autopilot/autopilot_follow.c \
  src/btech/autopilot/autopilot_goto.c \
  src/btech/autopilot/autopilot_navigation.c \
  src/btech/autopilot/autopilot_pathfinding.c \
  src/btech/autopilot/autopilot_physical_attack.c \
  src/btech/autopilot/autopilot_radio.c \
  src/btech/autopilot/autopilot_radio_catalog.c \
  src/btech/autopilot/autopilot_radio_handlers.c \
  src/btech/autopilot/autopilot_radio_parser.c \
  src/btech/autopilot/autopilot_roam.c \
  src/btech/autopilot/autopilot_sensor_policy.c \
  src/btech/autopilot/autopilot_target_scoring.c \
  src/btech/autopilot/autopilot_weapon_profile.c || true)

if [[ -e src/btech/ui/coolmenu_interface.h ]]; then
  echo "src/btech/ui/coolmenu_interface.h: unused macro interface is not allowed"
  status=1
fi

if [[ -e src/btech/autopilot/autopilot_commands_internal.h ]]; then
  echo "src/btech/autopilot/autopilot_commands_internal.h: aggregate command header is not allowed"
  status=1
fi

if [[ -e src/btech/autopilot/autopilot_ai_internal.h ]]; then
  echo "src/btech/autopilot/autopilot_ai_internal.h: aggregate AI header is not allowed"
  status=1
fi

if find src/btech/src -type f -print -quit 2>/dev/null | grep -q .; then
  echo "src/btech/src: legacy nested source tree is not allowed"
  status=1
fi

exit "$status"
