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
#include "legacy_macros.h"
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
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
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

static int mech_hull_down_change_delay(const Mech *mech) {
  return 30 / BOUNDED(1, mech_maximum_speed(mech) / MP2, 30);
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

  cch(MECH_USUALO);
  condition = mech_condition_summary(mech);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(context, mech_movement_type(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(context, mech_carried_dbref(mech) > 0,
                  "You cannot sprint while towing!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(context, mech_is_jumping(mech),
                  "You cannot do this while jumping.");
  DOCHECK_CONTEXT(context,
                  mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(context,
                  battle_terrain_is_water(mech_real_terrain_get(mech)) &&
                      mech_position_z(mech) < 0 && !mech_is_water_beast(mech) &&
                      !condition.sprinting,
                  "You cannot start sprinting while in water!");
  DOCHECK_CONTEXT(
      context, mech_is_water_beast(mech) && !mech_is_on_water(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(context, condition.evading || condition.dodging,
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(context, condition.swarm_target > 0,
                  "You cannot sprint while mounted!");
  if (mech_class(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(context,
                    mech_section_is_destroyed(mech, RLEG) ||
                        mech_section_is_destroyed(mech, LLEG) ||
                        (mech_movement_type(mech) != MOVE_QUAD
                             ? 0
                             : mech_section_is_destroyed(mech, RLEG) ||
                                   mech_section_is_destroyed(mech, LLEG)),
                    "That's kind of hard while limping.");

  DOCHECK_CONTEXT(
      context, mech_charge_target_dbref(mech) > 0,
      "You are currently charging a target and unable to start sprinting!");

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

  cch(MECH_USUALO);
  condition = mech_condition_summary(mech);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(context, mech_movement_type(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(context, mech_is_jumping(mech),
                  "You cannot do this while jumping.");
  DOCHECK_CONTEXT(context,
                  mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(context, mech_carried_dbref(mech) > 0,
                  "You can't do that while towing");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(context,
                  !condition.evading && mech_class(mech) == CLASS_MECH &&
                      (mech_critical_is_nonfunctional(mech, LLEG, 0) ||
                       mech_critical_is_nonfunctional(mech, RLEG, 0)),
                  "You need both hips functional to evade.");
  DOCHECK_CONTEXT(
      context, mech_is_water_beast(mech) && !mech_is_on_water(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(context, condition.sprinting || condition.dodging,
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(context, condition.swarm_target > 0,
                  "You cannot evade while mounted!");
  DOCHECK_CONTEXT(context, mech_charge_target_dbref(mech) > 0,
                  "You cannot evade while charging!");

  if (mech_class(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(context,
                    mech_section_is_destroyed(mech, RLEG) ||
                        mech_section_is_destroyed(mech, LLEG) ||
                        (mech_movement_type(mech) != MOVE_QUAD
                             ? 0
                             : mech_section_is_destroyed(mech, RLEG) ||
                                   mech_section_is_destroyed(mech, LLEG)),
                    "That's kind of hard while limping.");

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

  cch(MECH_USUALO);
  condition = mech_condition_summary(mech);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(context, mech_movement_type(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(context,
                  mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(
      context, mech_is_water_beast(mech) && !mech_is_on_water(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(context, condition.sprinting || condition.evading,
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(
      context,
      !HasBoolAdvantage(context, player, "dodge_maneuver") ||
          player != mech_pilot_dbref(mech),
      "You either are not the pilot of this mech, have no Dodge Maneuver "
      "adavantage, or both.");
  DOCHECK_CONTEXT(context, mech_charge_target_dbref(mech) > 0,
                  "You cannot dodge while charging!");

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

  cch(MECH_USUALO);
  condition = mech_condition_summary(mech);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(context, mech_movement_type(mech) != MOVE_QUAD,
                  "Only QUADs can hulldown.");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You can't hulldown from a FALLEN position");
  DOCHECK_CONTEXT(context, mech_is_jumping(mech),
                  "You can't hulldown while jumping!");
  DOCHECK_CONTEXT(context, mech_current_speed(mech) > 0.5F,
                  "You can't hulldown while moving!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_JUMPSTABIL),
                  "You are still stabilizing from your last jump.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You haven't finished standing up yet.");

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

  DOCHECK_CONTEXT(context, condition.hull_down, "You are already hulldown.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode.");

  mech_notify(mech, MECHALL, "You start to lower yourself to the ground.");
  mech_los_broadcast(mech, "begins to lower itself to the ground.");
  mech_desired_speed_set(mech, 0.0F);

  mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                      mech_hull_down_change_delay(mech), 1);
}
