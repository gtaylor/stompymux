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
#include "legacy_macros.h"
#include "map.h"
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
#include "mech_status_types.h"
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

static bool mech_control_requires_water(const Mech *mech) {
  return mech_movement_type(mech) == MOVE_HULL ||
         mech_movement_type(mech) == MOVE_FOIL;
}

static bool mech_control_is_on_water(Mech *mech) {
  return battle_terrain_is_water(mech_real_terrain_get(mech)) &&
         mech_position_z(mech) <= 0;
}

static bool mech_control_is_rolling(const Mech *mech) {
  return mech_class(mech) == CLASS_AERO || mech_class(mech) == CLASS_DS;
}

static bool mech_control_is_flying(const Mech *mech) {
  return mech_is_aerospace_unit(mech) || mech_movement_type(mech) == MOVE_VTOL;
}

static float mech_control_walking_speed(float maximum_speed) {
  return 2.0F * maximum_speed / 3.0F;
}

static bool mech_control_is_running(float speed, float maximum_speed) {
  return speed > mech_control_walking_speed(maximum_speed) + 0.1F;
}

/* Facing related */
void mech_heading(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  char *args[1];
  int newheading;

  cch(MECH_USUAL);
  if (mech_parseattributes(buffer, args, 1) == 1) {
    MechConditionSummary condition = mech_condition_summary(mech);
    DOCHECK_CONTEXT(context, mech_movement_type(mech) == MOVE_NONE,
                    "This piece of equipment is stationary!");
    DOCHECK_CONTEXT(context, condition.fortified,
                    "Your fortified state prevents you from moving.");
    DOCHECK_CONTEXT(
        context,
        mech_control_requires_water(mech) && !mech_control_is_on_water(mech),
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    DOCHECK_CONTEXT(context,
                    mech_is_aerospace_unit(mech) && condition.spinning &&
                        !mech_is_landed(mech),
                    "You are unable to control your craft at the moment.");
    DOCHECK_CONTEXT(context, condition.performing_action,
                    "You are too busy at the moment to turn.");
    DOCHECK_CONTEXT(context, condition.dug_in,
                    "You are in a hole you dug, unable to move [use "
                    "speed cmd to get out].");
    DOCHECK_CONTEXT(context, condition.hull_down,
                    "You can not turn while hulldown");
    DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                    "You are busy changing your hulldown mode");
    if (condition.digging) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    newheading = AcceptableDegree(atoi(args[0]));
    mech_desired_heading_set(mech, newheading);
    mech_printf(mech, MECHALL, "Heading changed to %d.", newheading);
    mech_maybe_move(mech);
  } else {
    notify_printf(btech_context_evaluation(context), player,
                  "Your current heading is %i.", mech_heading_degrees(mech));
  }
}

void mech_turret(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[1];
  int newheading;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(
      context,
      mech_class(mech) == CLASS_MECH || mech_class(mech) == CLASS_MW ||
          mech_class(mech) == CLASS_BSUIT || mech_is_aerospace_unit(mech) ||
          !mech_section_internal(mech, TURRET),
      "You don't have a turret.");
  DOCHECK_CONTEXT(context, condition.turret_jammed,
                  "Your turret is jammed in position.");
  DOCHECK_CONTEXT(context, condition.turret_locked,
                  "Your turret is locked in position.");
  if (mech_parseattributes(buffer, args, 1) == 1) {
    newheading = AcceptableDegree(atoi(args[0]));
    mech_turret_heading_absolute_set(mech, newheading);
    mech_printf(mech, MECHALL, "Turret facing changed to %d.",
                mech_turret_heading_absolute(mech));
  } else {
    notify_printf(btech_context_evaluation(context), player,
                  "Your turret is currently facing %d.",
                  mech_turret_heading_absolute(mech));
  }

  MarkForLOSUpdate(mech);
}

