/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

#ifdef BT_MOVEMENT_MODES
static bool mech_is_water_beast(const Mech *mech) {
  return mech_movement_type(mech) == MOVE_HULL ||
         mech_movement_type(mech) == MOVE_FOIL;
}

static bool mech_is_on_water(Mech *mech) {
  return battle_terrain_is_water(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) <= 0;
}

static int mech_movement_mode_delay(const Mech *mech) {
  return mech_class(mech) == CLASS_BSUIT || mech_class(mech) == CLASS_MW
             ? TURN / 2
             : TURN;
}
#endif

static int mech_hull_down_change_delay(const Mech *mech) {
  const float speed_factor = mech_maximum_speed(mech) / MP2;
  const float bounded_factor = fminf(fmaxf(1.0F, speed_factor), 30.0F);
  const float delay = 30.0F / bounded_factor;
  return (int)delay;
}
static void mech_hulldown_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long type = (long)e->data2;

  if (!mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
    return;

  if (!mech_is_started(mech))
    return;

  if (type == 0) {
    mech_hull_down_set(mech, false);
    mech_notify(mech, MECHALL, "You finish lifting yourself up.");
    mech_los_broadcast(mech, "finishes lifting itself up");
  } else {
    mech_hull_down_set(mech, true);
    mech_notify(mech, MECHALL, "You finish lowering yourself to the ground.");
    mech_los_broadcast(mech, "finishes lowering itself to the ground.");
  }
}

#ifdef BT_MOVEMENT_MODES
void mech_sprint(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While falling out of the sky?");
    return;
  }
  if (mech_movement_type(mech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This piece of equipment is stationary!");
    return;
  }
  if (mech_carried_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot sprint while towing!");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently standing up and cannot move.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot do this while jumping.");
    return;
  }
  if (mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
      mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your vehicle's movement system is destroyed.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently prone and cannot move.");
    return;
  }
  if (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
      mech_position_z(mech) < 0 && !mech_is_water_beast(mech) &&
      !condition.sprinting) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot start sprinting while in water!");
    return;
  }
  if (mech_is_water_beast(mech) && !mech_is_on_water(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    return;
  }
  if (mech_event_count(mech, EVENT_MOVEMODE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already changing movement modes!");
    return;
  }
  if (condition.evading || condition.dodging) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot perform multiple movement modes!");
    return;
  }
  if (condition.swarm_target > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot sprint while mounted!");
    return;
  }
  if (mech_class(mech) == CLASS_MECH)
    if (mech_section_is_destroyed(mech, RLEG) ||
        mech_section_is_destroyed(mech, LLEG) ||
        (mech_movement_type(mech) != MOVE_QUAD
             ? 0
             : mech_section_is_destroyed(mech, RLEG) ||
                   mech_section_is_destroyed(mech, LLEG))) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That's kind of hard while limping.");
      return;
    }

  if (mech_charge_target_dbref(mech) > 0) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are currently charging a target and unable to start sprinting!");
    return;
  }

  d |= MODE_SPRINT | (condition.sprinting ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = mech_recycling_state(mech, CHECK_BOTH)) > 0) {
      mech_printf(mech, MECHALL, "You have %s recycling!",
                  (i == 1   ? "weapons"
                   : i == 2 ? "limbs"
                            : "error"));
      return;
    }
    mech_notify(mech, MECHALL, "You begin the process of sprinting...");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_movement_mode_delay(mech), d);
  } else {
    mech_notify(mech, MECHALL, "You begin the process of ceasing to sprint.");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_movement_mode_delay(mech), d);
  }
  return;
}

