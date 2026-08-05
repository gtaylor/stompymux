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
#include "environment_damage_api.h"
#include "legacy_macros.h"
#include "map.h"
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
#include "mech_stagger.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
int DropGetElevation(Mech *mech) {
  if (mech_real_terrain_get(mech) == BRIDGE) {
    if (MechZ(mech) < (MechElev(mech))) {
      if (Overwater(mech))
        return 0;
      return bridge_w_elevation(mech);
    }
    return MechElevation(mech);
  }
  if (Overwater(mech) ||
      (mech_real_terrain_get(mech) == ICE && MechZ(mech) >= 0))
    return MAX(0, MechElevation(mech));
  else
    return MechElevation(mech);
}

void DropSetElevation(Mech *mech, int wantdrop) {
  if (mech_real_terrain_get(mech) == BRIDGE) {
    bridge_set_elevation(mech);
    return;
  }
  MechZ(mech) = DropGetElevation(mech);
  MechFZ(mech) = MechZ(mech) * ZSCALE;
  if (wantdrop)
    if (mech_real_terrain_get(mech) == ICE && MechZ(mech) >= 0)
      possibly_drop_thru_ice(mech);
}

void LandMech(Mech *mech) {
  Mech *target;
  BattleMap *mech_map =
      btech_context_get_map(mech->xcode.context, mech->mapindex);
  int dfa = 0;
  int done = 0;

  /*
   * Added check to see if we're actually awake when we try to land
   * - Kipsta
   * - 8/3/99
   */

  if (Uncon(mech)) {
    mech_notify(mech, MECHALL,
                "Your lack of conciousness makes you fall to the ground. Not "
                "like you can read this anyway.");
    MechFalls(mech, 1, 0);
    dfa = 1;
    done = 1;
  } else {
    /* Handle DFA attack */
    if (MechStatus(mech) & DFA_ATTACK) {
      /* is the target here? */
      target = btech_context_get_mech(mech->xcode.context, MechDFATarget(mech));
      if (target) {
        if (MechX(target) == MechX(mech) && MechY(target) == MechY(mech))
          dfa = DeathFromAbove(mech, target);
        else
          mech_notify(mech, MECHPILOT, "Your DFA target has moved!");
      } else
        mech_notify(mech, MECHPILOT, "Your target has become invalid.");
    }

    if (!dfa)
      mech_notify(mech, MECHALL, "You finish your jump.");

    /* Better reset the FZ */
    MechElev(mech) = map_elevation_get(mech_map, MechX(mech), MechY(mech));
    MechZ(mech) = MechElev(mech) - 1;
    MechFZ(mech) = ZSCALE * MechZ(mech);
    DropSetElevation(mech, 1);

    if (Staggering(mech)) {
      mech_notify(mech, MECHALL,
                  "The damage you've taken makes the landing a bit harder...");

      if (!MadePilotSkillRoll(mech, calcStaggerBTHMod(mech))) {
        mech_notify(mech, MECHALL,
                    "... something you apparently can't handle!");
        mech_los_broadcast(mech, "lands, staggers, and falls down!");
        MechFalls(mech, 1, 0);
        return;
      }
    }

    /* Check piloting rolls, etc. */
    if (MechType(mech) == CLASS_MECH) {
      if (CountDestroyedLegs(mech) > 0) {
        mech_notify(mech, MECHPILOT,
                    "Your missing leg makes it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your missing leg has caused you to fall upon landing!");
          mech_los_broadcast(mech, "lands, unbalanced, and falls down!");
          dfa = 1;
          MechFalls(mech, 1, 0);
          done = 1;
        }
      } else if (MechSections(mech)[RLEG].basetohit ||
                 MechSections(mech)[LLEG].basetohit) {
        mech_notify(mech, MECHPILOT,
                    "Your damaged leg actuators make it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your damaged leg actuators have caused you to fall upon "
                      "landing!");
          mech_los_broadcast(mech, "lands, stumbles, and falls down!");
          dfa = 1;
          done = 1;
          MechFalls(mech, 1, 0);
        }
      } else if ((MechCritStatus(mech) & GYRO_DAMAGED) ||
                 (MechCritStatus(mech) & GYRO_DESTROYED)) {
        mech_notify(mech, MECHPILOT,
                    "Your damaged gyro makes it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your damaged gyro has caused you to fall upon landing!");
          mech_los_broadcast(mech, "lands, twists awkwardly, and falls down!");
          dfa = 1;
          done = 1;
          MechFalls(mech, 1, 0);
        }
      }
    }
  }

  if ((MechType(mech) == CLASS_MECH) && CountSwarmers(mech)) {
    mech_notify(mech, MECHALL,
                "The suits hanging off you make landing harder!");

    if (MadePilotSkillRoll(mech, 4)) {
      StopBSuitSwarmers(
          btech_context_find_object(mech->xcode.context, mech->mapindex), mech,
          0);
    } else {
      mech_notify(mech, MECHALL,
                  "You fail to properly control your unbalanced landing!");
      mech_los_broadcast(mech,
                         "lands and crashes to the ground from the weight "
                         "of the battlesuits!");
      MechFalls(mech, 1, 0);
    }
  }

  if (!dfa && !Fallen(mech) && !mech_domino_resolve(mech, MECH_DOMINO_JUMP)) {
    if (MechType(mech) != CLASS_VEH_GROUND)
      mech_los_broadcast(mech, "lands gracefully.");
    else
      mech_los_broadcast(mech, "returns to the ground where it belongs.");
  }

  /* If we aren't jumping anymore, we already took care of the event.
     (e.g. in MechFalls()) */
  if (Jumping(mech))
    mech_event_schedule(mech, EVENT_JUMPSTABIL, mech_stabilizing_event,
                        JUMP_TO_HIT_RECYCLE, 0);
  MechStatus(mech) &= ~JUMPING;
  MechStatus(mech) &= ~DFA_ATTACK;
  MechDFATarget(mech) = -1;
  MechGoingX(mech) = MechGoingY(mech) = 0;
  MechSpeed(mech) = 0;
  mech_event_cancel(mech, EVENT_JUMP); /* Kill the event for moving 'round */
  mech_maybe_move(mech);               /* Possibly start movin' on da ground */

  if (!done)
    possible_mine_poof(mech, MINE_LAND);

  MechFloods(mech);
  water_extinguish_inferno(mech);
  // this is only for non-new-stagger
  if (!mech->xcode.context->configuration->btech_newstagger)
    mech_stop_stagger_check(mech);
}

