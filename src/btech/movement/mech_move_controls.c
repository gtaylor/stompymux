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
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
/* Facing related */
void mech_heading(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  int newheading;

  cch(MECH_USUAL);
  if (mech_parseattributes(buffer, args, 1) == 1) {
    DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                    "This piece of equipment is stationary!");
    DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                    "Your fortified state prevents you from moving.");
    DOCHECK_CONTEXT(
        mech->xcode.context, WaterBeast(mech) && NotInWater(mech),
        "You are regrettably unable to move at this time. We apologize for "
        "the inconvenience.");
    DOCHECK_CONTEXT(mech->xcode.context,
                    is_aero(mech) && Spinning(mech) && !Landed(mech),
                    "You are unable to control your craft at the moment.");
    DOCHECK_CONTEXT(mech->xcode.context, PerformingAction(mech),
                    "You are too busy at the moment to turn.");
    DOCHECK_CONTEXT(mech->xcode.context, MechDugIn(mech),
                    "You are in a hole you dug, unable to move [use "
                    "speed cmd to get out].");
    DOCHECK_CONTEXT(mech->xcode.context, IsHulldown(mech),
                    "You can not turn while hulldown");
    DOCHECK_CONTEXT(mech->xcode.context,
                    mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                    "You are busy changing your hulldown mode");
    if (Digging(mech)) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    newheading = AcceptableDegree(atoi(args[0]));
    MechDesiredFacing(mech) = newheading;
    mech_printf(mech, MECHALL, "Heading changed to %d.", newheading);
    mech_maybe_move(mech);
  } else {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Your current heading is %i.", MechFacing(mech));
  }
}

void mech_turret(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  int newheading;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW ||
                      MechType(mech) == CLASS_BSUIT || is_aero(mech) ||
                      !GetSectInt(mech, TURRET),
                  "You don't have a turret.");
  DOCHECK_CONTEXT(mech->xcode.context, MechTankCritStatus(mech) & TURRET_JAMMED,
                  "Your turret is jammed in position.");
  DOCHECK_CONTEXT(mech->xcode.context, MechTankCritStatus(mech) & TURRET_LOCKED,
                  "Your turret is locked in position.");
  if (mech_parseattributes(buffer, args, 1) == 1) {
    newheading = AcceptableDegree(atoi(args[0]) - MechFacing(mech));
    MechTurretFacing(mech) = newheading;
    mech_printf(mech, MECHALL, "Turret facing changed to %d.",
                AcceptableDegree(MechTurretFacing(mech) + MechFacing(mech)));
  } else {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Your turret is currently facing %d.",
                  AcceptableDegree(MechTurretFacing(mech) + MechFacing(mech)));
  }

  MarkForLOSUpdate(mech);
}

void mech_rotatetorso(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2];

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_BSUIT, "Huh?");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_MECH && MechType(mech) != CLASS_MW,
                  "You don't have a torso.");
  DOCHECK_CONTEXT(
      mech->xcode.context, Fallen(mech),
      "You're lying flat on your face, you can't rotate your torso.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (MechType(mech) == CLASS_MECH) && (MechIsQuad(mech)),
                  "Quads can't rotate their torsos.");
  if (mech_parseattributes(buffer, args, 2) == 1) {
    switch (args[0][0]) {
    case 'L':
    case 'l':
      DOCHECK_CONTEXT(mech->xcode.context, MechStatus(mech) & TORSO_LEFT,
                      "You cannot rotate torso beyond 60 degrees!");
      if (MechStatus(mech) & TORSO_RIGHT)
        MechStatus(mech) &= ~TORSO_RIGHT;
      else
        MechStatus(mech) |= TORSO_LEFT;
      mech_notify(mech, MECHALL, "You rotate your torso left.");
      break;
    case 'R':
    case 'r':
      DOCHECK_CONTEXT(mech->xcode.context, MechStatus(mech) & TORSO_RIGHT,
                      "You cannot rotate torso beyond 60 degrees!");
      if (MechStatus(mech) & TORSO_LEFT)
        MechStatus(mech) &= ~TORSO_LEFT;
      else
        MechStatus(mech) |= TORSO_RIGHT;
      mech_notify(mech, MECHALL, "You rotate your torso right.");
      break;
    case 'C':
    case 'c':
      MechStatus(mech) &= ~(TORSO_RIGHT | TORSO_LEFT);
      mech_notify(mech, MECHALL, "You center your torso.");
      break;
    default:
      notify(btech_context_evaluation(mech->xcode.context), player,
             "Rotate must have LEFT RIGHT or CENTER.");
      break;
    }
  } else
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Invalid number of arguments!");
  MarkForLOSUpdate(mech);
}

struct {
  char *name;
  int flag;
} speed_tables[] = {{"walk", 1},   {"run", 2},   {"stop", 0}, {"back", -1},
                    {"cruise", 1}, {"flank", 2}, {NULL, 0.0}};

