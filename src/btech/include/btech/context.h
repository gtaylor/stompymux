/** @file
 * Public ownership and command-scope interface for BTech.
 */

#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <time.h>

#include "btech/ids.h"

typedef struct AccessControlStore AccessControlStore;
typedef struct BtechContext BtechContext;
typedef struct MechTemplateRegistry MechTemplateRegistry;
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

/** Skill category credited for damage experience. */
typedef enum BtechDamageExperienceMode : int {
  /** Credit gunnery experience. */
  BTECH_DAMAGE_XP_GUNNERY,
  /** Credit piloting experience. */
  BTECH_DAMAGE_XP_PILOTING,
  /** Do not credit damage experience. */
  BTECH_DAMAGE_XP_NONE,
} BtechDamageExperienceMode;

/** Borrowed services required by a BTech runtime context. */
typedef struct BtechDependencies {
  /** Server configuration; must outlive the BTech context. */
  ServerConfiguration *configuration;
  /** Runtime clock; must outlive the BTech context. */
  RuntimeClock *clock;
  /** Background command context; must outlive the BTech context. */
  CommandContext *background_command;
  /** Game database; must outlive the BTech context. */
  GameDatabase *database;
  /** Event scheduler; must outlive the BTech context. */
  MuxEventScheduler *events;
  /** Server lifecycle; must outlive the BTech context. */
  ServerLifecycle *lifecycle;
  /** Server log; must outlive the BTech context. */
  ServerLog *log;
  /** Persistence registry; must outlive the BTech context. */
  PersistenceContext *persistence;
  /** World indexes; must outlive the BTech context. */
  WorldIndexes *world_indexes;
  /** Access-control store; must outlive the BTech context. */
  AccessControlStore *access_control;
  /** Time at which the server process started. */
  time_t process_start_time;
} BtechDependencies;

/** Temporarily binds a command context to a BTech invocation. */
struct BtechCommandScope {
  /** Borrowed BTech context for the nested invocation. */
  BtechContext *context;
  /** Borrowed command context for the nested invocation. */
  CommandContext *command;
  /** Previously active command scope. */
  BtechCommandScope *previous;
  /** Whether this scope is currently entered. */
  bool active;
};

/**
 * Creates a BTech runtime context.
 *
 * @param[in] dependencies Borrowed services that must outlive the context.
 * @return New context, or `nullptr` when creation fails.
 */
BtechContext *btech_context_create(const BtechDependencies *dependencies);

/**
 * Destroys a BTech runtime context.
 *
 * @param[in,out] context Context to destroy.
 */
void btech_context_destroy(BtechContext *context);

/**
 * Replaces the server lifecycle used by BTech.
 *
 * @param[in,out] context BTech runtime context.
 * @param[in] lifecycle Borrowed lifecycle that must outlive @p context.
 */
void btech_context_set_lifecycle(BtechContext *context,
                                 ServerLifecycle *lifecycle);

/**
 * Sets the server process start time.
 *
 * @param[in,out] context BTech runtime context.
 * @param[in] process_start_time Server process start time.
 */
void btech_context_set_process_start_time(BtechContext *context,
                                          time_t process_start_time);

/**
 * Returns the command context active for BTech work.
 *
 * @param[in] context BTech runtime context.
 * @return Borrowed command context.
 */
CommandContext *btech_context_command(BtechContext *context);

/**
 * Returns the active expression-evaluation context.
 *
 * @param[in] context BTech runtime context.
 * @return Borrowed evaluation context.
 */
EvaluationContext *btech_context_evaluation(BtechContext *context);

/**
 * Returns the game database used by BTech.
 *
 * @param[in] context BTech runtime context.
 * @return Borrowed game database.
 */
GameDatabase *btech_context_database(BtechContext *context);

/**
 * Returns the configured mech-template path.
 *
 * @param[in] context BTech runtime context.
 * @return Borrowed, null-terminated path.
 */
const char *btech_context_mech_template_path(const BtechContext *context);

/**
 * Returns the mech-template registry.
 *
 * @param[in] context BTech runtime context.
 * @return Borrowed registry, or `nullptr` when none is installed.
 */
MechTemplateRegistry *
btech_context_mech_template_registry(const BtechContext *context);

/**
 * Installs the mech-template registry.
 *
 * @param[in,out] context BTech runtime context.
 * @param[in] registry Borrowed registry, or `nullptr` to clear it.
 */
void btech_context_mech_template_registry_set(BtechContext *context,
                                              MechTemplateRegistry *registry);
