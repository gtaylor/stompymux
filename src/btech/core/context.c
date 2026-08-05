/* context.c - Runtime dependency bridge for legacy BTech callbacks. */

#include "btech/context.h"

#include <assert.h>
#include <stdlib.h>

#include "mux/commands/command_context.h"
#include "mux/network/mux_event.h"
#include "mux/server/runtime_clock.h"
#include "mux/server/server_config.h"

BtechContext *btech_context_create(const BtechDependencies *dependencies) {
  if (dependencies == nullptr)
    return nullptr;
  BtechContext *context = calloc(1, sizeof(*context));
  if (context == nullptr)
    return nullptr;
  *context = (BtechContext){
      .dependencies = *dependencies,
      .configuration = dependencies->configuration,
      .clock = dependencies->clock,
      .background_command = dependencies->background_command,
      .database = dependencies->database,
      .events = dependencies->events,
      .lifecycle = dependencies->lifecycle,
      .log = dependencies->log,
      .persistence = dependencies->persistence,
      .world_indexes = dependencies->world_indexes,
      .access_control = dependencies->access_control,
      .process_start_time = dependencies->process_start_time,
      .cached_target_character = -1,
  };
  return context;
}

void btech_context_destroy(BtechContext *context) {
  if (context == nullptr)
    return;
  btech_context_release_owned_state(context);
  free(context);
}

void btech_context_set_lifecycle(BtechContext *context,
                                 ServerLifecycle *lifecycle) {
  if (context == nullptr)
    return;
  context->dependencies.lifecycle = lifecycle;
  context->lifecycle = lifecycle;
}

void btech_context_set_process_start_time(BtechContext *context,
                                          time_t process_start_time) {
  assert(context != nullptr);
  context->dependencies.process_start_time = process_start_time;
  context->process_start_time = process_start_time;
}

CommandContext *btech_context_command(BtechContext *context) {
  assert(context != nullptr);
  return context->command_scope != nullptr ? context->command_scope->command
                                           : context->background_command;
}

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return &btech_context_command(context)->evaluation;
}

GameDatabase *btech_context_database(BtechContext *context) {
  assert(context != nullptr);
  return context->database;
}

BtechObjectId btech_context_afterlife_dbref(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->afterlife_dbref;
}

bool btech_context_in_character_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_ic;
}

int btech_context_experience_loss(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_xploss;
}

bool btech_context_mechwarrior_experience_loss_enabled(
    const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_xploss_for_mw;
}

bool btech_context_transported_unit_death_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_transported_unit_death;
}

bool btech_context_vtol_ice_fire_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_vtol_ice_causes_fire;
}

bool btech_context_combat_arcs_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->combat_overrides.arcs;
}

void btech_context_combat_arcs_override_set(BtechContext *context, int arcs) {
  assert(context != nullptr);
  context->combat_overrides.arcs = arcs;
}

void btech_context_combat_pilot_override_set(BtechContext *context,
                                             BtechObjectId pilot) {
  assert(context != nullptr);
  context->combat_overrides.pilot = pilot;
}

bool btech_context_seismic_detects_stopped_units(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_seismic_see_stopped;
}

bool btech_context_inferno_penalty_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_inferno_penalty;
}

bool btech_context_uses_fasa_turning(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_fasaturn;
}

bool btech_context_uses_extended_movement_modifiers(
    const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_extendedmovemod;
}

bool btech_context_uses_extended_weapon_ranges(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_erange;
}

bool btech_context_hotload_uses_half_modifier(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_hotloadaddshalfbthmod;
}

bool btech_context_overrides_weapon_arcs(const BtechContext *context) {
  assert(context != nullptr);
  return context->combat_overrides.arcs;
}

bool btech_context_dig_bonus_requires_front(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_dig_only_fs;
}

int btech_context_dig_bonus(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_digbonus;
}

bool btech_context_woods_modify_damage(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_moddamagewithwoods;
}

int btech_context_sprint_to_hit_modifier(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_sprint_bth;
}

bool btech_context_uses_skid_cliff_rules(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_skidcliff;
}

bool btech_context_uses_roll_on_backwalk(const BtechContext *context) {
  return context && context->configuration &&
         context->configuration->btech_roll_on_backwalk;
}

bool btech_context_uses_new_terrain_rules(const BtechContext *context) {
  return context && context->configuration &&
         context->configuration->btech_newterrain;
}

bool btech_context_uses_advanced_vehicle_fire(const BtechContext *context) {
  return context && context->configuration &&
         context->configuration->btech_fasaadvvhlfire;
}

