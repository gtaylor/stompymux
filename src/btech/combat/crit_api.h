/* Declares the BattleTech crit API. */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

/* crit.c */
void mech_speed_correct(Mech *mech);
void mech_explosion_apply(Mech *wounded, Mech *attacker);
typedef struct WeaponCriticalRequest {
  Mech *attacker;
  Mech *wounded;
  CriticalSlotReference slot;
  int part_type;
} WeaponCriticalRequest;

int mech_weapon_critical_handle(const WeaponCriticalRequest *request);

typedef struct VehicleCriticalRequest {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  int section;
} VehicleCriticalRequest;

void mech_vtol_critical_handle(const VehicleCriticalRequest *request);
void mech_main_weapon_destroy(Mech *mech);
void mech_main_weapon_jam(Mech *mech);
void mech_random_weapon_select(Mech *objMech, int wLoc, int *critNum,
                               int wIgnoreJams);
void mech_speed_limit_to_cruise(Mech *objMech);
void mech_vehicle_stabilizer_critical_apply(Mech *objMech, int wLoc);
void mech_turret_jam_critical_apply(Mech *objMech);
void mech_weapon_jam_critical_apply(Mech *objMech, int wLoc);
void mech_turret_lock_critical_apply(Mech *objMech);
typedef struct RandomWeaponDestructionRequest {
  Mech *attacker;
  Mech *mech;
  int section;
} RandomWeaponDestructionRequest;

void mech_weapon_destroyed_critical_apply(
    const RandomWeaponDestructionRequest *request);
void mech_turret_blown_off_critical_apply(Mech *objMech, Mech *objAttacker,
                                          int LOS);
typedef struct AmmunitionCriticalRequest {
  Mech *mech;
  Mech *attacker;
  int section;
} AmmunitionCriticalRequest;

void mech_ammunition_critical_apply(const AmmunitionCriticalRequest *request);
void mech_cargo_infantry_critical_apply(Mech *objMech, int wLoc);
void mech_vehicle_engine_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vehicle_fuel_tank_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vehicle_crew_stun_critical_apply(Mech *objMech);
void mech_vehicle_driver_critical_apply(Mech *objMech);
void mech_vehicle_sensor_critical_apply(Mech *objMech);
void mech_vehicle_commander_critical_apply(Mech *objMech);
void mech_vehicle_crew_killed_critical_apply(Mech *objMech, Mech *objAttacker);
void mech_vtol_copilot_critical_apply(Mech *objMech);
void mech_vtol_pilot_critical_apply(Mech *objMech);
void mech_vtol_rotor_damaged_critical_apply(Mech *objMech);
void mech_vtol_tail_rotor_critical_apply(Mech *objMech);
void mech_vtol_rotor_destroyed_critical_apply(Mech *objMech, Mech *objAttacker,
                                              int LOS);
void mech_vtol_crash_start(Mech *objMech);
void mech_advanced_vehicle_critical_handle(
    const VehicleCriticalRequest *request);
void mech_fasa_vehicle_critical_handle(const VehicleCriticalRequest *request);
void mech_vehicle_critical_handle(const VehicleCriticalRequest *request);

typedef struct CriticalEffectRequest {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  CriticalSlotReference slot;
  int part_type;
  int part_data;
} CriticalEffectRequest;

int mech_critical_effect_apply(const CriticalEffectRequest *request);
typedef struct CriticalHitDispatch {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  int section;
  int count;
} CriticalHitDispatch;

void mech_critical_handle(const CriticalHitDispatch *dispatch);
void mech_section_actuator_criticals_normalize(Mech *objMech, int wLoc);
void mech_actuator_criticals_normalize(Mech *objMech);
