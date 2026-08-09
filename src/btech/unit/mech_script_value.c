#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_identity_api.h"
#include "mech_network_api.h"
#include "mech_script_value_api.h"
#include "mech_targeting_api.h"

#include <string.h>

#include "checked_conversion.h"
#include "mech_internal.h"
#include "mech_stagger.h"

bool mech_script_value_read(const Mech *mech, MechScriptValueKey key,
                            MechScriptValue *value) {
  if (!mech || !value)
    return false;

  switch (key) {
  case MECH_SCRIPT_MAP_DBREF:
    value->dbref = mech->mapindex;
    return true;
  case MECH_SCRIPT_NAME:
    value->string = ((mech)->ud.mech_name);
    return true;
  case MECH_SCRIPT_MAXIMUM_SPEED:
    value->floating = ((mech)->ud.maxspeed);
    return true;
  case MECH_SCRIPT_ERA:
    value->string = ((mech)->ud.unit_era);
    return true;
  case MECH_SCRIPT_TRO:
    value->string = ((mech)->ud.unit_tro);
    return true;
  case MECH_SCRIPT_TEMPLATE_SPEED:
    value->floating = ((mech)->ud.template_maxspeed);
    return true;
  case MECH_SCRIPT_PILOT_DBREF:
    value->dbref = mech_pilot_dbref(mech);
    return true;
  case MECH_SCRIPT_EXPERIENCE_MODIFIER:
    value->floating = ((mech)->rd.xpmod);
    return true;
  case MECH_SCRIPT_PILOT_DAMAGE:
    value->integer = mech_pilot_status(mech);
    return true;
  case MECH_SCRIPT_STRUCTURAL_INTEGRITY:
    value->integer = ((mech)->ud.si);
    return true;
  case MECH_SCRIPT_ORIGINAL_STRUCTURAL_INTEGRITY:
    value->integer = ((mech)->ud.si_orig);
    return true;
  case MECH_SCRIPT_SPEED:
    value->floating = ((mech)->rd.speed);
    return true;
  case MECH_SCRIPT_BASE_WALK_SPEED:
    value->integer = ((mech)->ud.walkspeed);
    return true;
  case MECH_SCRIPT_BASE_RUN_SPEED:
    value->integer = ((mech)->ud.runspeed);
    return true;
  case MECH_SCRIPT_HEADING:
    value->integer = ((mech)->pd.facing);
    return true;
  case MECH_SCRIPT_STALL:
    value->integer = ((mech)->pd.stall);
    return true;
  case MECH_SCRIPT_STATUS:
    value->integer = ((mech)->rd.status);
    return true;
  case MECH_SCRIPT_STATUS_SECONDARY:
    value->integer = ((mech)->rd.status2);
    return true;
  case MECH_SCRIPT_CRITICAL_STATUS:
    value->integer = ((mech)->rd.critstatus);
    return true;
  case MECH_SCRIPT_CRITICAL_STATUS_SECONDARY:
    value->integer = ((mech)->rd.critstatus2);
    return true;
  case MECH_SCRIPT_VEHICLE_CRITICAL_STATUS:
    value->integer = ((mech)->rd.tankcritstatus);
    return true;
  case MECH_SCRIPT_TARGET_DBREF:
    value->dbref = mech_target_dbref(mech);
    return true;
  case MECH_SCRIPT_TEAM:
    value->integer = ((mech)->pd.team);
    return true;
  case MECH_SCRIPT_TONNAGE:
    value->integer = ((mech)->ud.tons);
    return true;
  case MECH_SCRIPT_TOWING_DBREF:
    value->dbref = ((mech)->rd.carrying);
    return true;
  case MECH_SCRIPT_HEAT_PRODUCTION:
    value->floating = ((mech)->rd.plus_heat);
    return true;
  case MECH_SCRIPT_DISABLED_HEAT_SINKS:
    value->integer = ((mech)->rd.disabled_hs);
    return true;
  case MECH_SCRIPT_HEAT:
    value->floating = ((mech)->rd.heat);
    return true;
  case MECH_SCRIPT_HEAT_DISSIPATION:
    value->floating = ((mech)->rd.minus_heat);
    return true;
  case MECH_SCRIPT_ENGINE_HEAT_SINK_OVERRIDE:
    value->integer = ((mech)->ud.hsengoverride);
    return true;
  case MECH_SCRIPT_HEAT_SINKS:
    value->integer = ((mech)->ud.numsinks);
    return true;
  case MECH_SCRIPT_LAST_STARTUP:
    value->integer = ((mech)->rd.last_startup);
    return true;
  case MECH_SCRIPT_C3I_NETWORK_SIZE:
    value->integer = mech_c3i_network_size(mech);
    return true;
  case MECH_SCRIPT_MAXIMUM_BATTLE_SUITS:
    value->integer = ((mech)->rd.maxsuits);
    return true;
  case MECH_SCRIPT_REAL_WEIGHT:
    value->integer = ((mech)->rd.row);
    return true;
  case MECH_SCRIPT_STAGGER_DAMAGE:
    value->integer = mech_stagger_damage_total(mech);
    return true;
  case MECH_SCRIPT_PREFERENCES:
    value->integer = ((mech)->rd.mech_prefs);
    return true;
  case MECH_SCRIPT_SWARM_TARGET:
    value->dbref = ((mech)->rd.swarming);
    return true;
  case MECH_SCRIPT_SWARMER:
    value->dbref = ((mech)->rd.swarmedby);
    return true;
  case MECH_SCRIPT_FUEL:
    value->integer = ((mech)->ud.fuel);
    return true;
  case MECH_SCRIPT_ORIGINAL_FUEL:
    value->integer = ((mech)->ud.fuel_orig);
    return true;
  case MECH_SCRIPT_COCOON:
    value->integer = ((mech)->rd.cocoon);
    return true;
  case MECH_SCRIPT_SEEN_COUNT:
    value->integer = ((mech)->rd.num_seen);
    return true;
  case MECH_SCRIPT_REAL_X:
    value->floating = ((mech)->pd.fx);
    return true;
  case MECH_SCRIPT_REAL_Y:
    value->floating = ((mech)->pd.fy);
    return true;
  case MECH_SCRIPT_REAL_Z:
    value->floating = ((mech)->pd.fz);
    return true;
  case MECH_SCRIPT_X:
    value->integer = ((mech)->pd.x);
    return true;
  case MECH_SCRIPT_Y:
    value->integer = ((mech)->pd.y);
    return true;
  case MECH_SCRIPT_Z:
    value->integer = ((mech)->pd.z);
    return true;
  case MECH_SCRIPT_TARGETING_COMPUTER:
    value->integer = mech_targeting_computer_type(mech);
    return true;
  case MECH_SCRIPT_LONG_RANGE_SENSOR_RANGE:
    value->integer = mech_long_range_sensor_range(mech);
    return true;
  case MECH_SCRIPT_RADIO_RANGE:
    value->integer = mech_radio_range(mech);
    return true;
  case MECH_SCRIPT_SCAN_RANGE:
    value->integer = mech_scanner_range(mech);
    return true;
  case MECH_SCRIPT_TACTICAL_SENSOR_RANGE:
    value->integer = mech_tactical_range(mech);
    return true;
  case MECH_SCRIPT_RADIO_TYPE:
    value->integer = mech_radio_configuration(mech);
    return true;
  case MECH_SCRIPT_BATTLE_VALUE:
    value->integer = ((mech)->ud.mechbv);
    return true;
  case MECH_SCRIPT_CARGO_SPACE:
    value->integer = ((mech)->ud.cargospace);
    return true;
  case MECH_SCRIPT_CARRIER_MAXIMUM_TONNAGE:
    value->integer = ((mech)->ud.carmaxton);
    return true;
  case MECH_SCRIPT_BAY_0:
    value->dbref = ((mech)->pd.bay[0]);
    return true;
  case MECH_SCRIPT_BAY_1:
    value->dbref = ((mech)->pd.bay[1]);
    return true;
  case MECH_SCRIPT_BAY_2:
    value->dbref = ((mech)->pd.bay[2]);
    return true;
  case MECH_SCRIPT_BAY_3:
    value->dbref = ((mech)->pd.bay[3]);
    return true;
  case MECH_SCRIPT_TURRET_0:
    value->dbref = ((mech)->pd.turret[0]);
    return true;
  case MECH_SCRIPT_TURRET_1:
    value->dbref = ((mech)->pd.turret[1]);
    return true;
  case MECH_SCRIPT_TURRET_2:
    value->dbref = ((mech)->pd.turret[2]);
    return true;
  case MECH_SCRIPT_UNUSABLE_ARCS:
    value->integer = ((mech)->pd.unusable_arcs);
    return true;
  case MECH_SCRIPT_MAXIMUM_JUMP_SPEED:
    value->floating = ((mech)->rd.jumpspeed);
    return true;
  case MECH_SCRIPT_JUMP_HEADING:
    value->integer = ((mech)->rd.jumpheading);
    return true;
  case MECH_SCRIPT_JUMP_LENGTH:
    value->integer = ((mech)->rd.jumplength);
    return true;
  case MECH_SCRIPT_RADIO:
    value->integer = mech_radio_quality(mech);
    return true;
  case MECH_SCRIPT_COMPUTER:
    value->integer = mech_computer_quality(mech);
    return true;
  case MECH_SCRIPT_PERCEPTION:
    value->integer = mech_perception_target(mech);
    return true;
  case MECH_SCRIPT_SHOTS_FIRED:
    value->integer = ((mech)->rd.shots_fired);
    return true;
  case MECH_SCRIPT_SHOTS_MISSED:
    value->integer = ((mech)->rd.shots_missed);
    return true;
  case MECH_SCRIPT_SHOTS_HIT:
    value->integer = ((mech)->rd.shots_hit);
    return true;
  case MECH_SCRIPT_DAMAGE_TAKEN:
    value->integer = ((mech)->rd.damage_taken);
    return true;
  case MECH_SCRIPT_DAMAGE_INFLICTED:
    value->integer = ((mech)->rd.damage_inflicted);
    return true;
  case MECH_SCRIPT_UNITS_KILLED:
    value->integer = ((mech)->rd.units_killed);
    return true;
  case MECH_SCRIPT_HEXES_WALKED:
    value->floating = ((mech)->pd.hexes_walked);
    return true;
  }

  return false;
}

