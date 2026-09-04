#!/usr/bin/env bash

set -euo pipefail

root=${1:?repository root is required}
build_root=${2:-$root/.build}
cd "$root"

if ! command -v rg >/dev/null 2>&1; then
  echo "btech architecture check requires ripgrep (rg)" >&2
  exit 1
fi

status=0
architecture_started=$(date +%s%N)
header_log_root=

if rg -n '\b__clang__\b' src; then
  echo "src: compiler-specific __clang__ conditionals are not allowed"
  status=1
fi

match=$(rg -n '\bBTECH_INTERNAL\b' src tests CMakeLists.txt \
  -g '!check_btech_architecture.sh' || true)
if [[ -n "$match" ]]; then
  echo "$match"
  echo "BTECH_INTERNAL is not allowed; include private dependencies explicitly"
  status=1
fi

match=$(rg -n '\b(BT_COMPLEXREPAIRS|BTECH_COMPLEX_REPAIRS|btech_complexrepair|btech_context_uses_complex_repairs)\b' \
  src tests CMakeLists.txt -g '!check_btech_architecture.sh' || true)
if [[ -n "$match" ]]; then
  echo "$match"
  echo "removed complex-repair build and runtime switches are not allowed"
  status=1
fi

match=$(
  rg -n '#include ".*context_internal\.h"' src/mux --glob '*.[ch]' || true
  rg -n '#include ".*context_internal\.h"' src/btech --glob '*.h' \
    -g '!core/context_internal.h' || true
)
if [[ -n "$match" ]]; then
  echo "$match"
  echo "context_internal.h is private to BTech implementation files"
  status=1
fi

nested_positional_initializer_pattern='\.[A-Za-z_][A-Za-z0-9_]*[[:space:]]*=[[:space:]]*\{(?![[:space:]]*(?:\.|\[|0[[:space:]]*\}))'

ratchet_expect_match() {
  local fixture=$1
  if ! printf '%s\n' "$fixture" |
    rg --pcre2 -q -U "$nested_positional_initializer_pattern"; then
    echo "designated-initializer ratchet failed to reject: $fixture"
    return 1
  fi
}

ratchet_expect_no_match() {
  local fixture=$1
  if printf '%s\n' "$fixture" |
    rg --pcre2 -q -U "$nested_positional_initializer_pattern"; then
    echo "designated-initializer ratchet incorrectly rejected: $fixture"
    return 1
  fi
}

ratchet_expect_match '.pair = {left, right}' || status=1
ratchet_expect_match $'.future_value = {\n  left,\n  right\n}' || status=1
ratchet_expect_match '.state = {0, 1}' || status=1
ratchet_expect_no_match '.pair = {.x = left, .y = right}' || status=1
ratchet_expect_no_match '.items = {[0] = left}' || status=1
ratchet_expect_no_match '.state = {0}' || status=1

while IFS= read -r path; do
  if tail -n 1 "$path" | rg -q '^#include[[:space:]]'; then
    echo "$path: source file ends with an include directive"
    status=1
  fi
done < <(find src/btech -type f -name '*.c' -print | sort)

if rg --pcre2 -n -U "$nested_positional_initializer_pattern" \
  src/btech -g '*.[ch]'; then
  echo "src/btech: nested aggregate values require designated initializers"
  status=1
fi

# Function-like macros are limited to the five command invoker generators.
# Additions and stale allowlist entries both fail this check.
macro_allowlist=tests/unit/btech/contracts/btech_function_macro_allowlist.txt
actual_macros=$(mktemp)
allowed_macros=$(mktemp)
macro_definitions=$(mktemp)
cleanup() {
  rm -f "$actual_macros" "$allowed_macros" "$macro_definitions"
  if [[ -n $header_log_root ]]; then
    rm -rf -- "$header_log_root"
  fi
}
trap cleanup EXIT

rg -n --glob '*.{c,h}' '^\s*#\s*define\s+[A-Za-z_][A-Za-z0-9_]*\(' src/btech \
  >"$macro_definitions"
if [[ $(wc -l <"$macro_definitions") -ne 5 ]]; then
  echo "src/btech: expected exactly five command-generator macro definitions"
  status=1
fi

sed -E 's/^([^:]+):[0-9]+:.*#[[:space:]]*define[[:space:]]+([A-Za-z_][A-Za-z0-9_]*)\(.*/\1:\2/' \
  "$macro_definitions" |
  sort -u >"$actual_macros"