bool btech_context_uses_advanced_vehicle_criticals(
    const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_fasaadvvhlcrit;
}

bool btech_context_uses_advanced_vtol_criticals(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_fasaadvvtolcrit;
}

bool btech_context_uses_fasa_criticals(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_fasacrit;
}

bool btech_context_uses_exile_stun_code(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_exile_stun_code;
}

int btech_context_exile_stun_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_exile_stun_code;
}

int btech_context_critical_level(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_critlevel;
}

int btech_context_hit_arc_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_hit_arcs;
}

int btech_context_vehicle_critical_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_vcrit;
}

bool btech_context_uses_tank_friendly_criticals(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_tankfriendly;
}

bool btech_context_uses_tank_critical_shielding(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_tankshield;
}

bool btech_context_uses_tsm_sprint_bonus(const BtechContext *context) {
  return context && context->configuration &&
         context->configuration->btech_tsm_sprint_bonus;
}

bool btech_context_uses_tsm_tow_bonus(const BtechContext *context) {
  return context && context->configuration &&
         context->configuration->btech_tsm_tow_bonus;
}

int btech_context_stand_careful_modifier(const BtechContext *context) {
  return context && context->configuration
             ? context->configuration->btech_standcareful
             : 0;
}

int btech_context_landing_zone_mode(const BtechContext *context) {
  return context && context->configuration
             ? context->configuration->btech_blzmapmode
             : 0;
}

int btech_context_self_destruct_time(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_explode_time;
}

bool btech_context_self_destruct_can_stop(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_explode_stop;
}

bool btech_context_self_destruct_ammunition_enabled(
    const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_explode_ammo;
}

bool btech_context_self_destruct_reactor_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_explode_reactor;
}

int btech_context_reactor_explosion_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_explode_reactor;
}

bool btech_context_stackpole_enabled(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_stackpole;
}

bool btech_context_requires_backwalk_rolls(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_roll_on_backwalk;
}

bool btech_context_uses_new_charge_rules(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_newcharge;
}

int btech_context_movement_slowdown_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_slowdown;
}

int btech_context_stacking_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_stacking;
}

int btech_context_stacking_damage(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_stackdamage;
}

int btech_context_stagger_mode(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_newstagger;
}

int btech_context_stagger_interval(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_newstaggertime;
}

bool btech_context_stagger_uses_tonnage(const BtechContext *context) {
  assert(context != nullptr);
  return context->configuration->btech_newstaggertons;
}

int btech_context_event_tick(const BtechContext *context) {
  assert(context != nullptr);
  return context->events->tick;
}

void btech_context_hit_roll_record(BtechContext *context, int roll) {
  assert(context != nullptr);
  assert(roll >= 2 && roll <= 12);
  context->random.statistics.hit_rolls[roll - 2]++;
  context->random.statistics.total_hit_rolls++;
}

void btech_context_critical_roll_record(BtechContext *context, int roll) {
  assert(context != nullptr);
  context->random.statistics.critical_rolls[roll - 2]++;
  context->random.statistics.total_critical_rolls++;
}

long btech_context_random_i31(BtechContext *context) {
  assert(context != nullptr);
  return btech_random_i31(&context->random);
}

int btech_context_missile_hit_count(const BtechContext *context,
                                    int weapon_index, int roll_index) {
  assert(context != nullptr);
  const MissileHitEntry *entry =
      missile_hit_registry_find_weapon(&context->missile_hits, weapon_index);
  return entry ? entry->num_missiles[roll_index] : 0;
}

int btech_context_event_data_count(const BtechContext *context, int event_type,
                                   intptr_t event_data) {
  assert(context != nullptr);
  return mux_event_count_type_data2(context->events, event_type,
                                    (void *)event_data);
}

time_t btech_context_now(const BtechContext *context) {
  assert(context != nullptr);
  return context->clock->now;
}

void btech_command_scope_enter(BtechCommandScope *scope, BtechContext *context,
                               CommandContext *command) {
  assert(scope != nullptr);
  assert(context != nullptr);
  assert(command != nullptr);
  *scope = (BtechCommandScope){
      .context = context,
      .command = command,
      .previous = context->command_scope,
      .active = true,
  };
  context->command_scope = scope;
}

void btech_command_scope_leave(BtechCommandScope *scope) {
  assert(scope != nullptr);
  assert(scope->active);
  assert(scope->context->command_scope == scope);
  scope->context->command_scope = scope->previous;
  *scope = (BtechCommandScope){0};
}
