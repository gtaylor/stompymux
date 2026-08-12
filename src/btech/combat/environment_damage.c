/* Environmental and terrain-driven unit damage. */

#include "environment_damage_api.h"

#include "artillery_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_ecm_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
void mech_reactor_explode(Mech *wounded, Mech *attacker) {
  BtechContext *context = mech_context(wounded);
  int z = mech_position_z(wounded);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(wounded));
  DbRef wounded_pilot = mech_pilot_dbref(wounded);
  int dam;

  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = CTORSO});
  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = LTORSO});
  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = RTORSO});
  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = LLEG});
  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = RLEG});

  /* Need to autoeject before the explosion reaches the head */
  if (!battle_map_is_underground(map))
    autoeject(wounded_pilot, wounded, 0);

  mech_section_destroy(&(SectionDestructionRequest){
      .wounded = wounded, .attacker = attacker, .section = HEAD});
  mech_position_z_set(wounded, z + 6);
  dam = max(mech_tonnage(wounded) / 5, mech_engine_rating(wounded) / 10);

  mech_sensors_scramble_infrared_and_liteamp(&(SensorScrambleRequest){
      .source = wounded,
      .duration = 4,
      .infrared_message = "The searing blast of heat burns out your sensors!",
      .light_amplification_message =
          "The blinding flash of light overloads your sensors!"});

  BlastRealAreaRequest request = {
      .center =
          {
              .map = map,
              .damage = {.total = dam,
                         .hit_size = 3,
                         .heat = max(mech_tonnage(wounded) / 10,
                                     mech_engine_rating(wounded) / 25)},
              .impact = {.x = mech_position_real_x(wounded),
                         .y = mech_position_real_y(wounded)},
              .source = {.x = mech_position_real_x(wounded),
                         .y = mech_position_real_y(wounded)},
              .messages =
                  {.target =
                       "[fg=red bold]You bear full brunt of the blast![reset]",
                   .observers = "is hit badly by the blast!"},
              .hit_table = btech_context_reactor_explosion_mode(context) > 1,
              .safety = {.above = 3, .below = 5, .underwater = true},
          },
      .neighbor_messages =
          {.target = "[fg=yellow bold]You receive some damage from the "
                     "blast![reset]",
           .observers = "is hit by the blast!"},
      .neighbor_radius = 2,
  };
  blast_hit_real_area(&request);
  mech_position_z_set(wounded, z);
  headhitmwdamage(wounded, attacker, 4);
}