sed -E '/^[[:space:]]*(#|$)/d; s/[[:space:]]+#.*$//' "$macro_allowlist" |
  sort -u >"$allowed_macros"

while IFS= read -r macro; do
  echo "$macro: non-allowlisted function-like macro"
  status=1
done < <(comm -23 "$actual_macros" "$allowed_macros")

while IFS= read -r macro; do
  echo "$macro_allowlist: stale macro allowlist entry: $macro"
  status=1
done < <(comm -13 "$actual_macros" "$allowed_macros")

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
  echo "$match: disabled legacy implementation is not allowed"
  status=1
done < <(rg -n '#if[[:space:]]+0|#if[[:space:]]+false' src/btech \
  -g '*.[ch]' || true)

if [[ -e src/btech/unit/mech.h ]]; then
  echo "src/btech/unit/mech.h: aggregate Mech layout header is not allowed"
  status=1
fi

if [[ -e src/btech/unit/template_legacy_load.c ]]; then
  echo "src/btech/unit/template_legacy_load.c: positional template loader is not allowed"
  status=1
fi

while IFS= read -r match; do
  echo "$match: legacy template loading control is not allowed"
  status=1
done < <(rg -n 'LOADNEW_LOADS_|template_load_legacy' src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: removed fixed compile-time choice is not allowed"
  status=1
done < <(rg -n '\b(ADVANCED_LOS|BT_CALCULATE_BV|BT_PARTIAL|BT_PART_WEIGHTS|BT_USE_VRT|BUILDINGS_REBUILD_FROM_DESTRUCTION|BUILDINGS_REPAIR_THEMSELVES|C3_SUPPORT|CLAN_SUPPORT|ECM_ON_CONTACTS|ECON_ALLOW_MULTIPLE_LOAD_UNLOAD|LOCK_TICK|ODDJUMP|TEMPLATE_VERBOSE_ERRORS|WEIGHTVARIABLE_STATUS|HEX_BASED|ONE_LINE_TEXTS)\b' \
  src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  [[ -z "$match" ]] && continue
  echo "$match: retired movement-mode and MW3 stats compile choices are not allowed"
  status=1
done < <(rg -n '\b(BTECH_MOVEMENT_MODES|BT_MOVEMENT_MODES|BTECH_MW3STATS|BT_EXILE_MW3STATS)\b' \
  CMakeLists.txt src -g 'CMakeLists.txt' -g '*.[ch]' -g '*.in' || true)

while IFS= read -r match; do
  [[ -z "$match" ]] && continue
  echo "$match: retired debug or persistence prepare compile indirection is not allowed"
  status=1
done < <(rg -n '\b(DPRINTK|dprintk|BTECH_PERSISTENCE_PREPARE_IMPLEMENTATION|SQLITE3_PREPARE_V2)\b|#\s*(ifdef|ifndef)\s+DEBUG\b' \
  src -g '*.[ch]' || true)

if [[ -e src/btech/sensors/object_spatial.h ||
      -e src/btech/autopilot/spath_api.h ]]; then
  echo "src/btech: retired spatial pathfinding headers are not allowed"
  status=1
fi

if [[ -e src/btech/repair/mech_tech.h ]]; then
  echo "src/btech/repair/mech_tech.h: legacy repair macro header is not allowed"
  status=1
fi

if [[ -e src/btech/core/legacy_macros.h ]]; then
  echo "src/btech/core/legacy_macros.h: caller-control macros are not allowed"
  status=1
fi

if [[ -e src/btech/ui/mech_notify.h ]]; then
  echo "src/btech/ui/mech_notify.h: notification audiences belong to mech_notify_api.h"
  status=1
fi

if [[ -e src/btech/special/registry_commands.c ||
      -e src/btech/special/registry_commands_aux.c ]]; then
  echo "src/btech/special: central command catalog is not allowed"
  status=1
fi

if [[ -e src/btech/commands/command_legacy_wrappers.c ]]; then
  echo "src/btech/commands/command_legacy_wrappers.c: legacy command wrappers are not allowed"
  status=1
fi

while IFS= read -r match; do
  echo "$match: command registry callback exposes a void object"
  status=1
done < <(rg -n 'BtechCommandHandler.*void|\(DbRef actor, void \*object' \
  src/btech/special/command_registry.h || true)

while IFS= read -r match; do
  echo "$match: runtime code must read context-owned XP thresholds"
  status=1
done < <(rg -n 'default_xp_threshold' src/btech -g '*.[ch]' \
  -g '!character_value_catalog.c' -g '!btechstats.h' || true)

while IFS= read -r match; do
  echo "$match: XP threshold changes must identify their BtechContext"
  status=1
done < <(rg --pcre2 -n -U \
  'CharacterValueThreshold\)[[:space:]]*\{(?:(?!\.context)[\s\S])*?\}' \
  src/btech -g '*.c' || true)

if ! bash tests/unit/btech/contracts/check_writable_globals.sh \
  "$root" "$build_root" \
  tests/unit/btech/contracts/writable_global_allowlist.txt; then
  status=1
fi

while IFS= read -r match; do
  echo "$match: non-unit source includes a private unit layout header"
  status=1
done < <(rg -n '^#include "(mech_internal|mech_macros)\.h"' src/btech \
  -g '*.[ch]' -g '!src/btech/unit/**' || true)

while IFS= read -r match; do
  echo "$match: non-special source includes the private registry layout"
  status=1
done < <(rg -n '^#include "registry_internal\.h"' src/btech \
  -g '*.[ch]' -g '!src/btech/special/**' || true)

while IFS= read -r match; do
  echo "$match: non-unit source requires a complete Mech value"
  status=1
done < <(rg -n 'sizeof\(Mech\)|\bMech[[:space:]]+[A-Za-z_][A-Za-z0-9_]*[;=]' \
  src/btech -g '*.[ch]' -g '!src/btech/unit/**' | \
  rg -v 'typedef struct Mech' || true)

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
  echo "$match: converted sensor module uses a legacy Mech layout interface"
  status=1
done < <(rg -n \
  '#include "mech(_macros)?\.h"|\b(Mech[A-Z][A-Za-z0-9_]*|GetSect[A-Za-z0-9_]*|GetPart[A-Za-z0-9_]*|SetSect[A-Za-z0-9_]*|SetPart[A-Za-z0-9_]*|Destroyed|Started|Fallen|Jumping|Landed|HasWorkingECMSuite|ECMProtected|ECMCountered|AnyECMDisturbed)\(' \
  src/btech/sensors/los_trace.c \
  src/btech/sensors/mech_ecm.c \
  src/btech/sensors/mech_lite.c \
  src/btech/sensors/mech_sensor_events.c \
  src/btech/sensors/mech_sensor_functions.c \
  src/btech/sensors/mech_sensor.c \
  src/btech/sensors/mech_sensor_selection.c \
  src/btech/sensors/mech_scan_navigation.c \
  src/btech/sensors/mech_scan.c \
  src/btech/sensors/mech_scan_view.c \
  src/btech/sensors/mech_los.c \
  src/btech/sensors/mech_spot.c \
  src/btech/sensors/mech_contacts.c \
  src/btech/sensors/mech_c3.c \
  src/btech/sensors/mech_c3_misc.c \
  src/btech/sensors/mech_c3i.c \
  src/btech/sensors/mech_tag.c || true)

while IFS= read -r match; do
  echo "$match: sensor catalogue exposes aggregate Mech layout"
  status=1
done < <(rg -n '#include "mech(_macros)?\.h"' \
  src/btech/sensors/mech_sensor.h || true)

while IFS= read -r match; do
  echo "$match: converted ice hazard module uses an aggregate domain layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|map|t)->|\b(MechWeapons|MapBridgesCS)\b' \
  src/btech/movement/mech_ice.c || true)

while IFS= read -r match; do
  echo "$match: converted turret module uses the aggregate Mech layout"
  status=1
done < <(rg -n \
  '#include "mech(_macros)?\.h"|\bmech->|\bMech[A-Z][A-Za-z0-9_]*\(|#if 0|\bnewturret\b' \
  src/btech/movement/ds_turret.c src/btech/unit/turret.h || true)

while IFS= read -r match; do
  echo "$match: converted bomb module uses a legacy aggregate interface"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|map)->|\b(Mech[A-Z][A-Za-z0-9_]*|GetPart[A-Za-z0-9_]*|SetPart[A-Za-z0-9_]*|Landed)\(|\b(BOMBINFO|bomb_shot|BombWeight|DestroyBomb|calc_dest|simulate_flight)\b' \
  src/btech/movement/aero_bomb.c src/btech/movement/aero_bomb_api.h || true)

while IFS= read -r match; do
  echo "$match: legacy cargo-weight export is not allowed"
  status=1
done < <(rg -n '\bSetCargoWeight\b' src/btech || true)

if [[ -e src/btech/movement/aero_bomb.h ]]; then
  echo "src/btech/movement/aero_bomb.h: unused bomb implementation header"
  status=1
fi

while IFS= read -r match; do
  echo "$match: converted collision module uses an aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(me|mech|map)->|\b(Mech(X|Y|Type|Team|Speed|Facing|Lateral|JumpHeading|RTons|Charge)|Destroyed|Started|Landed|Jumping|OODing|IsDS|Fallen|JumpSpeedMP)\(' \
  src/btech/movement/mech_domino.c || true)

while IFS= read -r match; do
  echo "$match: legacy collision or weight export is not allowed"
  status=1
done < <(rg -n '\b(domino_space|mechs_in_hex|cause_damage|get_weight)\b' \
  src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: converted DropShip bay module uses an aggregate domain layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|ds|tempMech|tmpm|car|map|mech_map|tmpmap)->|\b(AeroBay|Mech(X|Y|Z|F[XYZ]|Last[XY]|Elev|Terrain|Type|Move|Speed|VerticalSpeed|Specials2|Carrying|Pilot)|GetPartType|PartIsDestroyed|IsDS|Landed|Started|Uncon|Jumping|Fallen|OODing|is_aero|FlyingT|DSBearMod|MirrorPosition)\(' \
  src/btech/movement/ds_bay.c || true)

while IFS= read -r match; do
  echo "$match: legacy DropShip bay export is not allowed"
  status=1
done < <(rg -n \
  '\b(Find_DS_Bay_Number|Find_DS_Bay_Dir|Find_DS_Bay_In_MechHex|Leave_DS|DS_Bay_Is_Open|DS_Bay_Is_EnterOK|DS_Place)\b' \
  src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: converted pickup module uses an aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|target|towMech|newmap)->|\b(Mech(X|Y|Z|F[XYZ]|Type|Move|Speed|VerticalSpeed|Specials2?|Carrying|SwarmTarget|Tons|Team|CritStatus|Status|TankCritStatus|DesiredSpeed)|Fortified|CarryingClub|Jumping|Fallen|Towed|Towable|Destroyed|Started|OODing|IsDS|SectIsDestroyed|OkayCritSectS|SetCarrying|MirrorPosition|FallCentersTorso)\(' \
  src/btech/movement/mech_pickup.c || true)

while IFS= read -r match; do
  echo "$match: converted out-of-door drop module uses an aggregate Mech layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|wounded|attacker)->|\b(Mech(Cocoon|Z|FZ|Elevation|JumpSpeed|Status|Type|Pilot|PilotSkillBase|DesiredSpeed|DesiredAngle|MaxSpeed|RTons|Specials2|X|Y)|GetSectOInt|OODing|Fallen|Uncon|Started|Blinded|WaterBeast|NotInWater|InWater|FlyingT|Digging|Evading|Sprinting)\(' \
  src/btech/movement/mech_ood.c || true)

while IFS= read -r match; do
  echo "$match: converted stance module uses an aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\bmech->|\b(Mech[A-Z][A-Za-z0-9_]*|Fortified|OODing|Fallen|Jumping|Started|WaterBeast|NotInWater|InWater|IsHulldown|SectIsDestroyed|PartIsNonfunctional)\(' \
  src/btech/movement/mech_stance.c || true)

while IFS= read -r match; do
  echo "$match: converted jump module uses an aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|tempMech|mech_map)->|\b(Mech(Type|Carrying|MaxSpeed|JumpSpeed|Status|Target|DFATarget|SwarmTarget|JumpTop|Cocoon|JumpHeading|StartF[XYZ]|JumpLength|Going[XY]|EndFZ|Speed|F[XYZ]|[XYZ])|Fortified|OODing|Fallen|Jumping|IsHulldown|Staggering)\(' \
  src/btech/movement/mech_jump.c || true)

while IFS= read -r match; do
  echo "$match: converted flooding module uses the aggregate Mech layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\bmech->|\b(Mech[A-Z][A-Za-z0-9_]*|GetSect[A-Za-z0-9_]*|SetSect[A-Za-z0-9_]*|SectIsFlooded|Fallen|InWater)\(' \
  src/btech/movement/mech_flooding.c || true)

while IFS= read -r match; do
  echo "$match: legacy flooding export is not allowed"
  status=1
done < <(rg -n '\bMechFloods(Loc)?\b' src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: converted landing module uses the aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|target|mech_map)->|\b(Mech(Type|Status|DFATarget|[FXYZ]|Elev|Elevation|Sections|CritStatus|Going[XY]|Speed)|Uncon|Staggering|Fallen|Jumping)\(' \
  src/btech/movement/mech_landing.c || true)

while IFS= read -r match; do
  echo "$match: legacy landing export is not allowed"
  status=1
done < <(rg -n '\b(LandMech|DropGetElevation|DropSetElevation)\b' \
  src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: legacy cargo-speed export is not allowed"
  status=1
done < <(rg -n '\bMechCargoMaxSpeed\b' src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: legacy out-of-door drop export is not allowed"
  status=1
done < <(rg -n '\binitiate_ood\b' src/btech -g '*.[ch]' || true)

while IFS= read -r match; do
  echo "$match: converted startup module uses an aggregate Mech or map layout"
  status=1
done < <(rg -n \
  '#include "(map|mech|mech_macros)\.h"|\b(mech|mech_map)->|\b(Mech(Type|Move|Specials2?|Z|Elevation|Pilot|VerticalSpeed|Status2?|CritStatus|Comm|Per|CommLast|LastStartup|DesiredAngle|DesiredSpeed|MaxSpeed|StartF[XYZ]|Heat|Sections|Speed)|InWater|FlyingT|Landed|Jumping|Started|Destroyed|Towed|IsDS|is_aero|UnSetMechPKiller)\(' \
  src/btech/movement/mech_startup.c || true)

while IFS= read -r match; do
  echo "$match: converted repair interface exposes the aggregate Mech layout"
  status=1
done < <(rg -n '#include "mech(_macros)?\.h"|\benum damage_type\b|\bMAX_DAMAGES\b' \
  src/btech/repair/mech_tech_damages.h src/btech/repair/mechrep.h || true)

while IFS= read -r match; do
  echo "$match: converted LOS tracer accesses BattleMap layout"
  status=1
done < <(rg -n -- '\bmap->' src/btech/sensors/los_trace.c || true)

while IFS= read -r match; do
  echo "$match: converted sensor module accesses BattleMap layout"
  status=1
done < <(rg -n -- '\bmap->' src/btech/sensors/mech_sensor_events.c \
  src/btech/sensors/mech_sensor_functions.c \
  src/btech/sensors/mech_sensor.c \
  src/btech/sensors/mech_sensor_selection.c || true)

while IFS= read -r match; do
  echo "$match: legacy sensor export is not allowed"
  status=1
done < <(rg -n \
  '\b(sendECMNotification|checkECM|isTAGDestroyed|stopTAG|checkTAG|mechSensorInfo|CanChangeTo|possibly_see_mech|ScrambleInfraAndLiteAmp|Sensor_ToHitBonus|Sensor_CanSee|Sensor_ArcBaseChance|Sensor_DriverBaseChance|Sensor_Sees|Sensor_SeesNow|Sensor_DoWeSeeNow|update_LOSinfo|add_sensor_info|ShowTurretFacing|PrintReport|PrintEnemyStatus|ActualElevation|CalculateLOSFlag|AddTerrainMod|InLineOfSight_NB|InLineOfSight|IsArtyMech|ClearFireAdjustments|FireSpot|getWeaponArc|getStatusChar|getFreeC3iNetworkPos|replicateC3iNetwork|addMechToC3iNetwork|clearMechFromC3iNetwork|clearC3iNetwork|validateC3iNetwork|getMechInTempNetwork|getOtherMechInNetwork|buildTempNetwork|sendNetworkMessage|showNetworkTargets|showNetworkData|mechSeenByNetwork|findC3RangeWithNetwork|findC3Range|debugC3|getC3MasterSize|isPartOfWorkingC3Master|countWorkingC3MastersOnMech|countTotalC3MastersOnMech|countMaxC3Units|trimC3Network|getFreeC3NetworkPos|replicateC3Network|addMechToC3Network|clearMechFromC3Network|clearC3Network|validateC3Network)\s*\(' \
  src/btech || true)

while IFS= read -r match; do
  echo "$match: converted map UI boundary accesses Mech layout directly"
  status=1
done < <(rg -n \
  -- '\b(mech|tempMech|tmpm|oMech)->(xcode|mynum|mapindex|mapnumber|brief|ID)\b' \
  src/btech/ui/mech_maps.c src/btech/ui/mech_base_entry.c \
  src/btech/ui/mech_lrs_map.c src/btech/ui/mech_tactical_command.c || true)

while IFS= read -r match; do
  echo "$match: converted scripting boundary accesses the Mech layout"
  status=1
done < <(rg -n -g '*.[ch]' \
  -- '#include "mech(_macros)?\.h"|\b(mech|mechA|mechB|omech|target)->' \
  src/btech/scripting || true)

while IFS= read -r match; do
  echo "$match: scripting registry fabricates a Mech field address"
  status=1
done < <(rg -n \
  'offsetof\(Mech|\b(MeEntry|MeVEntry|UglieV?)\b' \
  src/btech/scripting/registry_values.c || true)

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
  echo "$match: BTech domain reaches into stagger configuration directly"
  status=1
done < <(rg -n 'configuration->btech_newstagger(time|tons)?' \
  src/btech -g '*.[ch]' -g '!**/core/context.c' || true)

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
  '\b(MechPilot|MechPilotStatus|GunPilot|MechPer|MechLX|MechLY|MechX|MechY|MechTeam|MechType|MechMove|MechTons|MechXPMod|MechCritStatus|MechMaxSpeed|MechSpeed|MechDesiredSpeed|Destroyed|NoGunXP|SetSect[A-Za-z]*|SetPart[A-Za-z]*)\s*\(' \
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

if [[ -e src/btech/movement/mech_landing_falls.c ]]; then
  echo "src/btech/movement/mech_landing_falls.c: mixed landing/flooding/fall module is not allowed"
  status=1
fi

match=$(rg -n '\b(MechFalls|MechCocoon|MechStatus|MechStatus2|MechType|MechMove|MechSpeed|MechDesiredSpeed|MechVerticalSpeed|MechGoingY|MechStartFX|MechStartFY|MechStartFZ|MechFZ|MechZ|MechTons|MechRealTons|MechFacing|MechDesiredFacing|InWater|InSpecial|MapUnderSpecialRules|MapGravity)\b' \
  src/btech/movement/mech_falls.c || true)
if [[ -n "$match" ]]; then
  echo "$match: fall resolution must use opaque unit and map APIs"
  status=1
fi

match=$(rg -n '\bMechFalls\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy fall export is not allowed"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros)\.h"|mech->|\b(MechStatus|MechStatus2|MechCritStatus|MechTankCritStatus|MechType|MechMove|MechSpeed|MechDesiredSpeed|MechVerticalSpeed|MechFacing|MechDesiredFacing|MechTurretFacing|MechPilot|MechCarrying|MechSpecials|MechSwarmTarget|Fortified|Fallen|Landed|Jumping|WaterBeast|NotInWater|PerformingAction|MechDugIn|IsHulldown|Digging|Spinning|RollingT|FlyingT|AeroFuel|MMaxSpeed|WalkingSpeed|IsRunning)\s*\(' \
  src/btech/movement/mech_move_controls.c || true)
if [[ -n "$match" ]]; then
  echo "$match: movement controls must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|wounded->|\b(Mech[A-Z][A-Za-z0-9_]*|Started|Uncon|Jumping|Fallen|OODing|IsDS|is_aero)\s*\(' \
  src/btech/movement/mech_update.c \
  src/btech/movement/mech_update_damage.c \
  src/btech/movement/mech_update_heartbeat.c || true)
if [[ -n "$match" ]]; then
  echo "$match: update coordinators must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\bCheckDamage\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy damage-update export is not allowed"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|InWater|Landed|is_aero)\s*\(' \
  src/btech/movement/mech_update_altitude.c || true)
if [[ -n "$match" ]]; then
  echo "$match: altitude handling must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\b(CheckNavalHeight|CheckVTOLHeight)\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy altitude exports are not allowed"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Started|Uncon|Blinded|Jumping|Fallen|OODing|InSpecial|InGravity|IsRunning|SectIsDestroyed)\s*\(' \
  src/btech/movement/mech_update_piloting.c || true)
if [[ -n "$match" ]]; then
  echo "$match: piloting updates must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\b(UpdatePilotSkillRolls|updateAutoturnTurret)\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy piloting update exports are not allowed"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Started|Destroyed|SectIsDestroyed|WpnIsRecycling|GetPartData|GetPartFireMode|PartTempNuke)\s*\(' \
  src/btech/movement/mech_update_recycle.c || true)
if [[ -n "$match" ]]; then
  echo "$match: weapon recycling must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\b(recycle_weaponry|SkidMod|move_unit_back)\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy recycle, skid, or rollback exports are not allowed"
  status=1
fi

match=$(rg -n '\b(move_mech|NewHexEntered)\s*\(' src/btech | rg -v '"' || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy movement coordinator exports are not allowed"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Jumping|IsDS|is_aero|mark_for_los_update|GetTurnMode)\s*\(' \
  src/btech/movement/mech_update_motion.c || true)
if [[ -n "$match" ]]; then
  echo "$match: heading integration must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Fallen|Jumping|Landed|Sprinting|Evading|GetTurnMode|MMaxSpeed)\s*\(' \
  src/btech/movement/mech_update_speed.c || true)
if [[ -n "$match" ]]; then
  echo "$match: speed integration must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\b(UpdateHeading|UpdateSpeed|terrain_speed)\s*\(' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy heading or speed exports are not allowed"
  status=1
fi

match=$(rg -n '#include "mech_update_internal\.h"|#include "(mech|mech_macros)\.h"|mech->|\bMechHeat\s*\(' \
  src/btech/movement/mech_overheat_modifier.c || true)
if [[ -n "$match" ]]; then
  echo "$match: overheat modifiers must use opaque unit APIs"
  status=1
fi

match=$(rg -n '\b(OverheatMods|HandleOverheat|UpdateHeat|ammo_explosion)\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy heat exports are not allowed"
  status=1
fi

match=$(rg -n '\bfiery_death\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy fire hazard export is not allowed"
  status=1
fi

match=$(rg -n '\bDSOkToNotify\b' src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy DropShip notification export is not allowed"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\bMech[A-Z][A-Za-z0-9_]*\s*\(' \
  src/btech/movement/dropship_notification.c || true)
if [[ -n "$match" ]]; then
  echo "$match: DropShip notification throttling must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Overwater|MoveMod|Elevation)\s*\(' \
  src/btech/movement/mech_collision.c || true)
if [[ -n "$match" ]]; then
  echo "$match: collision evaluation must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|FaMechRange)\s*\(' \
  src/btech/movement/mech_charge_tracking.c || true)
if [[ -n "$match" ]]; then
  echo "$match: charge tracking must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\bMech[A-Z][A-Za-z0-9_]*\s*\(' \
  src/btech/movement/mech_towing_sync.c || true)
if [[ -n "$match" ]]; then
  echo "$match: towing synchronization must use opaque unit APIs"
  status=1
fi

if (( $(wc -l < src/btech/movement/mech_motion_integration.c) > 300 )); then
  echo "movement motion integration exceeds its 300-line responsibility limit"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Jumping|Landed|IsDS|MAPMOVEMOD|JumpSpeed)\s*\(' \
  src/btech/movement/mech_motion_integration.c || true)
if [[ -n "$match" ]]; then
  echo "$match: motion integration must use opaque unit and map APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Landed|IsForest|Elevation)\s*\(' \
  src/btech/movement/mech_update_hex.c || true)
if [[ -n "$match" ]]; then
  echo "$match: hex transition dispatch must use opaque unit and map APIs"
  status=1
fi

if [[ -e src/btech/movement/mech_update_internal.h ]]; then
  echo "legacy movement update aggregate header is not allowed"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|InSpecial|InGravity|InWater|Fallen|Jumping|Destroyed|OODing|MMaxSpeed|WalkingSpeed|MapGravity)\s*\(' \
  src/btech/movement/mech_move.c || true)
if [[ -n "$match" ]]; then
  echo "$match: ground movement must use opaque unit and map APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|map)\.h"|(mech|map)->|\b(Mech[A-Z][A-Za-z0-9_]*|IsDS|Landed|Started|Destroyed|Fallen|FlyingT|RollingT|SpheroidDS|Spinning|StopSpinning|StartSpinning|SetFacing|AeroFuel|MMaxSpeed|is_aero)\s*\(' \
  src/btech/movement/aero_move.c || true)
if [[ -n "$match" ]]; then
  echo "$match: aerospace movement must use opaque unit and map APIs"
  status=1
fi

match=$(rg -n '\b(aero_ControlEffect|ds_BridgeHit|aero_UpdateHeading|aero_UpdateSpeed|FuelCheck|ImproperLZ|DS_LandWarning)\b' \
  src/btech || true)
if [[ -n "$match" ]]; then
  echo "$match: legacy aerospace exports are not allowed"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Jumping)\s*\(' \
  src/btech/movement/mech_movement_validation.c || true)
if [[ -n "$match" ]]; then
  echo "$match: movement validation must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Destroyed|is_aero)\s*\(' \
  src/btech/movement/mech_fire_hazard.c || true)
if [[ -n "$match" ]]; then
  echo "$match: movement fire hazards must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(MechType|MechPilot|MechWeapHeat|GetPartAmmoMode|DestroyPart)\s*\(' \
  src/btech/combat/mech_ammunition_explosion.c || true)
if [[ -n "$match" ]]; then
  echo "$match: ammunition explosions must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Started|Jumping|OODing|Landed|Fallen|is_aero|JumpSpeedMP)\s*\(' \
  src/btech/movement/mech_overheat.c || true)
if [[ -n "$match" ]]; then
  echo "$match: overheat hazards must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Started|Jumping|Fallen|Jellied|InWater|InSpecial|Heatcutoff|MMaxSpeed|IsRunning)\s*\(' \
  src/btech/movement/mech_update_heat.c || true)
if [[ -n "$match" ]]; then
  echo "$match: heat accounting must use opaque unit APIs"
  status=1
fi

match=$(rg -n '#include "(mech|mech_macros|mech_update_internal)\.h"|mech->|\b(Mech[A-Z][A-Za-z0-9_]*|Jumping|MMaxSpeed|IsRunning|WalkingSpeed)\s*\(' \
  src/btech/movement/mech_update_hex_mech.c || true)
if [[ -n "$match" ]]; then
  echo "$match: mech hex transitions must use opaque unit APIs"
  status=1
fi

if [[ -e src/btech/sensors/mech_sensor_internal.h ]]; then
  echo "src/btech/sensors/mech_sensor_internal.h: aggregate sensor header is not allowed"
  status=1
fi

if [[ -e src/btech/sensors/mech_scan_internal.h ]]; then
  echo "src/btech/sensors/mech_scan_internal.h: aggregate scan header is not allowed"
  status=1
fi

if find src/btech/src -type f -print -quit 2>/dev/null | grep -q .; then
  echo "src/btech/src: legacy nested source tree is not allowed"
  status=1
fi

if rg -n '\btprintf\b|\bformatted_[0-9]+\b|\(\(void\)snprintf\(' src \
    --glob '*.{c,h}'; then
  echo "src: shared or generated formatting compatibility pattern is not allowed"
  status=1
fi

header_compiler=${BTECH_HEADER_COMPILER:-clang-22}
header_jobs=${BTECH_HEADER_JOBS:-$(nproc)}
if [[ ! $header_jobs =~ ^[1-9][0-9]*$ ]]; then
  echo "BTECH_HEADER_JOBS must be a positive integer" >&2
  exit 2
fi
header_flags=(
  -DBTECH_PERSISTENCE_TESTING=1
  -std=gnu23
  -Wall
  -Wextra
  -Wno-unused-parameter
  -Werror
  -Wno-gcc-install-dir-libstdcxx
  -Wformat=2
  -Wshadow
  "-I$root/src/btech/include"
  "-I$build_root/src/btech"
  "-I$build_root"
  "-I$root/src"
)

for domain in autopilot character combat commands core economy integration map \
              movement persistence repair scripting sensors special ui unit; do
  header_flags+=("-I$root/src/btech/$domain")
done

scan_finished=$(date +%s%N)
header_log_root=$(mktemp -d)
header_index=0
running_jobs=0
while IFS= read -r -d '' header; do
  header_index=$((header_index + 1))
  log_file=$(printf '%s/%06d.log' "$header_log_root" "$header_index")
  failure_file=$(printf '%s/%06d.failed' "$header_log_root" "$header_index")
  (
    if ! "$header_compiler" "${header_flags[@]}" -fsyntax-only -x c-header \
         "$header" >"$log_file" 2>&1; then
      printf '%s\n' "$header" >"$failure_file"
    fi
  ) &
  running_jobs=$((running_jobs + 1))
  if (( running_jobs >= header_jobs )); then
    wait -n
    running_jobs=$((running_jobs - 1))
  fi
done < <(find src/btech -type f -name '*.h' -print0 | sort -z)
wait

for ((header_number = 1; header_number <= header_index; header_number++)); do
  log_file=$(printf '%s/%06d.log' "$header_log_root" "$header_number")
  failure_file=$(printf '%s/%06d.failed' "$header_log_root" "$header_number")
  if [[ -e $failure_file ]]; then
    cat "$log_file"
    echo "$(<"$failure_file"): header does not compile independently"
    status=1
  fi
done

if [[ ${BTECH_ARCHITECTURE_PROFILE:-0} == 1 ]]; then
  architecture_finished=$(date +%s%N)
  echo "architecture profile: scans=$(((scan_finished - architecture_started) / 1000000))ms headers=$(((architecture_finished - scan_finished) / 1000000))ms jobs=$header_jobs" >&2
fi

exit "$status"