/** Returns the configured afterlife object. @param[in] context BTech context.
 */
BtechObjectId btech_context_afterlife_dbref(const BtechContext *context);
/** Reports whether in-character rules are enabled. @param[in] context BTech
 * context. */
bool btech_context_in_character_enabled(const BtechContext *context);
/** Returns the configured experience loss. @param[in] context BTech context. */
int btech_context_experience_loss(const BtechContext *context);
/** Reports whether mechwarriors lose experience. @param[in] context BTech
 * context. */
bool btech_context_mechwarrior_experience_loss_enabled(
    const BtechContext *context);
/** Reports whether transported units may die. @param[in] context BTech context.
 */
bool btech_context_transported_unit_death_enabled(const BtechContext *context);
/** Reports whether VTOL ICE fires are enabled. @param[in] context BTech
 * context. */
bool btech_context_vtol_ice_fire_enabled(const BtechContext *context);
/** Reports whether configured combat arcs are enabled. @param[in] context BTech
 * context. */
bool btech_context_combat_arcs_enabled(const BtechContext *context);
/** Sets the combat-arc override. @param[in,out] context BTech context.
 * @param[in] arcs Arc override value. */
void btech_context_combat_arcs_override_set(BtechContext *context, int arcs);
/** Sets the combat-pilot override. @param[in,out] context BTech context.
 * @param[in] pilot Pilot object identifier. */
void btech_context_combat_pilot_override_set(BtechContext *context,
                                             BtechObjectId pilot);
/** Reports whether seismic sensors detect stopped units. @param[in] context
 * BTech context. */
bool btech_context_seismic_detects_stopped_units(const BtechContext *context);
/** Reports whether inferno penalties are enabled. @param[in] context BTech
 * context. */
bool btech_context_inferno_penalty_enabled(const BtechContext *context);
/** Reports whether FASA turning rules are used. @param[in] context BTech
 * context. */
bool btech_context_uses_fasa_turning(const BtechContext *context);
/** Reports whether extended movement modifiers are used. @param[in] context
 * BTech context. */
bool btech_context_uses_extended_movement_modifiers(
    const BtechContext *context);
/** Reports whether extended weapon ranges are used. @param[in] context BTech
 * context. */
bool btech_context_uses_extended_weapon_ranges(const BtechContext *context);
/** Reports whether hotloading uses a half modifier. @param[in] context BTech
 * context. */
bool btech_context_hotload_uses_half_modifier(const BtechContext *context);
/** Reports whether weapon arcs are overridden. @param[in] context BTech
 * context. */
bool btech_context_overrides_weapon_arcs(const BtechContext *context);
/** Returns the weapon-arc override. @param[in] context BTech context. */
int btech_context_weapon_arc_override(const BtechContext *context);
/** Reports whether indirect fire requires a spotter. @param[in] context BTech
 * context. */
bool btech_context_idf_requires_spotter(const BtechContext *context);
/** Reports whether dig bonuses require facing front. @param[in] context BTech
 * context. */
bool btech_context_dig_bonus_requires_front(const BtechContext *context);
/** Returns the configured dig bonus. @param[in] context BTech context. */
int btech_context_dig_bonus(const BtechContext *context);
/** Reports whether range modifies damage. @param[in] context BTech context. */
bool btech_context_range_modifies_damage(const BtechContext *context);
/** Reports whether woods modify damage. @param[in] context BTech context. */
bool btech_context_woods_modify_damage(const BtechContext *context);
/** Reports whether glancing blows are enabled. @param[in] context BTech
 * context. */
bool btech_context_glancing_blows_enabled(const BtechContext *context);
/** Returns the glancing-blow rules mode. @param[in] context BTech context. */
int btech_context_glancing_blow_mode(const BtechContext *context);
/** Returns the rotor-damage divisor. @param[in] context BTech context. */
int btech_context_rotor_damage_divisor(const BtechContext *context);
/** Returns the damage-experience skill category. @param[in] context BTech
 * context. */
BtechDamageExperienceMode
btech_context_damage_experience_mode(const BtechContext *context);
/** Sets the damage-experience skill category. @param[in,out] context BTech
 * context. @param[in] mode Skill category to credit. */
void btech_context_damage_experience_mode_set(BtechContext *context,
                                              BtechDamageExperienceMode mode);
