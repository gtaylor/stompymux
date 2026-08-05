/* context.h - Public ownership and command-scope interface for BTech. */

#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <time.h>

#include "btech/ids.h"

typedef struct AccessControlStore AccessControlStore;
typedef struct BtechContext BtechContext;
typedef struct BtechCommandScope BtechCommandScope;
typedef struct CommandContext CommandContext;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct PersistenceContext PersistenceContext;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;
typedef struct ServerLifecycle ServerLifecycle;
typedef struct ServerLog ServerLog;
typedef struct WorldIndexes WorldIndexes;

typedef struct BtechDependencies {
  /* Every dependency is borrowed and must outlive the BTech context. */
  ServerConfiguration *configuration;
  RuntimeClock *clock;
  CommandContext *background_command;
  GameDatabase *database;
  MuxEventScheduler *events;
  ServerLifecycle *lifecycle;
  ServerLog *log;
  PersistenceContext *persistence;
  WorldIndexes *world_indexes;
  AccessControlStore *access_control;
  time_t process_start_time;
} BtechDependencies;

struct BtechCommandScope {
  /* Scope links are borrowed and valid only for the nested invocation. */
  BtechContext *context;
  CommandContext *command;
  BtechCommandScope *previous;
  bool active;
};

BtechContext *btech_context_create(const BtechDependencies *dependencies);
void btech_context_destroy(BtechContext *context);
void btech_context_set_lifecycle(BtechContext *context,
                                 ServerLifecycle *lifecycle);
void btech_context_set_process_start_time(BtechContext *context,
                                          time_t process_start_time);
CommandContext *btech_context_command(BtechContext *context);
EvaluationContext *btech_context_evaluation(BtechContext *context);
GameDatabase *btech_context_database(BtechContext *context);
BtechObjectId btech_context_afterlife_dbref(const BtechContext *context);
bool btech_context_in_character_enabled(const BtechContext *context);
int btech_context_experience_loss(const BtechContext *context);
bool btech_context_mechwarrior_experience_loss_enabled(
    const BtechContext *context);
bool btech_context_transported_unit_death_enabled(const BtechContext *context);
bool btech_context_vtol_ice_fire_enabled(const BtechContext *context);
bool btech_context_combat_arcs_enabled(const BtechContext *context);
void btech_context_combat_arcs_override_set(BtechContext *context, int arcs);
void btech_context_combat_pilot_override_set(BtechContext *context,
                                             BtechObjectId pilot);
bool btech_context_seismic_detects_stopped_units(const BtechContext *context);
bool btech_context_inferno_penalty_enabled(const BtechContext *context);
bool btech_context_uses_fasa_turning(const BtechContext *context);
bool btech_context_uses_extended_movement_modifiers(
    const BtechContext *context);
bool btech_context_uses_extended_weapon_ranges(const BtechContext *context);
bool btech_context_hotload_uses_half_modifier(const BtechContext *context);
bool btech_context_overrides_weapon_arcs(const BtechContext *context);
bool btech_context_dig_bonus_requires_front(const BtechContext *context);
int btech_context_dig_bonus(const BtechContext *context);
bool btech_context_range_modifies_damage(const BtechContext *context);
bool btech_context_woods_modify_damage(const BtechContext *context);
bool btech_context_glancing_blows_enabled(const BtechContext *context);
int btech_context_sprint_to_hit_modifier(const BtechContext *context);
bool btech_context_uses_skid_cliff_rules(const BtechContext *context);
bool btech_context_uses_roll_on_backwalk(const BtechContext *context);
bool btech_context_uses_new_terrain_rules(const BtechContext *context);
bool btech_context_uses_advanced_vehicle_fire(const BtechContext *context);
bool btech_context_uses_advanced_vehicle_criticals(const BtechContext *context);
bool btech_context_uses_advanced_vtol_criticals(const BtechContext *context);
bool btech_context_uses_fasa_criticals(const BtechContext *context);
bool btech_context_uses_exile_stun_code(const BtechContext *context);
int btech_context_exile_stun_mode(const BtechContext *context);
int btech_context_critical_level(const BtechContext *context);
int btech_context_hit_arc_mode(const BtechContext *context);
int btech_context_vehicle_critical_mode(const BtechContext *context);
bool btech_context_uses_tank_friendly_criticals(const BtechContext *context);
bool btech_context_uses_tank_critical_shielding(const BtechContext *context);
bool btech_context_uses_tsm_sprint_bonus(const BtechContext *context);
bool btech_context_uses_tsm_tow_bonus(const BtechContext *context);
int btech_context_stand_careful_modifier(const BtechContext *context);
int btech_context_landing_zone_mode(const BtechContext *context);
int btech_context_self_destruct_time(const BtechContext *context);
bool btech_context_self_destruct_can_stop(const BtechContext *context);
bool btech_context_self_destruct_ammunition_enabled(
    const BtechContext *context);
bool btech_context_self_destruct_reactor_enabled(const BtechContext *context);
int btech_context_reactor_explosion_mode(const BtechContext *context);
bool btech_context_stackpole_enabled(const BtechContext *context);
bool btech_context_requires_backwalk_rolls(const BtechContext *context);
bool btech_context_uses_new_charge_rules(const BtechContext *context);
int btech_context_movement_slowdown_mode(const BtechContext *context);
int btech_context_stacking_mode(const BtechContext *context);
int btech_context_stacking_damage(const BtechContext *context);
int btech_context_stagger_mode(const BtechContext *context);
int btech_context_stagger_interval(const BtechContext *context);
bool btech_context_stagger_uses_tonnage(const BtechContext *context);
int btech_context_event_tick(const BtechContext *context);
void btech_context_hit_roll_record(BtechContext *context, int roll);
void btech_context_critical_roll_record(BtechContext *context, int roll);
long btech_context_random_i31(BtechContext *context);
int btech_context_missile_hit_count(const BtechContext *context,
                                    int weapon_index, int roll_index);
int btech_context_missile_hit_count_by_name(const BtechContext *context,
                                            const char *name, int roll_index);
int btech_context_event_data_count(const BtechContext *context, int event_type,
                                   intptr_t event_data);
time_t btech_context_now(const BtechContext *context);
void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command);
void btech_command_scope_leave(BtechCommandScope *scope);

#ifdef BTECH_INTERNAL
#include "context_internal.h"
#endif
