#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <math.h>
#include <stdlib.h>
#include <time.h>

#include "aero_move_api.h"
#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_partnames_api.h"
#include "mech_sensor_api.h"
#include "mech_stagger.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#undef WEAPON_RECYCLE_DEBUG

void mech_heartbeat(Mech *mech) {
  mech_update_recycling(mech);
  if (btech_context_stagger_mode(mech_context(mech)) >= 1 &&
      MechType(mech) == CLASS_MECH) {
    // no sense checking if a fallen mech will fall down again, and let's not
    // let jumping mechs stagger.
    if (!Fallen(mech) && !Jumping(mech)) {
      mech_staggercheck_heartbeat(mech);
    }
    mech_stagger_damage_expire(mech, mech->xcode.context->clock->now);
  }
  // Aeros need to check fuel while sitting and hovering
  if (MechType(mech) == CLASS_AERO || MechType(mech) == CLASS_VTOL) {
    if (!Landed(mech) && (fabs(MechSpeed(mech)) == 0) &&
        (fabs(MechVerticalSpeed(mech)) == 0))
      aero_fuel_check(mech);
  }

  return;
}

void mech_staggercheck_heartbeat(Mech *mech) {
  time_t now = mech->xcode.context->clock->now;
  int curStaggerDamage = 0;
  int prevStaggerDamage = 0;
  int staggerLevel = 0;

  // if we've not checked stagger since last time... ruhroh!
  if (now - (mech)->rd.lastStaggerCheck >=
      btech_context_stagger_interval(mech_context(mech))) {
    (mech)->rd.lastStaggerCheck = now;

    // curStagger is stuff we haven't rolled against
    // prevStagger is stuf we have
    // stuff we have adds to the difficulty, but doesn't get rolled against
    curStaggerDamage = mech_stagger_damage_current(mech, now);
    prevStaggerDamage = mech_stagger_damage_current_counted(mech, now);
    if (curStaggerDamage < 20)
      return;
    else {
      staggerLevel = curStaggerDamage / 20;

      // Dont need to remove stagger anymore, it clears on fall,
      // unless we're using
      // Stagger mode 2 removes damage after it is checked.
      if (btech_context_stagger_mode(mech_context(mech)) == 2)
        mech_stagger_damage_remove(mech, staggerLevel);
      else {
        mech_stagger_damage_mark(mech, staggerLevel);
        staggerLevel = (curStaggerDamage + prevStaggerDamage) / 20;
      }
      switch (staggerLevel) {
      case 1:
        mech_notify(mech, MECHALL,
                    "[fg=yellow bold]The damage causes you to stagger a "
                    "little.[reset]");
        mech_los_broadcast(mech, "stumbles slightly!");
        break;

      case 2:
        mech_notify(
            mech, MECHALL,
            "[fg=red]The damage causes you to stagger even more![reset]");
        mech_los_broadcast(mech, "starts to stagger from the damage!");
        break;

      default:
        mech_notify(
            mech, MECHALL,
            "[fg=red bold]The damage causes you to stagger violently while "
            "attempting to keep your footing![reset]");
        mech_los_broadcast(
            mech, "staggers back and forth attempting to keep its footing!");
        break;
      }

      // do the actual staggering here
      mech_notify(mech, MECHALL, "You stagger from the damage!");

      if (!MadePilotSkillRoll(mech, calcNewStaggerBTHMod(mech, staggerLevel))) {
        mech_notify(mech, MECHALL,
                    "You loose the battle with gravity and tumble over!!");
        mech_los_broadcast(mech, "tumbles over, staggered by the damage!");
        mech_fall(mech, 1, 0);
      }
    }
  }
}

