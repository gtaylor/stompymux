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
void mech_jump(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *tempMech = NULL;
  BattleMap *mech_map;
  char *args[3];
  int argc;
  int target;
  char targetID[2];
  short mapx, mapy;
  int bearing;
  float range = 0.0;
  float realx, realy;
  int sz, tz, jps;

  mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
#ifdef BT_MOVEMENT_MODES
  DOCHECK_CONTEXT(mech->xcode.context, mech_move_mode_locked(mech),
                  "Movement modes disallow jumping.");
#endif
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_MECH && MechType(mech) != CLASS_MW &&
                      MechType(mech) != CLASS_BSUIT &&
                      MechType(mech) != CLASS_VEH_GROUND,
                  "This unit cannot jump.");
  DOCHECK_CONTEXT(mech->xcode.context, MechCarrying(mech) > 0,
                  "You can't jump while towing someone!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (MechMaxSpeed(mech) - MMaxSpeed(mech)) > MP1,
                  "No, with this cargo you won't!");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You can't Jump from a FALLEN position");
  DOCHECK_CONTEXT(mech->xcode.context, IsHulldown(mech),
                  "You can't Jump while hulldown");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You're already jumping!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_JUMPSTABIL),
                  "You haven't stabilized from your last jump yet.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You haven't finished standing up yet.");
  DOCHECK_CONTEXT(mech->xcode.context, fabs(MechJumpSpeed(mech)) <= 0.0,
                  "This mech doesn't have jump jets!");
  argc = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_DUMP),
                  "You can not jump while dumping ammo!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_UNJAM_AMMO),
                  "You can not jump while unjamming your weapon!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_REMOVE_PODS),
                  "You are too busy removing iNARC pods!");
  DOCHECK_CONTEXT(
      mech->xcode.context, MapIsUnderground(mech_map),
      "Realize the ceiling in this grotto is a bit to low for that!");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "You can't jump while orbital dropping!");

  DOCHECK_CONTEXT(mech->xcode.context, MechSwarmTarget(mech) > 0,
                  "Perhaps you should dismount your ride first!");

  if (Staggering(mech)) {
    mech_notify(mech, MECHALL, "The damage inhibits your coordination...");

    if (!MadePilotSkillRoll(mech, calcStaggerBTHMod(mech))) {
      mech_notify(mech, MECHALL, "... something you apparently can't handle!");
      mech_los_broadcast(
          mech,
          "engages jumpjets, rolls to the side and slams into the ground!");
      MechFalls(mech, 1, 0);
      return;
    }
  }

  if (doJettisonChecks(mech))
    return;

  DOCHECK_CONTEXT(mech->xcode.context, argc > 2,
                  "Too many arguments to JUMP function!");
  DOCHECK_CONTEXT(mech->xcode.context, argc < 0,
                  "Invalid number of arguments to JUMP function!");
  MechStatus(mech) &= ~DFA_ATTACK; /* By default no DFA */
  switch (argc) {
  case 0:
    /* DFA current target... */

    DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_MECH,
                    "Only mechs can do Death From Above attacks!");

    target = MechTarget(mech);
    tempMech = btech_context_get_mech(mech->xcode.context, target);
    DOCHECK_CONTEXT(mech->xcode.context, !tempMech, "Invalid Target!");
    range = FaMechRange(mech, tempMech);
    DOCHECK_CONTEXT(mech->xcode.context,
                    !mech_los_check(mech, tempMech, MechX(tempMech),
                                    MechY(tempMech), range),
                    "Target is not in line of sight!");
    DOCHECK_CONTEXT(mech->xcode.context, MechType(tempMech) == CLASS_MW,
                    "Even you can't aim your jump well enough to squish that!");
    mapx = MechX(tempMech);
    mapy = MechY(tempMech);
    MechDFATarget(mech) = MechTarget(mech);
    break;
  case 1:
    /* Jump Target */
    DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) != CLASS_MECH,
                    "Only mechs can do Death From Above attacks!");

    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    target = FindTargetDBREFFromMapNumber(mech, targetID);
    tempMech = btech_context_get_mech(mech->xcode.context, target);
    DOCHECK_CONTEXT(mech->xcode.context, !tempMech,
                    "Target is not in line of sight!");
    range = FaMechRange(mech, tempMech);
    DOCHECK_CONTEXT(mech->xcode.context,
                    !mech_los_check(mech, tempMech, MechX(tempMech),
                                    MechY(tempMech), range),
                    "Target is not in line of sight!");
    DOCHECK_CONTEXT(mech->xcode.context, MechType(tempMech) == CLASS_MW,
                    "Even you can't aim your jump well enough to squish that!");
    mapx = MechX(tempMech);
    mapy = MechY(tempMech);
    MechDFATarget(mech) = tempMech->mynum;
    break;
  case 2:
    bearing = atoi(args[0]);
    range = atof(args[1]);
    FindXY(MechFX(mech), MechFY(mech), bearing, range, &realx, &realy);

    /* This is so we are jumping to the center of a hex */
    /* and the bearing jives with the target hex */
    RealCoordToMapCoord(&mapx, &mapy, realx, realy);
    break;
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  mapx >= mech_map->map_width || mapy >= mech_map->map_height ||
                      mapx < 0 || mapy < 0,
                  "That would take you off the map!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechX(mech) == mapx && MechY(mech) == mapy,
                  "You're already in the target hex.");
  sz = MechZ(mech);
  if (map_real_terrain_get(mech_map, mapx, mapy) == ICE)
    tz = 0;
  else
    tz = Elevation(mech_map, mapx, mapy);
  jps = JumpSpeedMP(mech, mech_map);
  DOCHECK_CONTEXT(mech->xcode.context, range > jps,
                  "That target is out of range!");
  if (MechType(mech) != CLASS_BSUIT && tempMech)
    MechStatus(mech) |= DFA_ATTACK;
  /*   MechJumpTop(mech) = BOUNDED(3, (jps - range) + 2, jps - 1); */
  /* New idea: JumpTop = (JP + 1 - range / 3) - in another words,
     SDR jumping for 1 hexes has 8 + 1 = 9 hex elevation in mid-flight,
     SDR jumping for 8 hexes has 8 + 1 - 2 = 7 hex elevation in mid-flight,
     TDR jumping for 4 hexes has 4 + 1 - 1 = 4 hex elevation in mid-flight

     Come to think of it, the last SDR figure was ridiculous. New
     value: 2 * 1 + 2 = 4
   */
  MechJumpTop(mech) = MIN(jps + 1 - range / 3, 2 * range + 2);
  DOCHECK_CONTEXT(mech->xcode.context, (tz - sz) > jps,
                  "That target's high for you to reach with a single jump!");
  DOCHECK_CONTEXT(mech->xcode.context, (sz - tz) > jps,
                  "That target's low for you to reach with a single jump!");
  DOCHECK_CONTEXT(mech->xcode.context, sz < -1, "Glub glub glub.");
  MapCoordToRealCoord(mapx, mapy, &realx, &realy);
  bearing = FindBearing(MechFX(mech), MechFY(mech), realx, realy);

  /* TAKE OFF! */
  MechCocoon(mech) = 0;
  MechJumpHeading(mech) = bearing;
  MechStatus(mech) |= JUMPING;
  MechStartFX(mech) = MechFX(mech);
  MechStartFY(mech) = MechFY(mech);
  MechStartFZ(mech) = MechFZ(mech);
  MechJumpLength(mech) = length_hypotenuse((double)(realx - MechStartFX(mech)),
                                           (double)(realy - MechStartFY(mech)));
  MechGoingX(mech) = mapx;
  MechGoingY(mech) = mapy;
  MechEndFZ(mech) = ZSCALE * tz;
  MechSpeed(mech) = 0.0;
  if (MechStatus(mech) & DFA_ATTACK)
    mech_notify(mech, MECHALL,
                "You engage your jump jets for a Death From Above attack!");
  else
    mech_notify(mech, MECHALL, "You engage your jump jets.");
  MechSwarmTarget(mech) = -1;
  mech_los_broadcast(mech, "engages jumpjets!");
  mech_event_schedule(mech, EVENT_JUMP, mech_jump_event, JUMP_TICK, 0);
}