/** Returns the stat-engine object. @param[in] context BTech context. */
BtechObjectId btech_context_stat_engine_dbref(const BtechContext *context);
/** Returns the used-mech store object. @param[in] context BTech context. */
BtechObjectId btech_context_used_mech_store_dbref(const BtechContext *context);
/** Reports whether mechwarrior pickup triggers actions. @param[in] context
 * BTech context. */
bool btech_context_mechwarrior_pickup_triggers_actions(
    const BtechContext *context);
/** Reports whether physical attacks use piloting skill. @param[in] context
 * BTech context. */
bool btech_context_physical_attacks_use_pilot_skill(
    const BtechContext *context);
/** Reports whether repairs are limited to stalls. @param[in] context BTech
 * context. */
bool btech_context_limits_repairs_to_stalls(const BtechContext *context);
/** Returns the technology-time multiplier. @param[in] context BTech context. */
double btech_context_technology_time_multiplier(const BtechContext *context);
/** Returns the maximum technology time. @param[in] context BTech context. */
int btech_context_maximum_technology_time(const BtechContext *context);
/** Reports whether variable technology times are used. @param[in] context BTech
 * context. */
bool btech_context_uses_variable_technology_time(const BtechContext *context);
/** Returns the technology-time modifier. @param[in] context BTech context. */
int btech_context_technology_time_modifier(const BtechContext *context);
/** Returns the sprint to-hit modifier. @param[in] context BTech context. */
int btech_context_sprint_to_hit_modifier(const BtechContext *context);
/** Reports whether skid-cliff rules are used. @param[in] context BTech context.
 */
bool btech_context_uses_skid_cliff_rules(const BtechContext *context);
/** Reports whether backwalking requires a roll. @param[in] context BTech
 * context. */
bool btech_context_uses_roll_on_backwalk(const BtechContext *context);
/** Reports whether new terrain rules are used. @param[in] context BTech
 * context. */
bool btech_context_uses_new_terrain_rules(const BtechContext *context);
/** Reports whether advanced vehicle-fire rules are used. @param[in] context
 * BTech context. */
bool btech_context_uses_advanced_vehicle_fire(const BtechContext *context);
/** Reports whether advanced vehicle criticals are used. @param[in] context
 * BTech context. */
bool btech_context_uses_advanced_vehicle_criticals(const BtechContext *context);
/** Reports whether advanced VTOL criticals are used. @param[in] context BTech
 * context. */
bool btech_context_uses_advanced_vtol_criticals(const BtechContext *context);
/** Reports whether FASA critical rules are used. @param[in] context BTech
 * context. */
bool btech_context_uses_fasa_criticals(const BtechContext *context);
/** Reports whether exile stun handling is used. @param[in] context BTech
 * context. */
bool btech_context_uses_exile_stun_code(const BtechContext *context);
/** Returns the exile-stun mode. @param[in] context BTech context. */
int btech_context_exile_stun_mode(const BtechContext *context);
/** Returns the configured critical level. @param[in] context BTech context. */
int btech_context_critical_level(const BtechContext *context);
/** Returns the hit-arc mode. @param[in] context BTech context. */
int btech_context_hit_arc_mode(const BtechContext *context);
/** Returns the vehicle-critical mode. @param[in] context BTech context. */
int btech_context_vehicle_critical_mode(const BtechContext *context);
/** Reports whether tank-friendly criticals are used. @param[in] context BTech
 * context. */
bool btech_context_uses_tank_friendly_criticals(const BtechContext *context);
/** Reports whether tank critical shielding is used. @param[in] context BTech
 * context. */
bool btech_context_uses_tank_critical_shielding(const BtechContext *context);
/** Reports whether TSM grants a sprint bonus. @param[in] context BTech context.
 */
bool btech_context_uses_tsm_sprint_bonus(const BtechContext *context);
/** Reports whether TSM grants a towing bonus. @param[in] context BTech context.
 */
bool btech_context_uses_tsm_tow_bonus(const BtechContext *context);
/** Returns the careful-stand modifier. @param[in] context BTech context. */
int btech_context_stand_careful_modifier(const BtechContext *context);
/** Returns the landing-zone mode. @param[in] context BTech context. */
int btech_context_landing_zone_mode(const BtechContext *context);
/** Returns the self-destruct countdown. @param[in] context BTech context. */
int btech_context_self_destruct_time(const BtechContext *context);
/** Reports whether self-destruct may be stopped. @param[in] context BTech
 * context. */
bool btech_context_self_destruct_can_stop(const BtechContext *context);
/** Reports whether ammunition self-destruct is enabled. @param[in] context
 * BTech context. */