int calcNewStaggerBTHMod(Mech *mech, int staggerLevel) {
  int bthMod = 0;
  int tonnageMod = 0;

  if (!Started(mech)) {
    bthMod = 999;
  } else {
    bthMod = staggerLevel - 1;

    if (MechTons(mech) <= 35)
      tonnageMod = 1;
    else if (MechTons(mech) <= 55)
      tonnageMod = 0;
    else if (MechTons(mech) <= 75)
      tonnageMod = -1;
    else
      tonnageMod = -2;

    // disable tonnage mods if so configured
    if (btech_context_stagger_uses_tonnage(mech_context(mech)))
      bthMod += tonnageMod;
  }

  return bthMod;
}

static int factoral(int n) {
  int i, j = 0;

  for (i = 1; i <= n; i++)
    j += i;
  return j;
}

void mech_standfail_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_notify(mech, MECHALL,
              "[fg=green]You have finally recovered from your attempt to "
              "stand.[reset]");
}

void mech_fall_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long fallspeed = (long)e->data2;
  int fallen_elev;

  if (Started(mech) && fallspeed >= 0)
    return;
  if (fallspeed <= 0 &&
      (!Started(mech) || !(FlyingT(mech)) ||
       ((AeroFuel(mech) <= 0) && !mech_aero_has_free_fuel(mech)) ||
       ((MechType(mech) == CLASS_VTOL) && (SectIsDestroyed(mech, ROTOR)))))
    fallspeed -= FALL_ACCEL;
  else
    fallspeed += FALL_ACCEL;
  MarkForLOSUpdate(mech);
  if (MechsElevation(mech) > labs(fallspeed)) {
    MechZ(mech) -= labs(fallspeed);
    MechFZ(mech) = MechZ(mech) * ZSCALE;
    mech_event_schedule(mech, EVENT_FALL, mech_fall_event, FALL_TICK,
                        fallspeed);
    return;
  }
  /* Time to hit da ground */
  fallen_elev = factoral(labs(fallspeed));
  mech_notify(mech, MECHALL, "You hit the ground!");
  mech_los_broadcast(mech, "hits the ground!");
  mech_fall(mech, fallen_elev, 0);
  MechStatus(mech) &= ~JUMPING;
}

/* This is just a 'toy' event */
void mech_lock_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  Mech *target;

  if (MechTarget(mech) >= 0) {
    target = btech_context_find_object(mech->xcode.context, MechTarget(mech));
    if (!target)
      return;
    if (!mech_los_check(mech, target, MechX(target), MechY(target),
                        FaMechRange(mech, target)))
      return;
    mech_printf(mech, MECHALL, "The sensors acquire a stable lock on %s.",
                mech_to_mech_display_id(mech, target).text);
  } else if (MechTargX(mech) >= 0 && MechTargY(mech) >= 0)
    mech_printf(mech, MECHALL, "The sensors acquire a stable lock on (%d,%d).",
                MechTargX(mech), MechTargY(mech));
}

/* Various events that don't fit too well to other categories */

/* Basically the update events + some movenement events */
void mech_stabilizing_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_notify(mech, MECHSTARTED,
              "[fg=green]You have finally stabilized after your jump.[reset]");
}

void mech_jump_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_event_schedule(mech, EVENT_JUMP, mech_jump_event, JUMP_TICK, 0);
  mech_movement_update(mech);
  if (!Jumping(mech))
    mech_event_cancel(mech, EVENT_JUMP);
}

extern const int PilotStatusRollNeeded[];

void mech_recovery_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (Destroyed(mech) || !Uncon(mech))
    return;
  if (handlemwconc(mech, 0)) {
    MechStatus(mech) &= ~UNCONSCIOUS;
    mech_notify(mech, MECHALL, "The pilot regains consciousness!");
    return;
  }
}

