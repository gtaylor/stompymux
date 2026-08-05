#include "mech_script_value_api.h"

#include <string.h>

#include "mech_internal.h"
#include "mech_macros.h"

bool mech_script_value_read(const Mech *mech, MechScriptValueKey key,
                            MechScriptValue *value) {
  if (!mech || !value)
    return false;

#define READ_INTEGER(name, expression)                                         \
  case name:                                                                   \
    value->integer = (expression);                                             \
    return true
#define READ_FLOAT(name, expression)                                           \
  case name:                                                                   \
    value->floating = (expression);                                            \
    return true
#define READ_DBREF(name, expression)                                           \
  case name:                                                                   \
    value->dbref = (expression);                                               \
    return true
#define READ_STRING(name, expression)                                          \
  case name:                                                                   \
    value->string = (expression);                                              \
    return true

  switch (key) {
    READ_DBREF(MECH_SCRIPT_MAP_DBREF, mech->mapindex);
    READ_STRING(MECH_SCRIPT_NAME, MechType_Name(mech));
    READ_FLOAT(MECH_SCRIPT_MAXIMUM_SPEED, MechMaxSpeed(mech));
    READ_STRING(MECH_SCRIPT_ERA, MechUnitEra(mech));
    READ_STRING(MECH_SCRIPT_TRO, MechUnitTRO(mech));
    READ_FLOAT(MECH_SCRIPT_TEMPLATE_SPEED, TemplateMaxSpeed(mech));
    READ_DBREF(MECH_SCRIPT_PILOT_DBREF, MechPilot(mech));
    READ_FLOAT(MECH_SCRIPT_EXPERIENCE_MODIFIER, MechXPMod(mech));
    READ_INTEGER(MECH_SCRIPT_PILOT_DAMAGE, MechPilotStatus(mech));
    READ_INTEGER(MECH_SCRIPT_STRUCTURAL_INTEGRITY, AeroSI(mech));
    READ_INTEGER(MECH_SCRIPT_ORIGINAL_STRUCTURAL_INTEGRITY, AeroSIOrig(mech));
    READ_FLOAT(MECH_SCRIPT_SPEED, MechSpeed(mech));
    READ_INTEGER(MECH_SCRIPT_BASE_WALK_SPEED, MechBaseWalk(mech));
    READ_INTEGER(MECH_SCRIPT_BASE_RUN_SPEED, MechBaseRun(mech));
    READ_INTEGER(MECH_SCRIPT_HEADING, MechRFacing(mech));
    READ_INTEGER(MECH_SCRIPT_STALL, MechStall(mech));
    READ_INTEGER(MECH_SCRIPT_STATUS, MechStatus(mech));
    READ_INTEGER(MECH_SCRIPT_STATUS_SECONDARY, MechStatus2(mech));
    READ_INTEGER(MECH_SCRIPT_CRITICAL_STATUS, MechCritStatus(mech));
    READ_INTEGER(MECH_SCRIPT_CRITICAL_STATUS_SECONDARY, MechCritStatus2(mech));
    READ_INTEGER(MECH_SCRIPT_VEHICLE_CRITICAL_STATUS, MechTankCritStatus(mech));
    READ_DBREF(MECH_SCRIPT_TARGET_DBREF, MechTarget(mech));
    READ_INTEGER(MECH_SCRIPT_TEAM, MechTeam(mech));
    READ_INTEGER(MECH_SCRIPT_TONNAGE, MechTons(mech));
    READ_INTEGER(MECH_SCRIPT_TOWING_DBREF, MechCarrying(mech));
    READ_FLOAT(MECH_SCRIPT_HEAT_PRODUCTION, MechPlusHeat(mech));
    READ_INTEGER(MECH_SCRIPT_DISABLED_HEAT_SINKS, MechDisabledHS(mech));
    READ_FLOAT(MECH_SCRIPT_HEAT, MechHeat(mech));
    READ_FLOAT(MECH_SCRIPT_HEAT_DISSIPATION, MechMinusHeat(mech));
    READ_INTEGER(MECH_SCRIPT_ENGINE_HEAT_SINK_OVERRIDE,
                 MechHSEngOverRide(mech));
    READ_INTEGER(MECH_SCRIPT_HEAT_SINKS, MechRealNumsinks(mech));
    READ_INTEGER(MECH_SCRIPT_LAST_STARTUP, MechLastStartup(mech));
    READ_INTEGER(MECH_SCRIPT_C3I_NETWORK_SIZE, MechC3iNetworkSize(mech));
    READ_INTEGER(MECH_SCRIPT_MAXIMUM_BATTLE_SUITS, MechMaxSuits(mech));
    READ_INTEGER(MECH_SCRIPT_REAL_WEIGHT, MechRTonsV(mech));
    READ_INTEGER(MECH_SCRIPT_STAGGER_DAMAGE, StaggerDamage(mech));
    READ_INTEGER(MECH_SCRIPT_PREFERENCES, MechPrefs(mech));
    READ_DBREF(MECH_SCRIPT_SWARM_TARGET, MechSwarmTarget(mech));
    READ_DBREF(MECH_SCRIPT_SWARMER, MechSwarmer(mech));
    READ_INTEGER(MECH_SCRIPT_FUEL, AeroFuel(mech));
    READ_INTEGER(MECH_SCRIPT_ORIGINAL_FUEL, AeroFuelOrig(mech));
    READ_INTEGER(MECH_SCRIPT_COCOON, MechCocoon(mech));
    READ_INTEGER(MECH_SCRIPT_SEEN_COUNT, MechNumSeen(mech));
    READ_FLOAT(MECH_SCRIPT_REAL_X, MechFX(mech));
    READ_FLOAT(MECH_SCRIPT_REAL_Y, MechFY(mech));
    READ_FLOAT(MECH_SCRIPT_REAL_Z, MechFZ(mech));
    READ_INTEGER(MECH_SCRIPT_X, MechX(mech));
    READ_INTEGER(MECH_SCRIPT_Y, MechY(mech));
    READ_INTEGER(MECH_SCRIPT_Z, MechZ(mech));
    READ_INTEGER(MECH_SCRIPT_ELEVATION, MechElev(mech));
    READ_INTEGER(MECH_SCRIPT_TARGETING_COMPUTER, MechTargComp(mech));
    READ_INTEGER(MECH_SCRIPT_LONG_RANGE_SENSOR_RANGE, MechLRSRange(mech));
    READ_INTEGER(MECH_SCRIPT_RADIO_RANGE, MechRadioRange(mech));
    READ_INTEGER(MECH_SCRIPT_SCAN_RANGE, MechScanRange(mech));
    READ_INTEGER(MECH_SCRIPT_TACTICAL_SENSOR_RANGE, MechTacRange(mech));
    READ_INTEGER(MECH_SCRIPT_RADIO_TYPE, MechRadioType(mech));
    READ_INTEGER(MECH_SCRIPT_BATTLE_VALUE, MechBV(mech));
    READ_INTEGER(MECH_SCRIPT_CARGO_SPACE, CargoSpace(mech));
    READ_INTEGER(MECH_SCRIPT_CARRIER_MAXIMUM_TONNAGE, CarMaxTon(mech));
    READ_DBREF(MECH_SCRIPT_BAY_0, AeroBay(mech, 0));
    READ_DBREF(MECH_SCRIPT_BAY_1, AeroBay(mech, 1));
    READ_DBREF(MECH_SCRIPT_BAY_2, AeroBay(mech, 2));
    READ_DBREF(MECH_SCRIPT_BAY_3, AeroBay(mech, 3));
    READ_DBREF(MECH_SCRIPT_TURRET_0, AeroTurret(mech, 0));
    READ_DBREF(MECH_SCRIPT_TURRET_1, AeroTurret(mech, 1));
    READ_DBREF(MECH_SCRIPT_TURRET_2, AeroTurret(mech, 2));
    READ_INTEGER(MECH_SCRIPT_UNUSABLE_ARCS, AeroUnusableArcs(mech));
    READ_FLOAT(MECH_SCRIPT_MAXIMUM_JUMP_SPEED, MechJumpSpeed(mech));
    READ_INTEGER(MECH_SCRIPT_JUMP_HEADING, MechJumpHeading(mech));
    READ_INTEGER(MECH_SCRIPT_JUMP_LENGTH, MechJumpLength(mech));
    READ_INTEGER(MECH_SCRIPT_RADIO, MechRadio(mech));
    READ_INTEGER(MECH_SCRIPT_COMPUTER, MechComputer(mech));
    READ_INTEGER(MECH_SCRIPT_PERCEPTION, MechPer(mech));
    READ_INTEGER(MECH_SCRIPT_SHOTS_FIRED, MechShotsFired(mech));
    READ_INTEGER(MECH_SCRIPT_SHOTS_MISSED, MechShotsMissed(mech));
    READ_INTEGER(MECH_SCRIPT_SHOTS_HIT, MechShotsHit(mech));
    READ_INTEGER(MECH_SCRIPT_DAMAGE_TAKEN, MechDamageTaken(mech));
    READ_INTEGER(MECH_SCRIPT_DAMAGE_INFLICTED, MechDamageInflicted(mech));
    READ_INTEGER(MECH_SCRIPT_UNITS_KILLED, MechUnitsKilled(mech));
    READ_FLOAT(MECH_SCRIPT_HEXES_WALKED, MechHexes(mech));
  }

#undef READ_INTEGER
#undef READ_FLOAT
#undef READ_DBREF
#undef READ_STRING
  return false;
}

