/* context_internal.h - Private BTech runtime ownership. */

#pragma once

#include "coding_registry.h"
#include "missile_hit_registry.h"
#include "mux/support/red_black_tree.h"
#include "random.h"
#include "weapon_settings.h"

typedef struct HashTable HashTable;
typedef struct MechTemplateRegistry MechTemplateRegistry;
typedef struct MechReferenceCache MechReferenceCache;
typedef struct BtechPartCosts BtechPartCosts;
typedef struct PartNameRegistry PartNameRegistry;
typedef struct MuxTimer MuxTimer;

typedef struct BtechCombatOverrides {
  BtechObjectId pilot;
  int arcs;
  BtechDamageExperienceMode damage_experience;
} BtechCombatOverrides;

struct BtechContext {
  BtechDependencies dependencies;

  /* Transitional aliases disappear as domains move to narrow dependencies. */
  ServerConfiguration *configuration;
  RuntimeClock *clock;
  CommandContext *background_command;
  BtechCommandScope *command_scope;
  GameDatabase *database;
  MuxEventScheduler *events;
  ServerLifecycle *lifecycle;
  ServerLog *log;
  PersistenceContext *persistence;
  WorldIndexes *world_indexes;
  AccessControlStore *access_control;
  time_t process_start_time;

  RedBlackTree special_objects;
  HashTable *special_commands;
  size_t special_command_count;
  HashTable *player_value_hashes;
  char **char_value_short_names;
  size_t char_value_count;
  long cached_target_character;
  int cached_skill;
  int cached_skill_result;
  BtechCombatOverrides combat_overrides;
  MapCodingRegistry map_coding;
  MissileHitRegistry missile_hits;
  BtechRandom random;
  BtechWeaponSettings weapon_settings;
  BtechPartCosts *part_costs;
  PartNameRegistry *part_names;
  MechTemplateRegistry *templates;
  MechReferenceCache *reference_mech_cache;
  MuxTimer *heartbeat;
  time_t last_special_update;
  unsigned int tick;
  bool heartbeat_running;
};

void btech_context_release_owned_state(BtechContext *context);