void mech_unconsciousness_extend(Mech *mech, int len) {
  int l;

  if (Destroyed(mech))
    return;
  if (!mech_event_count(mech, EVENT_RECOVERY)) {
    MechStatus(mech) |= UNCONSCIOUS;
    mech_event_schedule(mech, EVENT_RECOVERY, mech_recovery_event, len, 0);
    return;
  }
  l = mux_event_last_type_data(mech->xcode.context->events, EVENT_RECOVERY,
                               (void *)mech) +
      len;
  mux_event_remove_type_data(mech->xcode.context->events, EVENT_RECOVERY,
                             (void *)mech);
  mech_event_schedule(mech, EVENT_RECOVERY, mech_recovery_event, l, 0);
}

struct foo {
  char *name;
  char *full;
  int ofs;
};
extern struct foo lateral_modes[];

#ifdef BT_MOVEMENT_MODES
void mech_sideslip_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  int roll;

  if (!mech || !Started(mech))
    return;
  mech_notify(mech, MECHALL, "You make a skill roll while sideslipping!");
  if (!MadePilotSkillRoll(mech,
                          HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                                           "maneuvering_ace")
                              ? -1
                              : 0)) {
    mech_notify(mech, MECHALL, "You fail and spin out!");
    mech_los_broadcast(mech, "spins out while sideslipping!");
    MechSpeed(mech) = 0.0;
    roll = btech_random_range(mech->xcode.context, 0, 5);
    AddFacing(mech, roll * 60);
    SetFacing(mech, AcceptableDegree(MechFacing(mech)));
    MechDesiredFacing(mech) = MechFacing(mech);
    MechDesiredSpeed(mech) = 0.0;
    MechLateral(mech) = 0;
    return;
  }
  mech_event_schedule(mech, EVENT_SIDESLIP, mech_sideslip_event, TURN, 0);
}
#endif

void mech_lateral_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long latmode = (long)e->data2;

  if (!mech || !Started(mech))
    return;
  mech_printf(mech, MECHALL,
              "Lateral movement mode change to %s (%d offset) completed.",
              lateral_modes[latmode].full, lateral_modes[latmode].ofs);
  MechLateral(mech) = lateral_modes[latmode].ofs;
#ifdef BT_MOVEMENT_MODES
  if (MechMove(mech) != MOVE_QUAD) {
    if (MechLateral(mech) == 0)
      mech_event_cancel(mech, EVENT_SIDESLIP);
    else if (!(mech_event_count(mech, EVENT_SIDESLIP)))
      mech_event_schedule(mech, EVENT_SIDESLIP, mech_sideslip_event, 1, 0);
  }
#endif
}

void mech_move_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (MechType(mech) == CLASS_VTOL)
    if (Landed(mech) || aero_fuel_check(mech))
      return;
  mech_heading_update(mech);
  if ((IsMechLegLess(mech)) || Jumping(mech) || OODing(mech)) {
    if (MechDesiredFacing(mech) != MechFacing(mech))
      mech_event_schedule(mech, EVENT_MOVE, mech_move_event, MOVE_TICK, 0);
    return;
  }
  mech_speed_update(mech);
  mech_movement_update(mech);

  if (mech->mapindex < 0)
    return;

  if (MechType(mech) == CLASS_VEH_NAVAL &&
      mech_real_terrain_get(mech) != BRIDGE &&
      mech_real_terrain_get(mech) != ICE &&
      mech_real_terrain_get(mech) != WATER)
    return;

  if (MechSpeed(mech) || MechDesiredSpeed(mech) ||
      MechDesiredFacing(mech) != MechFacing(mech) ||
      ((MechType(mech) == CLASS_VTOL || MechMove(mech) == MOVE_SUB) &&
       MechVerticalSpeed(mech)))
    mech_event_schedule(mech, EVENT_MOVE, mech_move_event, MOVE_TICK, 0);
}

void mech_stand_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  mech_los_broadcast(mech, "stands up!");
  mech_notify(mech, MECHALL, "You have finally finished standing up.");
  mech_make_stand(mech);
}