void mech_evade(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While falling out of the sky?");
    return;
  }
  if (mech_movement_type(mech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This piece of equipment is stationary!");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently standing up and cannot move.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot do this while jumping.");
    return;
  }
  if (mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
      mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your vehicle's movement system is destroyed.");
    return;
  }
  if (mech_carried_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't do that while towing");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently prone and cannot move.");
    return;
  }
  if (!condition.evading && mech_class(mech) == CLASS_MECH &&
      (mech_critical_is_nonfunctional(mech, LLEG, 0) ||
       mech_critical_is_nonfunctional(mech, RLEG, 0))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need both hips functional to evade.");
    return;
  }
  if (mech_is_water_beast(mech) && !mech_is_on_water(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    return;
  }
  if (mech_event_count(mech, EVENT_MOVEMODE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already changing movement modes!");
    return;
  }
  if (condition.sprinting || condition.dodging) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot perform multiple movement modes!");
    return;
  }
  if (condition.swarm_target > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot evade while mounted!");
    return;
  }
  if (mech_charge_target_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot evade while charging!");
    return;
  }

  if (mech_class(mech) == CLASS_MECH)
    if (mech_section_is_destroyed(mech, RLEG) ||
        mech_section_is_destroyed(mech, LLEG) ||
        (mech_movement_type(mech) != MOVE_QUAD
             ? 0
             : mech_section_is_destroyed(mech, RLEG) ||
                   mech_section_is_destroyed(mech, LLEG))) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That's kind of hard while limping.");
      return;
    }

  d |= MODE_EVADE | (condition.evading ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = mech_recycling_state(mech, CHECK_BOTH)) > 0) {
      mech_printf(mech, MECHALL, "You have %s recycling!",
                  (i == 1   ? "weapons"
                   : i == 2 ? "limbs"
                            : "error"));
      return;
    }
    mech_notify(mech, MECHALL, "You begin the process of evading...");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_movement_mode_delay(mech), d);
  } else {
    mech_notify(mech, MECHALL, "You begin the process of ceasing to evade.");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event,
                        mech_movement_mode_delay(mech), d);
  }
  return;
}

void mech_dodge(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While falling out of the sky?");
    return;
  }
  if (mech_movement_type(mech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This piece of equipment is stationary!");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently standing up and cannot move.");
    return;
  }
  if (mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
      mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your vehicle's movement system is destroyed.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are currently prone and cannot move.");
    return;
  }
  if (mech_is_water_beast(mech) && !mech_is_on_water(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    return;
  }
  if (mech_event_count(mech, EVENT_MOVEMODE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already changing movement modes!");
    return;
  }
  if (condition.sprinting || condition.evading) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot perform multiple movement modes!");
    return;
  }
  if (!HasBoolAdvantage(context, player, "dodge_maneuver") ||
      player != mech_pilot_dbref(mech)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You either are not the pilot of this mech, have no Dodge Maneuver "
        "adavantage, or both.");
    return;
  }
  if (mech_charge_target_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot dodge while charging!");
    return;
  }

  d |= MODE_DODGE | (condition.dodging ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = mech_recycling_state(mech, CHECK_PHYS)) > 0) {
      mech_printf(mech, MECHALL, "You have %s recycling!",
                  (i == 1   ? "weapons"
                   : i == 2 ? "limbs"
                            : "error"));
      return;
    }
    mech_notify(mech, MECHALL, "You begin the process of dodging...");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event, 1, d);
  } else {
    mech_notify(mech, MECHALL, "You begin the process of ceasing to dodge.");
    mech_event_schedule(mech, EVENT_MOVEMODE, mech_movemode_event, TURN, d);
  }
  return;
}
#endif

void mech_hulldown(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  int argc;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While falling out of the sky?");
    return;
  }
  if (mech_movement_type(mech) != MOVE_QUAD) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Only QUADs can hulldown.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown from a FALLEN position");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown while jumping!");
    return;
  }
  if (mech_current_speed(mech) > 0.5F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown while moving!");
    return;
  }
  if (mech_event_count(mech, EVENT_JUMPSTABIL)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are still stabilizing from your last jump.");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You haven't finished standing up yet.");
    return;
  }

  argc = mech_parseattributes(buffer, args, 1);

  if (argc > 0) {
    if (!strcmp(args[0], "-")) {
      if (!condition.hull_down)
        mech_notify(mech, MECHALL, "You are not hulldown.");
      else if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
        mech_notify(mech, MECHALL, "You are busy changing your hulldown mode.");
      else {
        mech_notify(mech, MECHALL, "You start to lift yourself up.");
        mech_los_broadcast(mech, "begins to raise up on its legs.");

        mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                            mech_hull_down_change_delay(mech), 0);
      }
    } else if (!strcasecmp(args[0], "stop")) {
      if (!mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
        mech_notify(mech, MECHALL,
                    "You are not currently changing your hulldown mode.");
      else {
        mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
        mech_notify(mech, MECHALL, "You stop changing your hulldown mode.");
      }
    } else
      mech_notify(mech, MECHALL, "Invalid argument for 'hulldown'.");

    return;
  }

  if (condition.hull_down) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already hulldown.");
    return;
  }
  if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are busy changing your hulldown mode.");
    return;
  }

  mech_notify(mech, MECHALL, "You start to lower yourself to the ground.");
  mech_los_broadcast(mech, "begins to lower itself to the ground.");
  mech_desired_speed_set(mech, 0.0F);

  mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                      mech_hull_down_change_delay(mech), 1);
}