/* Flooding code. Once we're in water, this is checked
   now and then (basically when DamageMech'ed and/or
   depth changes and/or we fall) */

void MechFloodsLoc(Mech *mech, int loc, int lev) {
  char locbuff[32];
  ;

  if (MechStatus(mech) & COMBAT_SAFE)
    return;

  if ((GetSectArmor(mech, loc) &&
       (GetSectRArmor(mech, loc) || !GetSectORArmor(mech, loc))) ||
      !GetSectInt(mech, loc))
    return;
  if (!InWater(mech))
    return;
  if (lev >= 0)
    return;
  /* No armor, and in water. */
  if (lev == -1 && (!Fallen(mech) && loc != LLEG && loc != RLEG &&
                    (!MechIsQuad(mech) || (loc != LARM && loc != RARM))))
    return;
  if (MechType(mech) != CLASS_MECH)
    return;

  if (SectIsFlooded(mech, loc))
    return;

  /* Woo, valid target. */
  ArmorStringFromIndex(loc, locbuff, MechType(mech), MechMove(mech));
  mech_printf(
      mech, MECHALL,
      "[fg=red bold]Water floods into your %s disabling everything that was "
      "there![reset]",
      locbuff);
  mech_los_broadcast(
      mech, tprintf("has a gaping hole in %s, and water pours in!", locbuff));

  SetSectFlooded(mech, loc);
  mech_parts_destroy(mech, mech, loc, 1, 1);
}

void MechFloods(Mech *mech) {
  int i;
  int elev = MechElevation(mech);

  if (!InWater(mech))
    return;

  /* Waterproof Tech - no flooding if we have this */
  if (MechSpecials2(mech) & WATERPROOF_TECH)
    return;

  if (MechType(mech) == CLASS_BSUIT) {

    if (MechSwarmTarget(mech) > 0)
      return;

    mech_notify(mech, MECHALL,
                "You somehow find yourself in water and realize this may "
                "really really suck...");
    mech_notify(mech, MECHALL,
                "Everything gets very dark as water starts to fill your suit "
                "and you sink towards the bottom!");

    mech_los_broadcast(
        mech, "shudders, splashes in the water for a second, then goes limp "
              "and sinks to the bottom.");

    KillMechContentsIfIC(mech);
    DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
    return;
  }

  if (MechType(mech) != CLASS_MECH)
    return;

  if (MechZ(mech) >= 0)
    return;

  for (i = 0; i < NUM_SECTIONS; i++)
    MechFloodsLoc(mech, i, elev);
}

