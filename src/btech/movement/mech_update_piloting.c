/* Implements BattleTech movement mechanics for unit update piloting. */

#include "equipment_types.h"
#include "mech_update_api.h"

#include <math.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "failures_api.h"
#include "map_coordinates.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_damage_api.h"
#include "mech_damage_history_api.h"
#include "mech_equipment_api.h"
#include "mech_heat_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"
#include "section_types.h"

static bool mech_piloting_is_running(const Mech *mech, float maximum_speed) {
  return mech_current_speed(mech) > (2.0F * maximum_speed / 3.0F) + 0.1F;
}

void mech_piloting_update(Mech *mech) {
  BtechContext *context = mech_context(mech);
  int makeroll = 0;
  int grav = 0;
  float maxspeed = mech_effective_maximum_speed(mech);
  int temp_tick = btech_context_event_tick(context);

  /* Preserve the legacy even-tick alignment before the per-turn checks. */
  if ((temp_tick & 1) != 0)
    temp_tick++;

  if (temp_tick % TURN == 0 && !mech_is_fallen(mech) &&
      !mech_is_jumping(mech) && !mech_is_out_of_control(mech)) {
    if (!mech_is_started(mech))
      makeroll = 4;

    if (mech_excess_heat(mech) >= 9.0F &&
        (mech_technology_flags(mech) & TRIPLE_MYOMER_TECH)) {
      maxspeed =
          ceilf((rintf((mech_effective_maximum_speed(mech) / 1.5F) / MP1) +
                 1.0F) *
                1.5F) *
          MP1;
    }
    if (mech_is_under_special_conditions(mech) && mech_is_under_gravity(mech)) {
      if (mech_current_speed(mech) > mech_maximum_speed(mech) &&
          mech_class(mech) == CLASS_MECH) {
        grav = 1;
        makeroll = 1;
      }
    }
  }

  MechConditionSummary condition = mech_condition_summary(mech);
  if (mech_piloting_is_running(mech, maxspeed) &&
      (condition.gyro_damaged || condition.hip_damaged))
    makeroll = 1;

  if (makeroll && !made_pilot_skill_roll(mech, makeroll - 1)) {
    if (grav) {
      int dam =
          clamp_float_to_int(
              (mech_current_speed(mech) - mech_maximum_speed(mech)) / MP1) +
          1;
      mech_notify(mech, MECHALL, "Your legs take some damage!");
      if (mech_movement_type(mech) == MOVE_QUAD) {
        if (!mech_section_is_destroyed(mech, LARM)) {
          mech_damage_apply(&(MechDamageRequest){.target = mech,
                                                 .attacker = mech,
                                                 .line_of_sight = false,
                                                 .attack_pilot = -1,
                                                 .hit_location = LARM,
                                                 .rear = false,
                                                 .critical = false,
                                                 .armor_damage = 0,
                                                 .internal_damage = dam,
                                                 .transfer = MECH_DAMAGE_NORMAL,
                                                 .cause = 0,
                                                 .base_to_hit = 0,
                                                 .weapon_index = -1,
                                                 .ammunition_mode = 0,
                                                 .ignore_swarmers = true});
        }
        if (!mech_section_is_destroyed(mech, RARM)) {
          mech_damage_apply(&(MechDamageRequest){.target = mech,
                                                 .attacker = mech,
                                                 .line_of_sight = false,
                                                 .attack_pilot = -1,
                                                 .hit_location = RARM,
                                                 .rear = false,
                                                 .critical = false,
                                                 .armor_damage = 0,
                                                 .internal_damage = dam,
                                                 .transfer = MECH_DAMAGE_NORMAL,
                                                 .cause = 0,
                                                 .base_to_hit = 0,
                                                 .weapon_index = -1,
                                                 .ammunition_mode = 0,
                                                 .ignore_swarmers = true});
        }
      }
      if (!mech_section_is_destroyed(mech, LLEG)) {
        mech_damage_apply(&(MechDamageRequest){.target = mech,
                                               .attacker = mech,
                                               .line_of_sight = false,
                                               .attack_pilot = -1,
                                               .hit_location = LLEG,
                                               .rear = false,
                                               .critical = false,
                                               .armor_damage = 0,
                                               .internal_damage = dam,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = 0,
                                               .base_to_hit = 0,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = true});
      }
      if (!mech_section_is_destroyed(mech, RLEG)) {
        mech_damage_apply(&(MechDamageRequest){.target = mech,
                                               .attacker = mech,
                                               .line_of_sight = false,
                                               .attack_pilot = -1,
                                               .hit_location = RLEG,
                                               .rear = false,
                                               .critical = false,
                                               .armor_damage = 0,
                                               .internal_damage = dam,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = 0,
                                               .base_to_hit = 0,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = true});
      }
    } else {
      mech_notify(mech, MECHALL, "Your damaged mech falls as you try to run!");
      mech_los_broadcast(mech, "falls down.");
      mech_fall(mech, 1, false);
    }
  }
  if (mech_class(mech) == CLASS_MECH)
    mech_damage_stagger_check(mech);
  else
    mech_turn_damage_clear(mech);
  if (temp_tick % TURN == 0 && mech_is_started(mech) &&
      mech_movement_type(mech) != MOVE_NONE)
    (void)mech_generic_failure_check(mech, FAILURE_SYSTEM_COMPUTER);
}

void mech_turret_autoturn_update(Mech *mech) {
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  Mech *target;
  int bearing;
  float fx;
  float fy;

  if (!mech_is_started(mech) || mech_pilot_is_unconscious(mech) ||
      mech_is_blinded(mech))
    return;

  if (condition.turret_jammed || condition.turret_locked)
    return;

  if (!mech_section_internal(mech, TURRET))
    return;

  if (mech_target_dbref(mech) == -1 &&
      (mech_target_hex_y(mech) == -1 || mech_target_hex_x(mech) == -1))
    return;

  if (mech_target_dbref(mech) != -1) {
    target = btech_context_get_mech(context, mech_target_dbref(mech));
    fx = mech_position_real_x(target);
    fy = mech_position_real_y(target);
  } else {
    map_coord_to_real_coord(mech_target_hex_x(mech), mech_target_hex_y(mech),
                            &fx, &fy);
  }

  bearing = acceptable_degree(
      map_bearing(&(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                              .y = mech_position_real_y(mech)},
                                    .end = {.x = fx, .y = fy}}) -
      mech_heading_degrees(mech));
  mech_turret_heading_relative_set(mech, bearing);
  mark_for_los_update(mech);
}

/* This function is called once every second for every mech in the game */