void mech_plos_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data, *target;
  BattleMap *map;
  int mapvis;
  int maplight;
  float range;
  int i;

  if (!Started(mech))
    return;
  if (!(map = btech_context_get_map(mech->xcode.context, mech->mapindex)))
    return;
  mech_event_schedule(mech, EVENT_PLOS, mech_plos_event, PLOS_TICK, 0);
  if (!MechPNumSeen(mech) && !(MechSpecials(mech) & AA_TECH))
    return;
  mapvis = map->mapvis;
  maplight = map->maplight;
  MechPNumSeen(mech) = 0;
  for (i = 0; i < map->first_free; i++)
    if (map->mechsOnMap[i] > 0 && map->mechsOnMap[i] != mech->mynum)
      if (!(map->LOSinfo[mech->mapnumber][i] & MECHLOSFLAG_SEEN)) {
        target =
            btech_context_find_object(mech->xcode.context, map->mechsOnMap[i]);
        if (!target)
          continue;
        range = FlMechRange(map, mech, target);
        MechPNumSeen(mech)++;
        mech_sensor_visibility_update(mech, &map->LOSinfo[mech->mapnumber][i],
                                      range, -1, -1, target, mapvis, maplight,
                                      map->cloudbase, 1, 0);
      }
}

void aero_move_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!Landed(mech)) {
    /* Returns 1 only if we
       1) Ran out of fuel, and
       2) Were VTOL, and
       3) Crashed
     */
    if (aero_fuel_check(mech))
      return;
    /* Genuine CHEAT :-) */
    if (Started(mech)) {
      aero_heading_update(mech);
      aero_speed_update(mech);
    }
    if (Fallen(mech))
      MechStartFZ(mech) = MechStartFZ(mech) - 1;
    mech_movement_update(mech);
    if (IsDS(mech) && MechZ(mech) <= (MechElevation(mech) + 5) &&
        ((mech->xcode.context->events->tick / WEAPON_TICK) % 10) == 0)
      DS_BlastNearbyMechsAndTrees(
          mech, "You are hit by the DropShip's plasma exhaust!",
          "is hit directly by DropShip's exhaust!",
          "You are hit by the DropShip's plasma exhaust!",
          "is hit by DropShip's exhaust!", "light up and burn.", 8);
    mech_event_schedule(mech, EVENT_MOVE, aero_move_event, MOVE_TICK, 0);
  } else if (Landed(mech) && !Fallen(mech) && RollingT(mech)) {
    mech_heading_update(mech);
    mech_speed_update(mech);
    mech_movement_update(mech);
    if (fabs(MechSpeed(mech)) > 0.0 || fabs(MechDesiredSpeed(mech)) > 0.0 ||
        MechDesiredFacing(mech) != MechFacing(mech))
      if (!aero_fuel_check(mech))
        mech_event_schedule(mech, EVENT_MOVE, aero_move_event, MOVE_TICK, 0);
  }
}

void very_fake_func(MuxEvent *e) {}

/*
 * Exile Stun Code Event
 */
void mech_crewstun_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (!mech)
    return;
  if (!Started(mech) || Destroyed(mech)) {
    if (MechCritStatus(mech) & MECH_STUNNED)
      MechCritStatus(mech) &= ~MECH_STUNNED;
    return;
  }
  if (MechType(mech) != CLASS_MECH)
    mech_notify(
        mech, MECHALL,
        "[fg=green bold]The crew recovers from their bewilderment![reset]");
  else
    mech_notify(
        mech, MECHALL,
        "[fg=green bold]You recover from your stunning experience![reset]");

  if (MechCritStatus(mech) & MECH_STUNNED)
    MechCritStatus(mech) &= ~MECH_STUNNED;
}

void unstun_crew_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;

  if (mech_event_count(mech, EVENT_UNSTUN_CREW) >
      1) /* If we've been stunned again! */
    return;

  mech_notify(
      mech, MECHALL,
      "Your head clears and you're able to control your vehicle again.");
  MechTankCritStatus(mech) &= ~CREW_STUNNED;
}

