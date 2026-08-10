/* Declares missile-hit and anti-missile combat operations. */

#pragma once

#include "map_coordinates.h"
#include "mux/server/platform.h"

typedef struct WeaponCriticalReference {
  int weapon_index;
  int section;
  int critical;
} WeaponCriticalReference;

typedef struct MissileHitsRequest {
  Mech *attacker;
  Mech *target;
  MapHexPosition target_hex;
  bool rear;
  bool critical;
  WeaponCriticalReference weapon;
  int fire_mode;
  int ammunition_mode;
  int missile_count;
  int damage_per_missile;
  int salvo_size;
  int los;
  int base_to_hit;
  bool swarm_attack;
} MissileHitsRequest;

typedef struct MissileHitIndexRequest {
  Mech *attacker;
  Mech *target;
  WeaponCriticalReference weapon;
  bool glancing;
} MissileHitIndexRequest;

typedef struct MissileAttackRequest {
  Mech *attacker;
  Mech *target;
  WeaponCriticalReference weapon;
  MapHexPosition target_hex;
  int los;
  int base_to_hit;
  int roll;
  int incoming;
  bool friend_or_foe;
  bool swarm_attack;
  int player_roll;
} MissileAttackRequest;

typedef struct AmsDefenseResult {
  bool found;
  int weapon_type;
  int ammunition_section;
  int ammunition_critical;
} AmsDefenseResult;

typedef struct AmsInterceptRequest {
  Mech *attacker;
  Mech *target;
  int incoming;
  AmsDefenseResult defense;
  int los;
  bool missiles_hit;
} AmsInterceptRequest;

void mech_missile_apply_hits(const MissileHitsRequest *request);
int mech_missile_hit_index(const MissileHitIndexRequest *request);
int mech_missile_hit_target(const MissileAttackRequest *request);
void mech_swarm_missile_hit_target(const MissileAttackRequest *request);
int mech_ams_intercept(const AmsInterceptRequest *request);
AmsDefenseResult mech_ams_locate_defenses(Mech *target);