void mech_rotatetorso(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[2];

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(context, mech_class(mech) == CLASS_BSUIT, "Huh?");
  DOCHECK_CONTEXT(
      context, mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW,
      "You don't have a torso.");
  DOCHECK_CONTEXT(
      context, mech_is_fallen(mech),
      "You're lying flat on your face, you can't rotate your torso.");
  DOCHECK_CONTEXT(context,
                  mech_class(mech) == CLASS_MECH &&
                      mech_movement_type(mech) == MOVE_QUAD,
                  "Quads can't rotate their torsos.");
  if (mech_parseattributes(buffer, args, 2) == 1) {
    switch (args[0][0]) {
    case 'L':
    case 'l':
      DOCHECK_CONTEXT(context, condition.torso_left,
                      "You cannot rotate torso beyond 60 degrees!");
      if (condition.torso_right)
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      else
        mech_torso_twist_set(mech, MECH_TORSO_LEFT);
      mech_notify(mech, MECHALL, "You rotate your torso left.");
      break;
    case 'R':
    case 'r':
      DOCHECK_CONTEXT(context, condition.torso_right,
                      "You cannot rotate torso beyond 60 degrees!");
      if (condition.torso_left)
        mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      else
        mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
      mech_notify(mech, MECHALL, "You rotate your torso right.");
      break;
    case 'C':
    case 'c':
      mech_torso_twist_set(mech, MECH_TORSO_CENTER);
      mech_notify(mech, MECHALL, "You center your torso.");
      break;
    default:
      notify(btech_context_evaluation(context), player,
             "Rotate must have LEFT RIGHT or CENTER.");
      break;
    }
  } else
    notify(btech_context_evaluation(context), player,
           "Invalid number of arguments!");
  MarkForLOSUpdate(mech);
}

static const struct MechSpeedName {
  const char *name;
  int flag;
} speed_tables[] = {{"walk", 1},   {"run", 2},   {"stop", 0}, {"back", -1},
                    {"cruise", 1}, {"flank", 2}, {nullptr, 0}};

void mech_speed(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition = mech_condition_summary(mech);
  char *args[1];
  float newspeed, walkspeed, maxspeed;
  int i;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
  if (mech_control_is_rolling(mech)) {
    DOCHECK_CONTEXT(context, !mech_is_landed(mech),
                    "Use thrust command instead!");
  } else if (mech_control_is_flying(mech)) {
    DOCHECK_CONTEXT(context, mech_class(mech) != CLASS_VTOL,
                    "Use thrust command instead!");
  }
  DOCHECK_CONTEXT(context, mech_movement_type(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(context, condition.performing_action,
                  "You are too busy at the moment to turn.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(context,
                  mech_is_fallen(mech) && mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(
      context,
      mech_control_requires_water(mech) && !mech_control_is_on_water(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");

  if (mech_class(mech) != CLASS_MECH)
    DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_REMOVE_PODS),
                    "You are too busy removing iNARC pods!");
  DOCHECK_CONTEXT(context, condition.hull_down,
                  "You can not move while hulldown");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");

  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify_printf(btech_context_evaluation(context), player,
                  "Your current speed is %.2f.", mech_current_speed(mech));
    return;
  }
  DOCHECK_CONTEXT(context,
                  mech_control_is_flying(mech) && mech_fuel(mech) <= 0 &&
                      !mech_aero_has_free_fuel(mech),
                  "You're out of fuel!");
  maxspeed = mech_effective_maximum_speed(mech);

  if (mech_movement_type(mech) == MOVE_VTOL)
    maxspeed = sqrt((float)maxspeed * maxspeed -
                    mech_vertical_speed(mech) * mech_vertical_speed(mech));

  maxspeed = maxspeed > 0.0 ? maxspeed : 0.0;

  walkspeed = mech_control_walking_speed(maxspeed);
  newspeed = atof(args[0]);

  if (newspeed < 0.1) {

    /* Possibly a string speed instead? */
    for (i = 0; speed_tables[i].name; i++)
      if (!strcasecmp(speed_tables[i].name, args[0])) {
        switch (speed_tables[i].flag) {
        case 0:
          newspeed = 0.0;
          break;
        case -1:
          newspeed = -walkspeed;
          break;
        case 1:
          newspeed = walkspeed;
          break;
        case 2:
          newspeed = maxspeed;
          break;
        }
        break;
      }
  }

  if (newspeed > maxspeed)
    newspeed = maxspeed;
  if (newspeed < -walkspeed)
    newspeed = -walkspeed;

  DOCHECK_CONTEXT(context,
                  newspeed < 0 && mech_carried_dbref(mech) > 0 &&
                      !(mech_technology_flags(mech) & SALVAGE_TECH),
                  "You can not backup while towing!");

  DOCHECK_CONTEXT(context, newspeed < 0 && condition.sprinting,
                  "You can not backup while sprinting!");

  if (mech_control_is_running(newspeed, maxspeed)) {
    DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_DUMP),
                    "You can not run while dumping ammo!");
    DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_UNJAM_AMMO),
                    "You can not run while unjamming your weapon!");

    /* Exile Stun Code Effect */
    if (condition.stunned) {
      mech_notify(mech, MECHALL,
                  "You cannot move faster than cruise"
                  " speed while stunned!");
      return;
    }

    DOCHECK_CONTEXT(
        context, mech_event_count(mech, EVENT_UNSTUN_CREW),
        "Your cannot possibly control a vehicle going this fast in your "
        "current mental state!");
    DOCHECK_CONTEXT(
        context, condition.tail_rotor_destroyed,
        "Your cannot possibly control a VTOL going this fast with a "
        "destroyed tail rotor!");
    DOCHECK_CONTEXT(
        context,
        mech_class(mech) == CLASS_MECH &&
            ((mech_position_z(mech) < 0 &&
              battle_terrain_is_water(mech_real_terrain_get(mech))) ||
             mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER),
        "You can't run through water!");
  }
  if (!is_wizard(btech_context_database(context), player) &&
      is_in_character(btech_context_database(context), mech_dbref(mech)) &&
      mech_pilot_dbref(mech) != player) {
    if (newspeed < 0.0) {
      notify(
          btech_context_evaluation(context), player,
          "Not being the Pilot of this beast, you cannot move it backwards.");
      return;
    } else if (newspeed > walkspeed) {
      notify(btech_context_evaluation(context), player,
             "Not being the Pilot of this beast, you cannot go faster "
             "than walking speed.");
      return;
    }
  }
  mech_desired_speed_set(mech, newspeed);
  mech_maybe_move(mech);
  if (fabs(newspeed) > 0.1) {
    if (condition.swarm_target > 0) {
      bsuit_swarm_stop(mech, 1);
      mech_hidden_set(mech, false);
    }
    if (condition.digging) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    mech_dug_in_set(mech, false);
  }
  mech_printf(mech, MECHALL, "Desired speed changed to %d KPH.", (int)newspeed);
}