void mech_unjam_ammo_event(MuxEvent *objEvent) {
  Mech *objMech = (Mech *)objEvent->data; /* get the mech */
  long wWeapNum = (long)objEvent->data2;  /* and now the weapon number */
  int wSect, wSlot, wWeapStatus, wWeapIdx;
  int ammoLoc, ammoCrit, ammoLoc1, ammoCrit1;
  int wRoll = 0;
  int wRollNeeded = 0;

  if (Uncon(objMech) || !Started(objMech))
    return;

  wWeapStatus = FindWeaponNumberOnMech(objMech, wWeapNum, &wSect, &wSlot);

  if (wWeapStatus ==
      TIC_NUM_DESTROYED) /* return if the weapon has been destroyed */
    return;

  wWeapIdx = FindWeaponIndex(objMech, wWeapNum);

  if (!FindAndCheckAmmo(objMech, wWeapIdx, wSect, wSlot, &ammoLoc, &ammoCrit,
                        &ammoLoc1, &ammoCrit1, 0)) {
    SetPartTempNuke(objMech, wSect, wSlot, 0);

    mech_printf(
        objMech, MECHALL,
        "You finish bouncing around and realize you no longer have "
        "ammo for your %s!",
        get_parts_long_name(objMech->xcode.context, I2Weapon(wWeapIdx), 0));
    return;
  }

  if (MechWeapons[wWeapStatus].special & RAC) {
    wRoll = btech_random_roll(objMech->xcode.context);
    wRollNeeded = FindPilotGunnery(objMech, wWeapStatus) + 3;

    mech_notify(objMech, MECHPILOT, "You make a roll to unjam the weapon!");
    mech_printf(objMech, MECHPILOT, "Modified Gunnery Skill: BTH %d\tRoll: %d",
                wRollNeeded, wRoll);

    if (wRoll < wRollNeeded) {
      mech_notify(objMech, MECHALL,
                  "Your attempt to remove the jammed slug fails. You'll need "
                  "to try again to clear it.");
      return;
    }
  } else {
    if (!MadePilotSkillRoll(objMech, 0)) {
      mech_notify(objMech, MECHALL,
                  "Your attempt to remove the jammed slug fails. You'll need "
                  "to try again to clear it.");
      return;
    }
  }

  SetPartTempNuke(objMech, wSect, wSlot, 0);
  mech_printf(
      objMech, MECHALL, "You manage to clear the jam on your %s!",
      get_parts_long_name(objMech->xcode.context, I2Weapon(wWeapIdx), 0));
  mech_los_broadcast(objMech, "ejects a mangled shell!");

  mech_ammunition_decrement(objMech, wWeapNum, wSect, wSlot, ammoLoc, ammoCrit,
                            ammoLoc1, ammoCrit1, 0);
}

void check_stagger_event(MuxEvent *event) {
  Mech *mech = (Mech *)event->data; /* get the mech */

  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("Triggered stagger check for %ld.", mech->mynum));

  if ((StaggerLevel(mech) < 1) || Fallen(mech) ||
      (MechType(mech) != CLASS_MECH)) {
    mech_stop_stagger_check(mech);
    return;
  }

  if (Jumping(mech)) {
    return;
  }

  mech_notify(mech, MECHALL, "You stagger from the damage!");
  if (!MadePilotSkillRoll(mech, calcStaggerBTHMod(mech))) {
    mech_notify(mech, MECHALL,
                "You loose the battle with gravity and tumble over!!");
    mech_los_broadcast(mech, "tumbles over, staggered by the damage!");
    mech_fall(mech, 1, 0);
  }

  mech_stop_stagger_check(mech);
  /* Since stagger rolls happen much more often now, this adds 10 damage
   * points of 'buffer' to mech that was just forced to make a stager roll.
   * Mechs whose damage accumulation times out without making a roll (<20
   * damage) don't get this help. This 10 points of damage assistance slowly
   * times out in mech_damage_stagger_check, or can be erased by weapons fire */
  mech->rd.staggerDamage = -10;
}