bool btech_context_self_destruct_ammunition_enabled(
    const BtechContext *context);
/** Reports whether reactor self-destruct is enabled. @param[in] context BTech
 * context. */
bool btech_context_self_destruct_reactor_enabled(const BtechContext *context);
/** Returns the reactor-explosion mode. @param[in] context BTech context. */
int btech_context_reactor_explosion_mode(const BtechContext *context);
/** Reports whether stackpole explosions are enabled. @param[in] context BTech
 * context. */
bool btech_context_stackpole_enabled(const BtechContext *context);
/** Reports whether backwalking requires piloting rolls. @param[in] context
 * BTech context. */
bool btech_context_requires_backwalk_rolls(const BtechContext *context);
/** Reports whether new charge rules are used. @param[in] context BTech context.
 */
bool btech_context_uses_new_charge_rules(const BtechContext *context);
/** Reports whether level-three charge rules are used. @param[in] context BTech
 * context. */
bool btech_context_uses_technology_level_three_charge_rules(
    const BtechContext *context);
/** Returns the movement-slowdown mode. @param[in] context BTech context. */
int btech_context_movement_slowdown_mode(const BtechContext *context);
/** Returns the unit-stacking mode. @param[in] context BTech context. */
int btech_context_stacking_mode(const BtechContext *context);
/** Returns damage applied for illegal stacking. @param[in] context BTech
 * context. */
int btech_context_stacking_damage(const BtechContext *context);
/** Returns the stagger rules mode. @param[in] context BTech context. */
int btech_context_stagger_mode(const BtechContext *context);
/** Returns the stagger evaluation interval. @param[in] context BTech context.
 */
int btech_context_stagger_interval(const BtechContext *context);
/** Reports whether stagger uses unit tonnage. @param[in] context BTech context.
 */
bool btech_context_stagger_uses_tonnage(const BtechContext *context);
/** Returns the current BTech event tick. @param[in] context BTech context. */
int btech_context_event_tick(const BtechContext *context);
/** Records a hit roll. @param[in,out] context BTech context. @param[in] roll
 * Roll result. */
void btech_context_hit_roll_record(BtechContext *context, int roll);
/** Records a general roll. @param[in,out] context BTech context. @param[in]
 * roll Roll result. */
void btech_context_roll_record(BtechContext *context, int roll);
/** Records a critical-hit roll. @param[in,out] context BTech context.
 * @param[in] roll Roll result. */
void btech_context_critical_roll_record(BtechContext *context, int roll);
/** Generates an unsigned 31-bit random value. @param[in,out] context BTech
 * context and random state. */
long btech_context_random_i31(BtechContext *context);

/** Inputs for a missile hit-table lookup. */
typedef struct MissileHitLookup {
  /** BTech context containing the hit tables. */
  const BtechContext *context;
  /** Weapon catalogue index. */
  int weapon;
  /** Hit-table roll index. */
  int roll;
} MissileHitLookup;

/** Returns a missile hit count. @param[in] lookup Weapon and roll to look up.
 */
int btech_context_missile_hit_count(const MissileHitLookup *lookup);
/** Reports whether a weapon has a missile hit table. @param[in] context BTech
 * context. @param[in] weapon_index Weapon catalogue index. */
bool btech_context_has_missile_hit_table(const BtechContext *context,
                                         int weapon_index);
/** Returns a named weapon's missile hit count. @param[in] context BTech
 * context. @param[in] name Weapon name. @param[in] roll_index Hit-table roll
 * index. */
int btech_context_missile_hit_count_by_name(const BtechContext *context,
                                            const char *name, int roll_index);
/** Returns a weapon's recycle time. @param[in] context BTech context.
 * @param[in] weapon_index Weapon catalogue index. */
int btech_context_weapon_recycle_time(const BtechContext *context,
                                      int weapon_index);
/** Counts matching queued events. @param[in] context BTech context. @param[in]
 * event_type Event type. @param[in] event_data Event payload to match. */
int btech_context_event_data_count(const BtechContext *context, int event_type,
                                   intptr_t event_data);
/** Returns the current runtime time. @param[in] context BTech context. */
time_t btech_context_now(const BtechContext *context);
/** Enters a nested BTech command scope. @param[out] scope Scope to initialize.
 * @param[in] context BTech context. @param[in] command Command context to bind.
 */
void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command);
/** Leaves an active BTech command scope. @param[in,out] scope Scope to restore
 * and deactivate. */
void btech_command_scope_leave(BtechCommandScope *scope);