bool mech_script_value_write(Mech *mech, MechScriptValueKey key,
                             MechScriptValue value) {
  if (!mech)
    return false;

#define WRITE_INTEGER(name, expression)                                        \
  case name:                                                                   \
    (expression) = value.integer;                                              \
    return true
#define WRITE_FLOAT(name, expression)                                          \
  case name:                                                                   \
    (expression) = value.floating;                                             \
    return true
#define WRITE_DBREF(name, expression)                                          \
  case name:                                                                   \
    (expression) = value.dbref;                                                \
    return true
#define WRITE_STRING(name, expression, capacity)                               \
  case name:                                                                   \
    strncpy((expression), value.string, (capacity) - 1);                       \
    (expression)[(capacity) - 1] = '\0';                                       \
    return true

  switch (key) {
    WRITE_STRING(MECH_SCRIPT_NAME, MechType_Name(mech), 31);
    WRITE_FLOAT(MECH_SCRIPT_MAXIMUM_SPEED, MechMaxSpeed(mech));
    WRITE_STRING(MECH_SCRIPT_ERA, MechUnitEra(mech), 25);
    WRITE_STRING(MECH_SCRIPT_TRO, MechUnitTRO(mech), 25);
    WRITE_FLOAT(MECH_SCRIPT_TEMPLATE_SPEED, TemplateMaxSpeed(mech));
    WRITE_DBREF(MECH_SCRIPT_PILOT_DBREF, MechPilot(mech));
    WRITE_FLOAT(MECH_SCRIPT_EXPERIENCE_MODIFIER, MechXPMod(mech));
    WRITE_INTEGER(MECH_SCRIPT_PILOT_DAMAGE, MechPilotStatus(mech));
    WRITE_INTEGER(MECH_SCRIPT_STRUCTURAL_INTEGRITY, AeroSI(mech));
    WRITE_INTEGER(MECH_SCRIPT_ORIGINAL_STRUCTURAL_INTEGRITY, AeroSIOrig(mech));
    WRITE_FLOAT(MECH_SCRIPT_SPEED, MechSpeed(mech));
    WRITE_INTEGER(MECH_SCRIPT_BASE_WALK_SPEED, MechBaseWalk(mech));
    WRITE_INTEGER(MECH_SCRIPT_BASE_RUN_SPEED, MechBaseRun(mech));
    WRITE_INTEGER(MECH_SCRIPT_HEADING, MechRFacing(mech));
    WRITE_INTEGER(MECH_SCRIPT_STALL, MechStall(mech));
    WRITE_INTEGER(MECH_SCRIPT_STATUS, MechStatus(mech));
    WRITE_INTEGER(MECH_SCRIPT_STATUS_SECONDARY, MechStatus2(mech));
    WRITE_INTEGER(MECH_SCRIPT_CRITICAL_STATUS, MechCritStatus(mech));
    WRITE_INTEGER(MECH_SCRIPT_CRITICAL_STATUS_SECONDARY, MechCritStatus2(mech));
    WRITE_INTEGER(MECH_SCRIPT_VEHICLE_CRITICAL_STATUS,
                  MechTankCritStatus(mech));
    WRITE_DBREF(MECH_SCRIPT_TARGET_DBREF, MechTarget(mech));
    WRITE_INTEGER(MECH_SCRIPT_TEAM, MechTeam(mech));
    WRITE_INTEGER(MECH_SCRIPT_TONNAGE, MechTons(mech));
    WRITE_FLOAT(MECH_SCRIPT_HEAT_PRODUCTION, MechPlusHeat(mech));
    WRITE_FLOAT(MECH_SCRIPT_HEAT, MechHeat(mech));
    WRITE_FLOAT(MECH_SCRIPT_HEAT_DISSIPATION, MechMinusHeat(mech));
    WRITE_INTEGER(MECH_SCRIPT_ENGINE_HEAT_SINK_OVERRIDE,
                  MechHSEngOverRide(mech));
    WRITE_INTEGER(MECH_SCRIPT_LAST_STARTUP, MechLastStartup(mech));
    WRITE_INTEGER(MECH_SCRIPT_MAXIMUM_BATTLE_SUITS, MechMaxSuits(mech));
    WRITE_INTEGER(MECH_SCRIPT_REAL_WEIGHT, MechRTonsV(mech));
    WRITE_INTEGER(MECH_SCRIPT_PREFERENCES, MechPrefs(mech));
    WRITE_DBREF(MECH_SCRIPT_SWARM_TARGET, MechSwarmTarget(mech));
    WRITE_DBREF(MECH_SCRIPT_SWARMER, MechSwarmer(mech));
    WRITE_INTEGER(MECH_SCRIPT_FUEL, AeroFuel(mech));
    WRITE_INTEGER(MECH_SCRIPT_ORIGINAL_FUEL, AeroFuelOrig(mech));
    WRITE_INTEGER(MECH_SCRIPT_SEEN_COUNT, MechNumSeen(mech));
    WRITE_FLOAT(MECH_SCRIPT_REAL_X, MechFX(mech));
    WRITE_FLOAT(MECH_SCRIPT_REAL_Y, MechFY(mech));
    WRITE_FLOAT(MECH_SCRIPT_REAL_Z, MechFZ(mech));
    WRITE_INTEGER(MECH_SCRIPT_X, MechX(mech));
    WRITE_INTEGER(MECH_SCRIPT_Y, MechY(mech));
    WRITE_INTEGER(MECH_SCRIPT_Z, MechZ(mech));
    WRITE_INTEGER(MECH_SCRIPT_ELEVATION, MechElev(mech));
    WRITE_INTEGER(MECH_SCRIPT_TARGETING_COMPUTER, MechTargComp(mech));
    WRITE_INTEGER(MECH_SCRIPT_LONG_RANGE_SENSOR_RANGE, MechLRSRange(mech));
    WRITE_INTEGER(MECH_SCRIPT_RADIO_RANGE, MechRadioRange(mech));
    WRITE_INTEGER(MECH_SCRIPT_SCAN_RANGE, MechScanRange(mech));
    WRITE_INTEGER(MECH_SCRIPT_TACTICAL_SENSOR_RANGE, MechTacRange(mech));
    WRITE_INTEGER(MECH_SCRIPT_RADIO_TYPE, MechRadioType(mech));
    WRITE_INTEGER(MECH_SCRIPT_BATTLE_VALUE, MechBV(mech));
    WRITE_INTEGER(MECH_SCRIPT_CARGO_SPACE, CargoSpace(mech));
    WRITE_DBREF(MECH_SCRIPT_BAY_0, AeroBay(mech, 0));
    WRITE_DBREF(MECH_SCRIPT_BAY_1, AeroBay(mech, 1));
    WRITE_DBREF(MECH_SCRIPT_BAY_2, AeroBay(mech, 2));
    WRITE_DBREF(MECH_SCRIPT_BAY_3, AeroBay(mech, 3));
    WRITE_DBREF(MECH_SCRIPT_TURRET_0, AeroTurret(mech, 0));
    WRITE_DBREF(MECH_SCRIPT_TURRET_1, AeroTurret(mech, 1));
    WRITE_DBREF(MECH_SCRIPT_TURRET_2, AeroTurret(mech, 2));
    WRITE_FLOAT(MECH_SCRIPT_MAXIMUM_JUMP_SPEED, MechJumpSpeed(mech));
    WRITE_INTEGER(MECH_SCRIPT_JUMP_HEADING, MechJumpHeading(mech));
    WRITE_INTEGER(MECH_SCRIPT_JUMP_LENGTH, MechJumpLength(mech));
    WRITE_INTEGER(MECH_SCRIPT_RADIO, MechRadio(mech));
    WRITE_INTEGER(MECH_SCRIPT_COMPUTER, MechComputer(mech));
    WRITE_INTEGER(MECH_SCRIPT_PERCEPTION, MechPer(mech));
    WRITE_INTEGER(MECH_SCRIPT_SHOTS_FIRED, MechShotsFired(mech));
    WRITE_INTEGER(MECH_SCRIPT_SHOTS_MISSED, MechShotsMissed(mech));
    WRITE_INTEGER(MECH_SCRIPT_SHOTS_HIT, MechShotsHit(mech));
    WRITE_INTEGER(MECH_SCRIPT_DAMAGE_TAKEN, MechDamageTaken(mech));
    WRITE_INTEGER(MECH_SCRIPT_DAMAGE_INFLICTED, MechDamageInflicted(mech));
    WRITE_INTEGER(MECH_SCRIPT_UNITS_KILLED, MechUnitsKilled(mech));
    WRITE_FLOAT(MECH_SCRIPT_HEXES_WALKED, MechHexes(mech));
  default:
    return false;
  }

#undef WRITE_INTEGER
#undef WRITE_FLOAT
#undef WRITE_DBREF
#undef WRITE_STRING
}