#ifdef BT_MOVEMENT_MODES
void mech_movemode_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long i = (long)e->data2;
  int dir = (i & MODE_ON ? 1 : i & MODE_OFF ? 0 : 0);

  if (!mech)
    return;

  if (!Started(mech) || Destroyed(mech)) {
    MechStatus2(mech) &= ~(MOVE_MODES);
    return;
  }
  if (dir) {
    if (i & MODE_EVADE) {
      MechStatus2(mech) |= EVADING;
      mech_notify(mech, MECHALL,
                  "You bounce chaotically as you maximize your movement mode "
                  "to evade!");
      mech_los_broadcast(
          mech,
          "suddenly begins to move erratically performing evasive maneuvers!");
    } else if (i & MODE_SPRINT) {
      MechStatus2(mech) |= SPRINTING;
      mech_notify(mech, MECHALL,
                  "You shimmy side to side as you get more speed from your "
                  "movement mode.");
      if ((MechType(mech) == CLASS_MECH) || (MechType(mech) == CLASS_BSUIT))
        mech_los_broadcast(mech, "breaks out into a full blown stride as it "
                                 "sprints over the terrain!");
      else
        mech_los_broadcast(mech,
                           "shifts into high gear as it gains more speed!");
      if (MechSpeed(mech) < 0) {
        mech_notify(mech, MECHALL,
                    "You stop your backward momemtum while sprinting and come "
                    "to a stop!");
        MechDesiredSpeed(mech) = 0;
      }
    } else if (i & MODE_DODGE) {
      if (mech_recycling_state(mech, CHECK_PHYS) > 0) {
        mech_notify(mech, MECHALL,
                    "You cannot enter DODGE mode due to physical useage.");
        return;
      } else {
        MechStatus2(mech) |= DODGING;
        mech_notify(mech, MECHALL,
                    "You brace yourself for any oncoming physicals.");
      }
    }
  } else {
    if (i & MODE_EVADE) {
      MechStatus2(mech) &= ~EVADING;
      mech_notify(
          mech, MECHALL,
          "Cockpit movement normalizes as you cease your evasive maneuvers.");
      mech_los_broadcast(mech, "ceases its evasive behavior and calms down.");
    } else if (i & MODE_SPRINT) {
      MechStatus2(mech) &= ~SPRINTING;
      mech_notify(mech, MECHALL,
                  "You feel less seasick as you leave your sprint mode and "
                  "resume normal movement.");
      mech_los_broadcast(mech, "slows down and enters a normal movement mode.");
    } else if (i & MODE_DODGE) {
      MechStatus2(mech) &= ~DODGING;
      if (i & MODE_DG_USED)
        mech_notify(mech, MECHALL,
                    "Your dodge maneuver has been used and you are no longer "
                    "braced for physicals.");
      else
        mech_notify(mech, MECHALL,
                    "You loosen up your stance and no longer dodge physicals.");
    }
  }
  if (MechSpeed(mech) > MMaxSpeed(mech) ||
      MechDesiredSpeed(mech) > MMaxSpeed(mech))
    MechDesiredSpeed(mech) = MMaxSpeed(mech);
  return;
}
#endif

int calcStaggerBTHMod(Mech *mech) {
  int bthMod = 0;
  int tonnageMod = 0;

  if (!Started(mech)) {
    bthMod = 999;
  } else {
    bthMod = StaggerLevel(mech);

    if (MechTons(mech) <= 35)
      tonnageMod = 1;
    else if (MechTons(mech) <= 55)
      tonnageMod = 0;
    else if (MechTons(mech) <= 75)
      tonnageMod = -1;
    else
      tonnageMod = -2;

    bthMod += tonnageMod;
  }

  return bthMod;
}