void MechFalls(Mech *mech, int levels, int seemsg) {
  int roll, spread, i, hitloc, hitGroup = 0;
  int isrear = 0, damage, iscritical = 0;
  BattleMap *map;

  /* get rid of our swarmers */
  if (CountSwarmers(mech))
    StopBSuitSwarmers(
        btech_context_find_object(mech->xcode.context, mech->mapindex), mech,
        0);

  /* Clear stagger damage if we use new stagger*/
  if (mech->xcode.context->configuration->btech_newstagger)
    mech_stagger_damage_clear(mech);

  /* damage pilot */
  MechCocoon(mech) = 0;

  /* Rule Reference: BMR Revised, Page 16 ( Fall = Bruise if Pilot roll fails)
   */
  /* Rule Reference: Total Warfare, Page 41 ( Fall = Bruise if Pilot roll fails)
   */

  if (!(MechStatus(mech) & COMBAT_SAFE)) {
    if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW || seemsg)
      mech_notify(mech, MECHPILOT,
                  "You try to avoid taking personal damage in the fall.");
    else
      mech_notify(mech, MECHPILOT, "You try to avoid taking personal damage.");
    if (!MadePilotSkillRoll(mech, levels)) {
      if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW || seemsg)
        mech_notify(mech, MECHPILOT, "You take personal injury from the fall!");
      else
        mech_notify(mech, MECHPILOT, "You take personal injury!");
      headhitmwdamage(mech, mech, 1);
    }
  }

  MechSpeed(mech) = 0;
  MechDesiredSpeed(mech) = 0;
  if (Jumping(mech)) {
    MechStatus(mech) &= ~JUMPING;
    MechStatus(mech) &= ~DFA_ATTACK;
    mech_event_cancel(mech, EVENT_JUMP);
    mech_event_schedule(mech, EVENT_JUMPSTABIL, mech_stabilizing_event,
                        JUMP_TO_HIT_RECYCLE, 0);
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_event_count(mech, EVENT_MOVEMODE))
    mech_event_cancel(mech, EVENT_MOVEMODE);
  if (MechStatus2(mech) & SPRINTING)
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        MODE_SPRINT | MODE_OFF);
  if (MechStatus2(mech) & EVADING)
    mech_event_schedule(
        mech, EVENT_MOVEMODE, mech_movemode_event,
        (MechType(mech) == CLASS_BSUIT || MechType(mech) == CLASS_MW) ? TURN / 2
                                                                      : TURN,
        MODE_EVADE | MODE_OFF);
#endif
  if (MechMove(mech) == MOVE_VTOL || MechMove(mech) == MOVE_FLY) {
    MechVerticalSpeed(mech) = 0;
    MechGoingY(mech) = 0;
    MechStartFX(mech) = 0.0;
    MechStartFY(mech) = 0.0;
    MechStartFZ(mech) = 0.0;
    MechStatus(mech) |= LANDED;
    if (!(MechStatus(mech) & COMBAT_SAFE)) {
      if (MechMove(mech) == MOVE_VTOL)
        mech_notify(mech, MECHALL, "Your rotor has been destroyed!");
      MechStatus(mech) |= FALLEN;
    }
    mech_event_cancel(mech, EVENT_MOVE);
  } else
    mech_maybe_move(mech);
  if (MechType(mech) == CLASS_MECH || MechType(mech) == CLASS_MW)
    mech_make_fall(mech);

  if (seemsg)
    mech_los_broadcast(mech, "falls down!");
  DropSetElevation(mech, 1);
  MechFZ(mech) = MechZ(mech) * ZSCALE;

  roll = btech_random_range(mech->xcode.context, 1, 6);
  switch (roll) {
  case 1:
    hitGroup = FRONT;
    break;
  case 2:
    AddFacing(mech, 60);
    hitGroup = RIGHTSIDE;
    break;
  case 3:
    AddFacing(mech, 120);
    hitGroup = RIGHTSIDE;
    break;
  case 4:
    AddFacing(mech, 180);
    hitGroup = BACK;
    break;
  case 5:
    AddFacing(mech, 240);
    hitGroup = LEFTSIDE;
    break;
  case 6:
    AddFacing(mech, 300);
    hitGroup = LEFTSIDE;
    break;
  }
  if (hitGroup == BACK)
    isrear = 1;
  SetFacing(mech, AcceptableDegree(MechFacing(mech)));
  MechDesiredFacing(mech) = MechFacing(mech);
  if (!InWater(mech) && mech_real_terrain_get(mech) != HIGHWATER)
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (MechTons(mech) + 5)) / 10;
#else
    damage = (levels * (MechRealTons(mech) + 5)) / 10;
#endif /* REALWEIGHT_DAMAGE */
  else
#ifndef REALWEIGHT_DAMAGE
    damage = (levels * (MechTons(mech) + 5)) / 20;
#else
    damage = (levels * (MechRealTons(mech) + 5)) / 20;
#endif /* REALWEIGHT_DAMAGE */
  if (InSpecial(mech))
    if ((map = btech_context_find_object(mech->xcode.context, mech->mapindex)))
      if (MapUnderSpecialRules(map))
        damage = damage * MIN(100, MapGravity(map)) / 100;

  if (MechType(mech) == CLASS_MW)
    damage *= 40;

  spread = damage / 5;

  if (!(MechStatus(mech) & COMBAT_SAFE)) {
    for (i = 0; i < spread; i++) {
      hitloc = FindHitLocation(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, 5, -1, -1, 0,
                 -1, 0, 0);
      MechFloods(mech);
      water_extinguish_inferno(mech);
    }
    if (damage % 5) {
      hitloc = FindHitLocation(mech, hitGroup, &iscritical, &isrear);
      DamageMech(mech, mech, 0, -1, hitloc, isrear, iscritical, (damage % 5),
                 -1, -1, 0, -1, 0, 0);
      MechFloods(mech);
      water_extinguish_inferno(mech);
    }
  }
  possible_mine_poof(mech, MINE_FALL);
  MarkForLOSUpdate(mech);
}