void mech_speed(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  float newspeed, walkspeed, maxspeed;
  int i;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
  if (RollingT(mech)) {
    DOCHECK_CONTEXT(mech->xcode.context, !Landed(mech),
                    "Use thrust command instead!");
  } else if (FlyingT(mech)) {
    DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_VTOL,
                    "Use thrust command instead!");
  }
  DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(mech->xcode.context, PerformingAction(mech),
                  "You are too busy at the moment to turn.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (Fallen(mech)) && (MechType(mech) != CLASS_MECH &&
                                     MechType(mech) != CLASS_MW),
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(
      mech->xcode.context, WaterBeast(mech) && NotInWater(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");

  if (MechType(mech) != CLASS_MECH)
    DOCHECK_CONTEXT(mech->xcode.context,
                    mech_event_count(mech, EVENT_REMOVE_PODS),
                    "You are too busy removing iNARC pods!");
  DOCHECK_CONTEXT(mech->xcode.context, IsHulldown(mech),
                  "You can not move while hulldown");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");

  if (mech_parseattributes(buffer, args, 1) != 1) {
    notify_printf(btech_context_evaluation(mech->xcode.context), player,
                  "Your current speed is %.2f.", MechSpeed(mech));
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  FlyingT(mech) && AeroFuel(mech) <= 0 &&
                      !mech_aero_has_free_fuel(mech),
                  "You're out of fuel!");
  maxspeed = MMaxSpeed(mech);

  if (MechMove(mech) == MOVE_VTOL)
    maxspeed = sqrt((float)maxspeed * maxspeed -
                    MechVerticalSpeed(mech) * MechVerticalSpeed(mech));

  maxspeed = maxspeed > 0.0 ? maxspeed : 0.0;

  /*   if (MechStatus(mech) & MASC_ENABLED) maxspeed = (4. / 3. ) * maxspeed; */
  walkspeed = WalkingSpeed(maxspeed);
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

  DOCHECK_CONTEXT(mech->xcode.context,
                  (newspeed < 0) && (MechCarrying(mech) > 0) &&
                      (!(MechSpecials(mech) & SALVAGE_TECH)),
                  "You can not backup while towing!");

  DOCHECK_CONTEXT(mech->xcode.context, (newspeed < 0) && Sprinting(mech),
                  "You can not backup while sprinting!");

  if (IsRunning(newspeed, maxspeed)) {
    DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_DUMP),
                    "You can not run while dumping ammo!");
    DOCHECK_CONTEXT(mech->xcode.context,
                    mech_event_count(mech, EVENT_UNJAM_AMMO),
                    "You can not run while unjamming your weapon!");

    /* Exile Stun Code Effect */
    if (MechCritStatus(mech) & MECH_STUNNED) {
      mech_notify(mech, MECHALL,
                  "You cannot move faster than cruise"
                  " speed while stunned!");
      return;
    }

    DOCHECK_CONTEXT(
        mech->xcode.context, mech_event_count(mech, EVENT_UNSTUN_CREW),
        "Your cannot possibly control a vehicle going this fast in your "
        "current mental state!");
    DOCHECK_CONTEXT(
        mech->xcode.context, MechTankCritStatus(mech) & TAIL_ROTOR_DESTROYED,
        "Your cannot possibly control a VTOL going this fast with a "
        "destroyed tail rotor!");
    DOCHECK_CONTEXT(
        mech->xcode.context,
        MechType(mech) == CLASS_MECH &&
            ((MechZ(mech) < 0 && (mech_real_terrain_get(mech) == WATER ||
                                  mech_real_terrain_get(mech) == BRIDGE ||
                                  mech_real_terrain_get(mech) == ICE)) ||
             mech_real_terrain_get(mech) == HIGHWATER),
        "You can't run through water!");
  }
  if (!is_wizard(mech->xcode.context->database, player) &&
      is_in_character(mech->xcode.context->database, mech->mynum) &&
      MechPilot(mech) != player) {
    if (newspeed < 0.0) {
      notify(
          btech_context_evaluation(mech->xcode.context), player,
          "Not being the Pilot of this beast, you cannot move it backwards.");
      return;
    } else if (newspeed > walkspeed) {
      notify(btech_context_evaluation(mech->xcode.context), player,
             "Not being the Pilot of this beast, you cannot go faster "
             "than walking speed.");
      return;
    }
  }
  MechDesiredSpeed(mech) = newspeed;
  mech_maybe_move(mech);
  if (fabs(newspeed) > 0.1) {
    if (MechSwarmTarget(mech) > 0) {
      StopSwarming(mech, 1);
      MechCritStatus(mech) &= ~HIDDEN;
    }
    if (Digging(mech)) {
      mech_notify(mech, MECHALL, "You cease your attempts at digging in.");
      mech_stop_digging(mech);
    }
    MechTankCritStatus(mech) &= ~DUG_IN;
  }
  mech_printf(mech, MECHALL, "Desired speed changed to %d KPH.", (int)newspeed);
}

void mech_vertical(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  char buff[50] = {0};
  float newspeed, maxspeed;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_VTOL && MechMove(mech) != MOVE_SUB,
                  "This command is for VTOLs only.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) == CLASS_VTOL && AeroFuel(mech) <= 0 &&
                      !mech_aero_has_free_fuel(mech),
                  "You're out of fuel!");
  DOCHECK_CONTEXT(
      mech->xcode.context, WaterBeast(mech) && NotInWater(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  tprintf("Current vertical speed is %.2f KPH.",
                          (float)MechVerticalSpeed(mech)));
  newspeed = atof(args[0]);
  maxspeed = MMaxSpeed(mech);
  maxspeed = sqrt((float)maxspeed * maxspeed -
                  MechDesiredSpeed(mech) * MechDesiredSpeed(mech));
  if ((newspeed > maxspeed) || (newspeed < -maxspeed)) {
    snprintf(buff, sizeof(buff), "Max vertical speed is + %d KPH and - %d KPH",
             (int)maxspeed, (int)maxspeed);
    notify(btech_context_evaluation(mech->xcode.context), player, buff);
  } else {
    DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                    "Your vehicle's movement system is destroyed.");
    DOCHECK_CONTEXT(mech->xcode.context,
                    MechType(mech) == CLASS_VTOL && Landed(mech),
                    "You need to take off first.");
    MechVerticalSpeed(mech) = newspeed;
    mech_printf(mech, MECHALL, "Vertical speed changed to %d KPH",
                (int)newspeed);
    mech_maybe_move(mech);
  }
}