static void mech_hulldown_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long type = (long)e->data2;

  if (!mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
    return;

  if (!Started(mech))
    return;

  if (type == 0) {
    MechStatus(mech) &= ~HULLDOWN;
    mech_notify(mech, MECHALL, "You finish lifting yourself up.");
    mech_los_broadcast(mech, "finishes lifting itself up");
  } else {
    MechStatus(mech) |= HULLDOWN;
    mech_notify(mech, MECHALL, "You finish lowering yourself to the ground.");
    mech_los_broadcast(mech, "finishes lowering itself to the ground.");
  }
}

#ifdef BT_MOVEMENT_MODES
void mech_sprint(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(mech->xcode.context, MechCarrying(mech) > 0,
                  "You cannot sprint while towing!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You cannot do this while jumping.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (Fallen(mech)) && (MechType(mech) != CLASS_MECH &&
                                     MechType(mech) != CLASS_MW),
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  InWater(mech) && !(WaterBeast(mech)) &&
                      !(MechStatus2(mech) & SPRINTING),
                  "You cannot start sprinting while in water!");
  DOCHECK_CONTEXT(
      mech->xcode.context, WaterBeast(mech) && NotInWater(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(mech->xcode.context, MechStatus2(mech) & (EVADING | DODGING),
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(mech->xcode.context, MechSwarmTarget(mech) > 0,
                  "You cannot sprint while mounted!");
  if (MechType(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(
        mech->xcode.context,
        SectIsDestroyed(mech, RLEG) || SectIsDestroyed(mech, LLEG) ||
            (MechMove(mech) != MOVE_QUAD
                 ? 0
                 : SectIsDestroyed(mech, RLEG) || SectIsDestroyed(mech, LLEG)),
        "That's kind of hard while limping.");

  DOCHECK_CONTEXT(
      mech->xcode.context, MechChargeTarget(mech) > 0,
      "You are currently charging a target and unable to start sprinting!");

  d |= MODE_SPRINT | ((MechStatus2(mech) & SPRINTING) ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = MechFullNoRecycle(mech, CHECK_BOTH)) > 0) {
      mech_printf(mech, MECHALL, "You have %s recycling!",
                  (i == 1   ? "weapons"
                   : i == 2 ? "limbs"
                            : "error"));
      return;
    }
    mech_notify(mech, MECHALL, "You begin the process of sprinting...");
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        d);
  } else {
    mech_notify(mech, MECHALL, "You begin the process of ceasing to sprint.");
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        d);
  }
  return;
}

