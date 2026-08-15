
/* p.mech.bth.h */

#pragma once

#include "mux/server/platform.h"

typedef struct MechNormalToHitRequest {
  Mech *attacker;
  BattleMap *map;
  int section;
  int critical;
  int weapon_index;
  float range;
  Mech *target;
  int indirect_fire;
} MechNormalToHitRequest;

typedef struct MechNormalToHitResult {
  int value;
  DbRef c3_reference;
} MechNormalToHitResult;

MechNormalToHitResult
mech_normal_to_hit_calculate(const MechNormalToHitRequest *request);

typedef struct MechArtilleryToHitRequest {
  Mech *attacker;
  int section;
  int weapon_index;
  bool indirect;
  float range;
} MechArtilleryToHitRequest;

int mech_artillery_to_hit_calculate(const MechArtilleryToHitRequest *request);
typedef enum WeaponRangeBracket : int {
  RANGE_SHORT = 0,
  RANGE_MED = 1,
  RANGE_LONG = 2,
  RANGE_EXTREME = 3,
  RANGE_TOFAR = 4,
  RANGE_NOWATER = 5,
} WeaponRangeBracket;

typedef struct WeaponRangeToHitRequest {
  Mech *attacker;
  Mech *target;
  int section;
  int weapon_index;
  float range;
  int fire_mode;
  int ammunition_mode;
} WeaponRangeToHitRequest;

typedef struct C3RangeToHitRequest {
  Mech *attacker;
  Mech *target;
  int section;
  int weapon_index;
  float physical_range;
  float network_range;
  int fire_mode;
} C3RangeToHitRequest;

typedef struct WeaponRangeToHitResult {
  WeaponRangeBracket bracket;
  int modifier;
} WeaponRangeToHitResult;

WeaponRangeToHitResult
mech_range_to_hit_calculate(const WeaponRangeToHitRequest *request);
WeaponRangeToHitResult
mech_c3_range_to_hit_calculate(const C3RangeToHitRequest *request);
int mech_attacker_movement_modifier(Mech *mech);
int mech_target_movement_modifier(Mech *mech, Mech *target, float range);

static_assert((RANGE_SHORT == 0 && RANGE_NOWATER == 5) != 0);
