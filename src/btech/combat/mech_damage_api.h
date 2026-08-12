
/* p.mech.damage.h */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct ArmorDamageRequest {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  bool rear;
  bool critical;
  int section;
  int damage;
  int *critical_hits;
  int weapon_index;
  int ammunition_mode;
} ArmorDamageRequest;

int cause_armordamage(const ArmorDamageRequest *request);

typedef struct InternalDamageRequest {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  int section;
  int damage;
  int *critical_hits;
} InternalDamageRequest;

int cause_internaldamage(const InternalDamageRequest *request);
typedef enum MechDamageTransferKind : int {
  MECH_DAMAGE_NORMAL = 0,
  MECH_DAMAGE_FORCE_TRANSFER = 1,
  MECH_DAMAGE_TRANSFER_CONTINUATION = 2,
} MechDamageTransferKind;

typedef struct MechDamageRequest {
  Mech *target;
  Mech *attacker;
  bool line_of_sight;
  DbRef attack_pilot;
  int hit_location;
  bool rear;
  bool critical;
  int armor_damage;
  int internal_damage;
  MechDamageTransferKind transfer;
  int cause;
  int base_to_hit;
  int weapon_index;
  int ammunition_mode;
  bool ignore_swarmers;
} MechDamageRequest;

void mech_damage_apply(const MechDamageRequest *request);
typedef struct WeaponDestructionRequest {
  Mech *mech;
  CriticalSlotReference first;
  int part_type;
  int criticals_to_destroy;
  int total_criticals;
} WeaponDestructionRequest;

void mech_weapon_destroy(const WeaponDestructionRequest *request);
int mech_weapon_count_in_section(Mech *mech, int loc);
typedef struct WeaponSectionLookup {
  Mech *mech;
  int section;
  int ordinal;
} WeaponSectionLookup;

int mech_weapon_index_in_section(const WeaponSectionLookup *lookup);
void mech_weapon_destroy_random(Mech *mech, int hitloc);
void mech_heat_sink_destroy(Mech *mech, int hitloc);
typedef struct SectionDestructionRequest {
  Mech *wounded;
  Mech *attacker;
  int line_of_sight;
  int section;
} SectionDestructionRequest;

void mech_section_destroy(const SectionDestructionRequest *request);

typedef struct ArmorStatusSetRequest {
  Mech *mech;
  const char *section;
  const char *armor_type;
  const char *value;
} ArmorStatusSetRequest;

const char *mech_armor_status_set_value(const ArmorStatusSetRequest *request);

typedef struct DamageClusterRequest {
  Mech *mech;
  int total_damage;
  int cluster_size;
  int direction;
  bool critical;
  const char *mech_message;
  const char *broadcast_message;
} DamageClusterRequest;

bool mech_damage_apply_clusters(const DamageClusterRequest *request);
void mech_damage(DbRef player, Mech *mech, char *buffer);
void mech_damage_section(DbRef player, Mech *mech, char *buffer);