bool mech_script_value_write(Mech *mech, MechScriptValueKey key,
                             MechScriptValue value) {
  if (!mech)
    return false;

  switch (key) {
  case MECH_SCRIPT_MAP_DBREF:
    mech_map_dbref_set(mech, value.dbref);
    return true;
  case MECH_SCRIPT_NAME:
    strncpy(((mech)->ud.mech_name), value.string, 31 - 1);
    ((mech)->ud.mech_name)[31 - 1] = '\0';
    return true;
  case MECH_SCRIPT_MAXIMUM_SPEED:
    ((mech)->ud.maxspeed) = value.floating;
    return true;
  case MECH_SCRIPT_ERA:
    strncpy(((mech)->ud.unit_era), value.string, 25 - 1);
    ((mech)->ud.unit_era)[25 - 1] = '\0';
    return true;
  case MECH_SCRIPT_TRO:
    strncpy(((mech)->ud.unit_tro), value.string, 25 - 1);
    ((mech)->ud.unit_tro)[25 - 1] = '\0';
    return true;
  case MECH_SCRIPT_TEMPLATE_SPEED:
    ((mech)->ud.template_maxspeed) = value.floating;
    return true;
  case MECH_SCRIPT_PILOT_DBREF:
    mech_pilot_dbref_set(mech, value.dbref);
    return true;
  case MECH_SCRIPT_EXPERIENCE_MODIFIER:
    ((mech)->rd.xpmod) = value.floating;
    return true;
  case MECH_SCRIPT_PILOT_DAMAGE:
    mech_pilot_status_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_STRUCTURAL_INTEGRITY:
    ((mech)->ud.si) = clamp_int_to_char(value.integer);
    return true;
  case MECH_SCRIPT_ORIGINAL_STRUCTURAL_INTEGRITY:
    ((mech)->ud.si_orig) = clamp_int_to_char(value.integer);
    return true;
  case MECH_SCRIPT_SPEED:
    ((mech)->rd.speed) = value.floating;
    return true;
  case MECH_SCRIPT_BASE_WALK_SPEED:
    ((mech)->ud.walkspeed) = value.integer;
    return true;
  case MECH_SCRIPT_BASE_RUN_SPEED:
    ((mech)->ud.runspeed) = value.integer;
    return true;
  case MECH_SCRIPT_HEADING:
    ((mech)->pd.facing) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_STALL:
    ((mech)->pd.stall) = value.integer;
    return true;
  case MECH_SCRIPT_STATUS:
    ((mech)->rd.status) = value.integer;
    return true;
  case MECH_SCRIPT_STATUS_SECONDARY:
    ((mech)->rd.status2) = value.integer;
    return true;
  case MECH_SCRIPT_CRITICAL_STATUS:
    ((mech)->rd.critstatus) = value.integer;
    return true;
  case MECH_SCRIPT_CRITICAL_STATUS_SECONDARY:
    ((mech)->rd.critstatus2) = value.integer;
    return true;
  case MECH_SCRIPT_VEHICLE_CRITICAL_STATUS:
    ((mech)->rd.tankcritstatus) = value.integer;
    return true;
  case MECH_SCRIPT_TARGET_DBREF:
    mech_target_dbref_set(mech, value.dbref);
    return true;
  case MECH_SCRIPT_TOWING_DBREF:
    mech->rd.carrying = value.dbref;
    return true;
  case MECH_SCRIPT_TEAM:
    ((mech)->pd.team) = value.integer;
    return true;
  case MECH_SCRIPT_TONNAGE:
    ((mech)->ud.tons) = value.integer;
    return true;
  case MECH_SCRIPT_HEAT_PRODUCTION:
    ((mech)->rd.plus_heat) = value.floating;
    return true;
  case MECH_SCRIPT_HEAT:
    ((mech)->rd.heat) = value.floating;
    return true;
  case MECH_SCRIPT_HEAT_DISSIPATION:
    ((mech)->rd.minus_heat) = value.floating;
    return true;
  case MECH_SCRIPT_ENGINE_HEAT_SINK_OVERRIDE:
    ((mech)->ud.hsengoverride) = value.integer;
    return true;
  case MECH_SCRIPT_LAST_STARTUP:
    ((mech)->rd.last_startup) = value.integer;
    return true;
  case MECH_SCRIPT_MAXIMUM_BATTLE_SUITS:
    ((mech)->rd.maxsuits) = value.integer;
    return true;
  case MECH_SCRIPT_REAL_WEIGHT:
    ((mech)->rd.row) = value.integer;
    return true;
  case MECH_SCRIPT_PREFERENCES:
    ((mech)->rd.mech_prefs) = value.integer;
    return true;
  case MECH_SCRIPT_SWARM_TARGET:
    ((mech)->rd.swarming) = value.dbref;
    return true;
  case MECH_SCRIPT_SWARMER:
    ((mech)->rd.swarmedby) = value.dbref;
    return true;
  case MECH_SCRIPT_FUEL:
    ((mech)->ud.fuel) = value.integer;
    return true;
  case MECH_SCRIPT_ORIGINAL_FUEL:
    ((mech)->ud.fuel_orig) = value.integer;
    return true;
  case MECH_SCRIPT_SEEN_COUNT:
    ((mech)->rd.num_seen) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_REAL_X:
    ((mech)->pd.fx) = value.floating;
    return true;
  case MECH_SCRIPT_REAL_Y:
    ((mech)->pd.fy) = value.floating;
    return true;
  case MECH_SCRIPT_REAL_Z:
    ((mech)->pd.fz) = value.floating;
    return true;
  case MECH_SCRIPT_X:
    ((mech)->pd.x) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_Y:
    ((mech)->pd.y) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_Z:
    ((mech)->pd.z) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_TARGETING_COMPUTER:
    mech_targeting_computer_type_set(mech,
                                     (TargetingComputerType)value.integer);
    return true;
  case MECH_SCRIPT_LONG_RANGE_SENSOR_RANGE:
    mech_long_range_sensor_range_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_RADIO_RANGE:
    mech_radio_range_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_SCAN_RANGE:
    mech_scanner_range_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_TACTICAL_SENSOR_RANGE:
    mech_tactical_range_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_RADIO_TYPE:
    mech_radio_configuration_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_BATTLE_VALUE:
    ((mech)->ud.mechbv) = value.integer;
    return true;
  case MECH_SCRIPT_CARGO_SPACE:
    ((mech)->ud.cargospace) = value.integer;
    return true;
  case MECH_SCRIPT_BAY_0:
    ((mech)->pd.bay[0]) = value.dbref;
    return true;
  case MECH_SCRIPT_BAY_1:
    ((mech)->pd.bay[1]) = value.dbref;
    return true;
  case MECH_SCRIPT_BAY_2:
    ((mech)->pd.bay[2]) = value.dbref;
    return true;
  case MECH_SCRIPT_BAY_3:
    ((mech)->pd.bay[3]) = value.dbref;
    return true;
  case MECH_SCRIPT_TURRET_0:
    ((mech)->pd.turret[0]) = value.dbref;
    return true;
  case MECH_SCRIPT_TURRET_1:
    ((mech)->pd.turret[1]) = value.dbref;
    return true;
  case MECH_SCRIPT_TURRET_2:
    ((mech)->pd.turret[2]) = value.dbref;
    return true;
  case MECH_SCRIPT_MAXIMUM_JUMP_SPEED:
    ((mech)->rd.jumpspeed) = value.floating;
    return true;
  case MECH_SCRIPT_JUMP_HEADING:
    ((mech)->rd.jumpheading) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_JUMP_LENGTH:
    ((mech)->rd.jumplength) = clamp_int_to_short(value.integer);
    return true;
  case MECH_SCRIPT_RADIO:
    mech_radio_quality_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_COMPUTER:
    mech_computer_quality_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_PERCEPTION:
    mech_perception_target_set(mech, value.integer);
    return true;
  case MECH_SCRIPT_SHOTS_FIRED:
    ((mech)->rd.shots_fired) = value.integer;
    return true;
  case MECH_SCRIPT_SHOTS_MISSED:
    ((mech)->rd.shots_missed) = value.integer;
    return true;
  case MECH_SCRIPT_SHOTS_HIT:
    ((mech)->rd.shots_hit) = value.integer;
    return true;
  case MECH_SCRIPT_DAMAGE_TAKEN:
    ((mech)->rd.damage_taken) = value.integer;
    return true;
  case MECH_SCRIPT_DAMAGE_INFLICTED:
    ((mech)->rd.damage_inflicted) = value.integer;
    return true;
  case MECH_SCRIPT_UNITS_KILLED:
    ((mech)->rd.units_killed) = value.integer;
    return true;
  case MECH_SCRIPT_HEXES_WALKED:
    ((mech)->pd.hexes_walked) = value.floating;
    return true;
  case MECH_SCRIPT_DISABLED_HEAT_SINKS:
  case MECH_SCRIPT_HEAT_SINKS:
  case MECH_SCRIPT_C3I_NETWORK_SIZE:
  case MECH_SCRIPT_STAGGER_DAMAGE:
  case MECH_SCRIPT_COCOON:
  case MECH_SCRIPT_CARRIER_MAXIMUM_TONNAGE:
  case MECH_SCRIPT_UNUSABLE_ARCS:
    return false;
  default:
    return false;
  }
}
