/* btech_context.h - Runtime dependencies for legacy BTech callbacks. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <time.h>

#include "btech/map_coding_registry.h"
#include "btech/missile_hit_registry.h"
#include "btech/random.h"
#include "btech/weapon_settings.h"
#include "mux/server/platform.h"
#include "mux/support/red_black_tree.h"

typedef struct AccessControlStore AccessControlStore;
typedef struct CommandContext CommandContext;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct HashTable HashTable;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct MuxTimer MuxTimer;
typedef struct MechTemplateRegistry MechTemplateRegistry;
typedef struct MechReferenceCache MechReferenceCache;
typedef struct PersistenceContext PersistenceContext;
typedef struct BtechPartCosts BtechPartCosts;
typedef struct BtechColorizeState BtechColorizeState;
typedef struct PartNameRegistry PartNameRegistry;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;
typedef struct WorldIndexes WorldIndexes;

typedef struct BtechContext BtechContext;
typedef struct BtechCommandScope BtechCommandScope;
typedef struct BtechCombatOverrides BtechCombatOverrides;
typedef enum BtechDamageExperienceMode {
  BTECH_DAMAGE_XP_GUNNERY,
  BTECH_DAMAGE_XP_PILOTING,
  BTECH_DAMAGE_XP_NONE,
} BtechDamageExperienceMode;

struct BtechCombatOverrides {
  DbRef pilot;
  int arcs;
  BtechDamageExperienceMode damage_experience;
};

struct BtechCommandScope {
  /* Scope links are borrowed and valid only for the nested invocation. */
  BtechContext *context;
  CommandContext *command;
  BtechCommandScope *previous;
  bool active;
};

struct BtechContext {
  /* Runtime service pointers are borrowed from MuxServer. */
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

  /* BTech owns its special-object registry and heartbeat state. */
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
  BtechColorizeState *colorize;
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

void btech_context_initialize(
    BtechContext *context, ServerConfiguration *configuration,
    RuntimeClock *clock, CommandContext *command_context,
    GameDatabase *database, MuxEventScheduler *events,
    ServerLifecycle *lifecycle, ServerLog *log, PersistenceContext *persistence,
    WorldIndexes *world_indexes, AccessControlStore *access_control,
    time_t process_start_time);
void btech_context_destroy(BtechContext *context);
CommandContext *btech_context_command(BtechContext *context);
EvaluationContext *btech_context_evaluation(BtechContext *context);
void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command);
void btech_command_scope_leave(BtechCommandScope *scope);