void mech_vertical(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);
  char *args[1];
  char buff[50] = {0};
  float newspeed, maxspeed;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(context,
                  mech_class(mech) != CLASS_VTOL &&
                      mech_movement_type(mech) != MOVE_SUB,
                  "This command is for VTOLs only.");
  DOCHECK_CONTEXT(context,
                  mech_class(mech) == CLASS_VTOL && mech_fuel(mech) <= 0 &&
                      !mech_aero_has_free_fuel(mech),
                  "You're out of fuel!");
  DOCHECK_CONTEXT(
      context,
      mech_control_requires_water(mech) && !mech_control_is_on_water(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(context, mech_parseattributes(buffer, args, 1) != 1,
                  tprintf("Current vertical speed is %.2f KPH.",
                          mech_vertical_speed(mech)));
  newspeed = atof(args[0]);
  maxspeed = mech_effective_maximum_speed(mech);
  maxspeed = sqrt((float)maxspeed * maxspeed -
                  mech_desired_speed(mech) * mech_desired_speed(mech));
  if ((newspeed > maxspeed) || (newspeed < -maxspeed)) {
    snprintf(buff, sizeof(buff), "Max vertical speed is + %d KPH and - %d KPH",
             (int)maxspeed, (int)maxspeed);
    notify(btech_context_evaluation(context), player, buff);
  } else {
    DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                    "Your vehicle's movement system is destroyed.");
    DOCHECK_CONTEXT(context,
                    mech_class(mech) == CLASS_VTOL && mech_is_landed(mech),
                    "You need to take off first.");
    mech_vertical_speed_set(mech, newspeed);
    mech_printf(mech, MECHALL, "Vertical speed changed to %d KPH",
                (int)newspeed);
    mech_maybe_move(mech);
  }
}
