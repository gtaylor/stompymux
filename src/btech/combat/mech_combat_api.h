/* Declares the BattleTech unit combat API. */

#pragma once

#include "map_coordinates.h"
#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct TerrainWeaponHitRequest {
  Mech *attacker;
  MapHexPosition position;
  int weapon_index;
  int ammunition_mode;
  int damage;
  bool hit;
} TerrainWeaponHitRequest;

typedef struct TerrainWeaponEffectRequest {
  Mech *mech;
  BattleMap *map;
  MapHexPosition position;
  int weapon_index;
  int ammunition_mode;
  int damage;
  bool intentional;
} TerrainWeaponEffectRequest;

/* mech.combat.c */
void mech_target(DbRef player, void *data, char *buffer);
void mech_sixth_sense_check(Mech *mech, Mech *target);
void mech_set_target(DbRef player, void *data, char *buffer);
void mech_fireweapon(DbRef player, void *data, char *buffer);
typedef struct WeaponFireCommandRequest {
  DbRef actor;
  Mech *mech;
  BattleMap *map;
  int weapon_number;
  int argument_count;
  char **arguments;
  bool sight;
} WeaponFireCommandRequest;

int mech_weapon_fire_command(const WeaponFireCommandRequest *request);
const char *mech_hex_target_description(const Mech *mech);
int canClearOrIgnite(int weapindx);
void mech_terrain_hex_hit(const TerrainWeaponHitRequest *request);
void mech_terrain_possibly_ignite_or_clear(
    const TerrainWeaponEffectRequest *request);
typedef struct WeaponFailureResolutionRequest {
  Mech *mech;
  int weapon_number;
  int weapon_index;
  CriticalSlotReference weapon;
  CriticalSlotLookupResult primary_ammunition;
  CriticalSlotLookupResult secondary_ammunition;
  float range;
  int gatling_shots;
} WeaponFailureResolutionRequest;

typedef struct WeaponFailureResolution {
  bool handled;
  bool range_ok;
  int modifier;
  int type;
} WeaponFailureResolution;

WeaponFailureResolution
weapon_failure_resolve(const WeaponFailureResolutionRequest *request);

typedef struct WeaponFireRequest {
  Mech *mech;
  BattleMap *map;
  Mech *target;
  int line_of_sight;
  int weapon_index;
  int weapon_number;
  CriticalSlotReference weapon;
  MapHexPosition target_hex;
  float range;
  int indirect_fire;
  bool sight;
  int target_kind;
} WeaponFireRequest;

void mech_weapon_fire(const WeaponFireRequest *request);
typedef struct HitDamageRequest {
  Mech *attacker;
  CriticalSlotReference weapon;
  Mech *target;
  MapHexPosition target_hex;
  int weapon_index;
  int gatling_shots;
  int base_damage;
  int ammunition_mode;
  int failure_type;
  int failure_modifier;
  bool temporary_calculation;
} HitDamageRequest;

int mech_hit_damage_determine(const HitDamageRequest *request);

typedef struct HitResolutionRequest {
  Mech *attacker;
  int weapon_index;
  CriticalSlotReference weapon;
  Mech *target;
  MapHexPosition target_hex;
  int line_of_sight;
  int failure_type;
  int failure_modifier;
  bool hit;
  int base_to_hit;
  int gatling_shots;
  bool swarm_attack;
  int player_roll;
} HitResolutionRequest;

void mech_hit_resolve(const HitResolutionRequest *request);