void mech_evade(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You are currently standing up and cannot move.");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You cannot do this while jumping.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  (Fallen(mech)) && (MechType(mech) != CLASS_MECH &&
                                     MechType(mech) != CLASS_MW),
                  "Your vehicle's movement system is destroyed.");
  DOCHECK_CONTEXT(mech->xcode.context, MechCarrying(mech) > 0,
                  "You can't do that while towing");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You are currently prone and cannot move.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  !(MechStatus2(mech) & EVADING) &&
                      MechType(mech) == CLASS_MECH &&
                      (PartIsNonfunctional(mech, LLEG, 0) ||
                       PartIsNonfunctional(mech, RLEG, 0)),
                  "You need both hips functional to evade.");
  DOCHECK_CONTEXT(
      mech->xcode.context, WaterBeast(mech) && NotInWater(mech),
      "You are regrettably unable to move at this time. We apologize for "
      "the inconvenience.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechStatus2(mech) & (SPRINTING | DODGING),
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(mech->xcode.context, MechSwarmTarget(mech) > 0,
                  "You cannot evade while mounted!");
  DOCHECK_CONTEXT(mech->xcode.context, MechChargeTarget(mech) > 0,
                  "You cannot evade while charging!");

  if (MechType(mech) == CLASS_MECH)
    DOCHECK_CONTEXT(
        mech->xcode.context,
        SectIsDestroyed(mech, RLEG) || SectIsDestroyed(mech, LLEG) ||
            (MechMove(mech) != MOVE_QUAD
                 ? 0
                 : SectIsDestroyed(mech, RLEG) || SectIsDestroyed(mech, LLEG)),
        "That's kind of hard while limping.");

  d |= MODE_EVADE | ((MechStatus2(mech) & EVADING) ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = MechFullNoRecycle(mech, CHECK_BOTH)) > 0) {
      mech_printf(mech, MECHALL, "You have %s recycling!",
                  (i == 1   ? "weapons"
                   : i == 2 ? "limbs"
                            : "error"));
      return;
    }
    mech_notify(mech, MECHALL, "You begin the process of evading...");
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        d);
  } else {
    mech_notify(mech, MECHALL, "You begin the process of ceasing to evade.");
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        d);
  }
  return;
}