void mech_parts_destroy(Mech *attacker, Mech *wounded, int hitloc, bool breach,
                        bool disable) {
  float oldjs;
  int i;
  int crit_type;
  int nhs = 0;
  int t_do_auto_fall = 0;
  int t_is_leg =
      ((hitloc == RLEG || hitloc == LLEG) ||
       ((hitloc == RARM || hitloc == LARM) && mech_is_quad(wounded)));

  if (!(mech_class(wounded) == CLASS_MECH || mech_class(wounded) == CLASS_MW ||
        mech_class(wounded) == CLASS_BSUIT)) {
    for (i = 0; i < mech_section_critical_count(wounded, hitloc); i++)
      if (mech_critical_part_type(wounded, hitloc, i) &&
          !mech_critical_is_destroyed(wounded, hitloc, i)) {
        if (disable)
          mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
        else
          mech_critical_destroy(wounded, hitloc, i);
      }
    return;
  }
  oldjs = mech_jump_speed(wounded);
  for (i = 0; i < mech_section_critical_count(wounded, hitloc); i++)
    if (!mech_critical_is_destroyed(wounded, hitloc, i)) {
      if (disable) {
        mech_critical_fire_mode_add(wounded, hitloc, i, DISABLED_MODE);
      } else if (mech_critical_is_disabled(wounded, hitloc, i)) {
        mech_critical_destroy(wounded, hitloc, i);
        continue;
      } else {
        mech_critical_destroy(wounded, hitloc, i);
      }

      crit_type = mech_critical_part_type(wounded, hitloc, i);
      if (equipment_is_ammunition(crit_type)) {
        mech_critical_data_set(wounded, hitloc, i, 0);
      }
      if ((equipment_is_special(crit_type))) {
        switch (special_from_equipment_index(crit_type)) {
        case UPPER_ACTUATOR:
        case LOWER_ACTUATOR:
        case HAND_OR_FOOT_ACTUATOR:
          break;
        case SHOULDER_OR_HIP:
          if (t_is_leg) {
            MechConditionSummary condition = mech_condition_summary(wounded);
            if (!condition.hip_damaged) {
              mech_hip_damage_set(wounded, true, false);
            } else {
              if (!mech_is_quad(wounded))
                mech_hip_damage_set(wounded, true, true);
            }
          }
          break;
        case HEAT_SINK:
          if (mech_technology_flags(wounded) & DOUBLE_HEAT_TECH) {
            if ((nhs++) % 3 == 2)
              mech_heat_sink_count_add(wounded, 1);
          }
          mech_heat_sink_count_remove(wounded, 1);
          break;
        case JUMP_JET:
          mech_jump_speed_lower(wounded, MP1);
          if (attacker && mech_jump_speed(wounded) <= 0.0F &&
              mech_is_jumping(wounded)) {
            mech_notify(wounded, MECHALL,
                        "Losing your last Jump Jet you fall from the sky!!!!!");
            mech_los_broadcast(wounded, "falls from the sky!");
            mech_fall(wounded, (int)(oldjs * MP_PER_KPH), 0);
            mech_domino_resolve(wounded, MECH_DOMINO_FALL);
          }
          break;
        case ENGINE:
          if (mech_engine_heat(wounded) < 10) {
            mech_engine_heat_add(wounded, 5);
          } else if (mech_engine_heat(wounded) < 15) {
            mech_engine_heat_set(wounded, 15);
            if (attacker) {
              mech_notify(wounded, MECHALL, "Your engine is destroyed!!");
              if (wounded != attacker)
                mech_notify(attacker, MECHALL, "You destroy the engine!!");
            }
            // check_stackpole(wounded, attacker);

            BtechContext *context = mech_context(wounded);
            if (btech_context_stackpole_enabled(context) &&
                (mech_reactor_instability_start_tick(wounded) +
                 MAX_BOOM_TIME) >= btech_context_event_tick(context) &&
                btech_random_roll(context) >= BOOM_BTH &&
                (mech_is_started(wounded) ||
                 mech_event_count(wounded, EVENT_STARTUP))) {

              hex_los_broadcast(
                  btech_context_get_map(context, mech_map_dbref(wounded)),
                  mech_position_x(wounded), mech_position_y(wounded),
                  "[fg=red bold]The hit destroys the last safety systems, "
                  "releasing the fusion reaction![reset]");

              mech_reactor_explode(wounded, attacker);
            }

            if (mech_class(wounded) == CLASS_MECH &&
                (hitloc == LTORSO || hitloc == RTORSO) &&
                (mech_technology_flags(wounded) & XL_TECH))
              mech_destroy(wounded, attacker, 1,
                           (wounded == attacker) ? KILL_TYPE_SELF_DESTRUCT
                                                 : KILL_TYPE_XLENGINE);
            else
              mech_destroy(wounded, attacker, 1,
                           (wounded == attacker) ? KILL_TYPE_SELF_DESTRUCT
                                                 : KILL_TYPE_NORMAL);
          }
          break;
        case ECM:
          if (!mech_condition_summary(wounded).ecm_destroyed) {
            mech_ecm_destroyed_set(wounded, true);
            mech_notify(wounded, MECHALL,
                        "Your ECM system has been destroyed!");
            mech_ecm_modes_disable(wounded);
            mech_ecm_check(wounded);
          }
          break;
        case TARGETING_COMPUTER:
          if (!mech_condition_summary(wounded).targeting_computer_destroyed) {
            if (attacker)
              mech_notify(wounded, MECHALL,
                          "Your Targeting Computer is Destroyed");
            mech_targeting_computer_destroyed_set(wounded, true);
          }
          break;
        }
      }
    }
  if (breach)
    if (mech_class(wounded) == CLASS_VEH_GROUND ||
        mech_class(wounded) == CLASS_VEH_NAVAL)
      mech_destroy(wounded, attacker, 0, KILL_TYPE_NORMAL);
  if (mech_class(wounded) == CLASS_MECH || mech_class(wounded) == CLASS_MW) {
    if (breach && hitloc == HEAD) {
      if (mech_is_under_vacuum(wounded))
        mech_notify(wounded, MECHALL, "You are exposed to vacuum!");
      else
        mech_notify(wounded, MECHALL, "Water floods into your cockpit!");

      mech_contents_kill_if_in_character(wounded);
      mech_destroy(wounded, attacker, 0, KILL_TYPE_FLOOD);
      return;
    }
    if (!mech_is_quad(wounded))
      if (hitloc == LARM || hitloc == RARM)
        return;
    if (hitloc == RLEG || hitloc == LLEG || hitloc == LARM || hitloc == RARM) {
      t_do_auto_fall = 1;
      mech_event_cancel(wounded, EVENT_STAND);
    }
    mech_actuator_criticals_normalize(wounded);
    if (t_is_leg && !mech_is_fallen(wounded) && !mech_is_jumping(wounded) &&
        !mech_is_out_of_control(wounded) && attacker) {
      if (t_do_auto_fall) {
        mech_notify(wounded, MECHALL,
                    "You realize remaining standing is no longer an option and "
                    "crash to the ground!");
        mech_los_broadcast(wounded, "crashes to the ground!");
        mech_fall(wounded, 1, 0);
      } else if (!made_pilot_skill_roll(wounded, 0)) {
        mech_notify(wounded, MECHALL, "You lose your balance and fall down!");
        mech_los_broadcast(wounded, "loses balance and falls down!");
        mech_fall(wounded, 1, 0);
      }
    }
  }
}

int mech_location_breach(Mech *attacker, Mech *mech, int hitloc) {
  char buf[SBUF_SIZE];

  if (!mech_is_under_special_conditions(mech))
    return 0;
  if (!mech_is_under_vacuum(mech))
    return 0;
  if (mech_section_is_destroyed(mech, hitloc) ||
      mech_section_is_breached(mech, hitloc))
    return 0;
  armor_string_from_index(hitloc, buf, mech_class(mech),
                          mech_movement_type(mech));
  mech_notify(mech, MECHALL, tprintf("Your %s has been breached!", buf));
  mech_section_breached_set(mech, hitloc, true);
  mech_parts_destroy(attacker, mech, hitloc, true, true);
  return 1;
}

int mech_location_maybe_breach(Mech *attacker, Mech *mech, int hitloc) {
  if (!mech_is_under_special_conditions(mech))
    return 0;
  if (btech_random_roll(mech_context(mech)) < 10)
    return 0;
  return mech_location_breach(attacker, mech, hitloc);
}