void mech_dodge(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  long d = 0;
  int i;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(mech->xcode.context, MechMove(mech) == MOVE_NONE,
                  "This piece of equipment is stationary!");
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
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_MOVEMODE),
                  "You are already changing movement modes!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechStatus2(mech) & (SPRINTING | EVADING),
                  "You cannot perform multiple movement modes!");
  DOCHECK_CONTEXT(
      mech->xcode.context,
      !(HasBoolAdvantage(mech->xcode.context, player, "dodge_maneuver")) ||
          player != MechPilot(mech),
      "You either are not the pilot of this mech, have no Dodge Maneuver "
      "adavantage, or both.");
  DOCHECK_CONTEXT(mech->xcode.context, MechChargeTarget(mech) > 0,
                  "You cannot dodge while charging!");

  d |= MODE_DODGE | ((MechStatus2(mech) & DODGING) ? MODE_OFF : MODE_ON);
  if (d & MODE_ON) {
    if ((i = MechFullNoRecycle(mech, CHECK_PHYS)) > 0) {
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

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, Fortified(mech),
                  "Your fortified state prevents you from moving.");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "While falling out of the sky?");
  DOCHECK_CONTEXT(mech->xcode.context, !MechIsQuad(mech),
                  "Only QUADs can hulldown.");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech),
                  "You can't hulldown from a FALLEN position");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You can't hulldown while jumping!");
  DOCHECK_CONTEXT(mech->xcode.context, MechSpeed(mech) > 0.5,
                  "You can't hulldown while moving!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_JUMPSTABIL),
                  "You are still stabilizing from your last jump.");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You haven't finished standing up yet.");

  argc = mech_parseattributes(buffer, args, 1);

  if (argc > 0) {
    if (!strcmp(args[0], "-")) {
      if (!IsHulldown(mech))
        mech_notify(mech, MECHALL, "You are not hulldown.");
      else if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
        mech_notify(mech, MECHALL, "You are busy changing your hulldown mode.");
      else {
        mech_notify(mech, MECHALL, "You start to lift yourself up.");
        mech_los_broadcast(mech, "begins to raise up on its legs.");

        mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                            StandMechTime(mech), 0);
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

  DOCHECK_CONTEXT(mech->xcode.context, IsHulldown(mech),
                  "You are already hulldown.");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode.");

  mech_notify(mech, MECHALL, "You start to lower yourself to the ground.");
  mech_los_broadcast(mech, "begins to lower itself to the ground.");
  MechDesiredSpeed(mech) = 0;

  mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                      StandMechTime(mech), 1);
}
