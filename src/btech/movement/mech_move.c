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
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_specification_api.h"
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

struct {
  char *name;
  char *full;
  int ofs;
} lateral_modes[] = {{"nw", "Front/Left", 300}, {"fl", "Front/Left", 300},
                     {"ne", "Front/Right", 60}, {"fr", "Front/Right", 60},
                     {"sw", "Rear/Left", 240},  {"rl", "Rear/Left", 240},
                     {"se", "Rear/Right", 120}, {"rr", "Rear/Right", 120},
                     {"-", "None", 0},          {NULL, NULL, 0}};

const char *LateralDesc(Mech *mech) {
  int i;

  for (i = 0; MechLateral(mech) != lateral_modes[i].ofs; i++)
    ;
  return lateral_modes[i].full;
}

void mech_lateral(DbRef player, void *data, char *buffer) {

  /* Rule Reference: BMR Revised, Page 82 (Quad Lateral) */
  /* Rule Reference: MaxTech Revised, Page 46 (All Units w/ Maneuvering Ace) */
  /* Rule Reference: MaxTech Revised, Page 29 (VTOL/Hover Lateral) */
  /* Rule Reference: Total Warfare, Page 50 (Quad Lateral) */
  /* Rule Reference: Total Warfare, Page 67 (VTOL/Hover Lateral, though doesn't
   * say intentional) */

  Mech *mech = (Mech *)data;
  long i;

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(
      mech->xcode.context,
      !((MechIsQuad(mech) && (CountDestroyedLegs(mech) == 0)) ||
        ((MechType(mech) == CLASS_VTOL) || (MechType(mech) == MOVE_HOVER)) ||
        ((HasBoolAdvantage(mech->xcode.context, player, "maneuvering_ace") &&
          (MechPilot(mech) == player)))),
      "You cannot alter your lateral movement!");

  skipws(buffer);

  for (i = 0; lateral_modes[i].name; i++)
    if (!strcasecmp(lateral_modes[i].name, buffer))
      break;
  DOCHECK_CONTEXT(mech->xcode.context, !lateral_modes[i].name, "Invalid mode!");

  if (lateral_modes[i].ofs == MechLateral(mech)) {
    DOCHECK_CONTEXT(mech->xcode.context, !mech_event_count(mech, EVENT_LATERAL),
                    "You are going that way already!");
    mech_notify(mech, MECHALL, "Lateral mode change aborted.");
    mech_event_cancel(mech, EVENT_LATERAL);
    return;
  }

  mech_printf(mech, MECHALL,
              "Wanted lateral movement mode changed to %s (%d offset).",
              lateral_modes[i].full, lateral_modes[i].ofs);
  mech_event_cancel(mech, EVENT_LATERAL);
  mech_event_schedule(mech, EVENT_LATERAL, mech_lateral_event, LATERAL_TICK, i);
}

void mech_turnmode(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!mech_has_pilot(mech) || MechPilot(mech) != player) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "You're not the pilot!");
    return;
  }

  if (!HasBoolAdvantage(mech->xcode.context, player, "maneuvering_ace")) {
    mech_notify(mech, MECHPILOT, "You're not skilled enough to do that.");
    return;
  }

  if (buffer && !strcasecmp(buffer, "tight")) {
    SetTurnMode(mech, 1);
    mech_notify(mech, MECHALL, "You brace for tighter turns.");
    return;
  }
  if (buffer && !strcasecmp(buffer, "normal")) {
    SetTurnMode(mech, 0);
    mech_notify(mech, MECHALL, "You assume a normal turn mode.");
    return;
  }
  mech_printf(mech, MECHALL, "Your turning type is : %s",
              GetTurnMode(mech) ? "TIGHT" : "NORMAL");
  return;
}

void mech_bootlegger(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  float fMinSpeed = (4 * MP1);
  int wBTHMod = 0;
  int wFallLevels = 0;
  int i;
  int wHeadingChange = 0;
  int wNewHeading;
  float fMechSpeed = MechSpeed(mech);
  int wMechTons = MechTons(mech);
  char strLocation[50];
  char *args[1];

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  DOCHECK_CONTEXT(mech->xcode.context, CountDestroyedLegs(mech) > 0,
                  "You can't perform a bootlegger with destroyed legs!");
  DOCHECK_CONTEXT(mech->xcode.context, fMechSpeed < fMinSpeed,
                  tprintf("You are going too slow to perform a bootlegger! The "
                          "required minimum speed is %4.1f KPH.",
                          fMinSpeed));

  switch (toupper(args[0][0])) {
  case 'R':
    wHeadingChange = 90;
    break;
  case 'L':
    wHeadingChange = -90;
    break;
  }

  DOCHECK_CONTEXT(mech->xcode.context, wHeadingChange == 0,
                  "Invalid turn direction!");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if ((i == LLEG) || (i == RLEG) ||
        (MechIsQuad(mech) && ((i == LARM) || (i == RARM)))) {
      ArmorStringFromIndex(i, strLocation, MechType(mech), MechMove(mech));

      if (SectHasBusyWeap(mech, i)) {
        mech_printf(mech, MECHALL, "You have weapons recycling in your %s.",
                    strLocation);
        return;
      }

      if (MechSections(mech)[i].recycle) {
        mech_printf(mech, MECHALL,
                    "Your %s is still recovering from its last action.",
                    strLocation);
        return;
      }

      wBTHMod += MechSections(mech)[i].basetohit;
    }
  }

  if (fMechSpeed <= (4 * MP1)) {
    wBTHMod += 0;
  } else if (fMechSpeed <= (8 * MP1)) {
    wBTHMod += 1;
  } else if (fMechSpeed <= (12 * MP1)) {
    wBTHMod += 2;
  } else {
    wBTHMod += 3;
  }

  if (wMechTons <= 35) {
    wBTHMod += 0;
  } else if (wMechTons <= 55) {
    wBTHMod += 1;
  } else if (wMechTons <= 75) {
    wBTHMod += 2;
  } else {
    wBTHMod += 3;
  }

  wBTHMod += (InWater(mech) ? 2 : 0);

  wBTHMod = MAX(wBTHMod, 1);

  skipws(buffer);

  btech_channel_send(
      mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("#%ld attempts to do a bootlegger (mech). Tonnage: %d, "
              "Speed: %4.1f, BTHMod: %d",
              mech->mynum, wMechTons, fMechSpeed, wBTHMod));

  if (MadePilotSkillRoll(mech, wBTHMod)) {
    wNewHeading = AcceptableDegree(MechFacing(mech) + wHeadingChange);

    SetFacing(mech, wNewHeading);
    MechDesiredFacing(mech) = wNewHeading;
    MechSpeed(mech) = MechSpeed(mech) / 2;

    mech_printf(mech, MECHALL,
                "You plant a foot and swivel, changing your heading to %d.",
                wNewHeading);

    for (i = 0; i < NUM_SECTIONS; i++) {
      if ((i == LLEG) || (i == RLEG) ||
          (MechIsQuad(mech) && ((i == LARM) || (i == RARM))))
        mech_set_recycle_limb(mech, i, 30);
    }

  } else {
    wFallLevels = MAX(wBTHMod, 1);

    mech_notify(mech, MECHALL, "You plant a foot and try to swivel...");
    mech_notify(
        mech, MECHALL,
        "... but realize a little late that this is harder than it looks!");
    mech_los_broadcast(mech,
                       "attempts to fight the forces of inertia but looses "
                       "the battle miserably!");

    if (wFallLevels > 2)
      mech_los_broadcast(mech, "tumbles over and over and over!");

    MechFalls(mech, wFallLevels, 1);
  }
}

void mech_eta(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  int argc, eta_x, eta_y;
  float fx, fy, range;
  int etahr, etamin;
  char *args[3];

  cch(MECH_USUAL);
  argc = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech->xcode.context, argc == 1,
                  "Invalid number of arguments!");
  switch (argc) {
  case 0:
    DOCHECK_CONTEXT(mech->xcode.context,
                    !(MechTargX(mech) >= 0 && MechTarget(mech) < 0),
                    "You have invalid default target for ETA!");
    eta_x = MechTargX(mech);
    eta_y = MechTargY(mech);
    break;
  case 2:
    eta_x = atoi(args[0]);
    eta_y = atoi(args[1]);
    break;
  default:
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Invalid arguments!");
    return;
  }
  MapCoordToRealCoord(eta_x, eta_y, &fx, &fy);
  range = FindRange(MechFX(mech), MechFY(mech), 0, fx, fy, 0);
  if (fabs(MechSpeed(mech)) < 0.1)
    mech_printf(mech, MECHALL,
                "Range to hex (%d,%d) is %.1f.  ETA: Never, mech not moving.",
                eta_x, eta_y, range);
  else {
    etamin = (int)fabs(range / (MechSpeed(mech) / KPH_PER_MP));
    etahr = etamin / 60;
    etamin = etamin % 60;
    mech_printf(mech, MECHALL, "Range to hex (%d,%d) is %.1f.  ETA: %.2d:%.2d.",
                eta_x, eta_y, range, etahr, etamin);
  }
}

float mech_cargo_maximum_speed(Mech *mech, float mspeed) {
  int lugged = 0, mod = 2;
  Mech *c;
  BattleMap *map;

  if (MechCarrying(mech) > 0) { /* Ug-lee! */
    Mech *t;

    if ((t = btech_context_get_mech(mech->xcode.context, MechCarrying(mech))))
      if (!(MechCritStatus(t) & OWEIGHT_OK))
        MechCritStatus(mech) &= ~LOAD_OK;
  }

  /*! \todo {Fix this calculation to include gravity and TSM for
   * when BT_MOVMENT_MODES is enabled} */
  if ((MechCritStatus(mech) & LOAD_OK) && (MechCritStatus(mech) & OWEIGHT_OK) &&
      (MechCritStatus(mech) & SPEED_OK)) {

    mspeed = MechRMaxSpeed(mech);
#if 0

		/* Is masc and/or scharge on */
		if((MechStatus(mech) & MASC_ENABLED) &&
		   (MechStatus(mech) & SCHARGE_ENABLED))
			mspeed = ceil((rint(mspeed / 1.5) / MP1) * 2.5) * MP1;
		else if(MechStatus(mech) & MASC_ENABLED)
			mspeed *= 4. / 3.;
		else if(MechStatus(mech) & SCHARGE_ENABLED)
			mspeed *= 4. / 3.;

		if(InSpecial(mech) && InGravity(mech))
			if((map = btech_context_find_object(mech->xcode.context, mech->mapindex)))
				mspeed = mspeed * 100 / MAX(50, MapGravity(map));

#else
    /* Is masc and/or scharge and/or sprinting on */
    if (MechStatus(mech) & (MASC_ENABLED | SCHARGE_ENABLED) ||
        MechStatus2(mech) & SPRINTING)
      mspeed = WalkingSpeed(mspeed) *
               (1.5 + (MechStatus(mech) & MASC_ENABLED ? 0.5 : 0.0) +
                (MechStatus(mech) & SCHARGE_ENABLED ? 0.5 : 0.0) +
                (MechStatus2(mech) & SPRINTING ? 0.5 : 0.0));

    if ((MechSpecials(mech) & TRIPLE_MYOMER_TECH) && (MechHeat(mech) >= 9.)) {
      if ((MechStatus2(mech) & SPRINTING)) {
        if (mech->xcode.context->configuration->btech_tsm_sprint_bonus)
          mspeed = ceil((rint((mspeed / 1.5) / MP1) + 1) * 1.5) * MP1;

      } else {
        mspeed = ceil((rint((mspeed / 1.5) / MP1) + 1) * 1.5) * MP1;
      }
    }

    /* if the player has speed demon give him his boost in speed */
    if (!mech_event_count(mech, EVENT_MOVEMODE) &&
        MechStatus2(mech) & SPRINTING &&
        HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "speed_demon"))
      mspeed += MP1;

    if (InSpecial(mech) && InGravity(mech))
      if ((map =
               btech_context_find_object(mech->xcode.context, mech->mapindex)))
        mspeed = mspeed * 100.0 / (float)MAX(50, MapGravity(map));

#endif
    return mspeed;
  }
  MechRTonsV(mech) = mech_calculated_weight(mech);

  /*! \todo {Check some of this math better} */
  if (!(MechCritStatus(mech) & LOAD_OK)) {
    if (MechCarrying(mech) > 0)
      if ((c = btech_context_get_mech(mech->xcode.context,
                                      MechCarrying(mech)))) {
        lugged = mech_calculated_weight(c) * 2;
        if (MechSpecials(mech) & SALVAGE_TECH)
          lugged = lugged / 2;
        if ((MechSpecials(mech) & TRIPLE_MYOMER_TECH) &&
            (MechHeat(mech) >= 9.) &&
            mech->xcode.context->configuration->btech_tsm_tow_bonus)
          lugged = lugged / 2;

        if (MechSpecials2(mech) & CARRIER_TECH)
          lugged = lugged / 2;
      }

    if (MechSpecials(mech) & CARGO_TECH)
      mod = 1;

    if (MechType(mech) == CLASS_MECH)
      mod = mod * 2;

    lugged += MechCarriedCargo(mech) * mod / 2;
    MechRCTonsV(mech) = lugged;
    MechCritStatus(mech) |= LOAD_OK;
  }
  if (Destroyed(mech))
    mspeed = 0.0;
  else {
    int mv = MechRTonsV(mech);
    int sv = MechTons(mech) * 1024;

    if (mv == 1 && !Destroyed(mech))
      mv = sv;
    else {
      if (mv > sv)
        mv = mv + (mv - sv) / 2;
      else
        mv = mv + (sv - mv) / 3;
    }
    if (3 * sv < (MechRCTonsV(mech) + mv))
      mspeed = 0.0;
    else
#ifdef WEIGHT_OVERSPEEDING
      mspeed = MechMaxSpeed(mech) * MechTons(mech) * 1024.0 /
               MAX(1024 * MechRealTons(mech) + MechRCTonsV(mech) / 3,
                   (MAX(1024, mv + MechRCTonsV(mech))));
#else
      mspeed = MechMaxSpeed(mech) * MechTons(mech) * 1024.0 /
               MAX(1024 * MechTons(mech) + MechRCTonsV(mech) / 3,
                   (MAX(1024, mv + MechRCTonsV(mech))));
#endif /* WEIGHT_OVERSPEEDING */
  }
  MechRMaxSpeed(mech) = mspeed;
  MechWalkXPFactor(mech) = MAX(1, (int)mspeed / MP1) * 2;
  MechCritStatus(mech) |= SPEED_OK;
  return MMaxSpeed(mech);
}

float mech_effective_maximum_speed(Mech *mech) {
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
}

void mech_drop(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  float s1;
  int wDropLevels = 0;
  int wDropBTH = 0;
  int tHasSwarmers = 0;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_BSUIT,
                  "No crawling!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_MECH && MechType(mech) != CLASS_MW,
                  "You can't prone in this!");
  DOCHECK_CONTEXT(mech->xcode.context, Fallen(mech), "You are already prone.");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech) || OODing(mech),
                  "You can't prone in the air!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STAND),
                  "You can't drop while trying to stand up!");

  s1 = MMaxSpeed(mech) / 3.0;

  if ((MechType(mech) == CLASS_MECH) && CountSwarmers(mech))
    tHasSwarmers = 1;

  if (MechType(mech) != CLASS_MW && fabs(MechSpeed(mech)) > s1 * 2) {
    mech_notify(mech, MECHALL, "You attempt a controlled drop while running.");
    wDropLevels = 2;
    wDropBTH = 2;
  } else if (fabs(MechSpeed(mech)) > s1) {
    mech_notify(mech, MECHALL,
                "You attempt a controlled drop from your fast walk.");
    wDropLevels = 1;
  }

  if (Staggering(mech)) {
    mech_notify(mech, MECHALL,
                "Still staggering, you try not to fall on your face.");
    wDropLevels = (wDropLevels == 0 ? 1 : wDropLevels);
    wDropBTH = wDropBTH + StaggerLevel(mech);
  }

  if (tHasSwarmers)
    mech_notify(mech, MECHALL,
                "The suits hanging off you make a controlled drop harder!");

  if ((wDropLevels > 0) || tHasSwarmers) {
    if (MadePilotSkillRoll(mech, wDropBTH)) {
      mech_notify(mech, MECHALL, "You hit the ground with minimal damage");
      mech_los_broadcast(mech, "drops to the ground!");

      if (tHasSwarmers)
        StopBSuitSwarmers(
            btech_context_find_object(mech->xcode.context, mech->mapindex),
            mech, 0);

    } else {
      mech_notify(mech, MECHALL, "You fall to the ground hard");
      mech_los_broadcast(mech, "falls hard to the ground!");

      if (wDropLevels <= 0)
        wDropLevels = 1;

      if (tHasSwarmers)
        StopBSuitSwarmers(
            btech_context_find_object(mech->xcode.context, mech->mapindex),
            mech, 0);

      MechFalls(mech, wDropLevels, 1);
    }
  } else {
    mech_notify(mech, MECHALL, "You drop to the ground prone!");
    mech_los_broadcast(mech, "drops to the ground!");
  }

  mech_make_fall(mech);
  MechDesiredSpeed(mech) = 0;
  MechSpeed(mech) = 0;
  MechFloods(mech);
  water_extinguish_inferno(mech);

  // as per ps, prone clears stagger
  if (btech_context_stagger_mode(mech_context(mech)))
    mech_stagger_damage_clear(mech);

  possible_mine_poof(mech, MINE_STEP);
}

void mech_stand(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2];
  int wcDeadLegs = 0;
  int tNeedsPSkill = 1;
  int tDoStand = 1;
  int bth, mechstandtime, standanyway = 0, standcarefulmod = 0;
  int i;

  cch(MECH_USUAL);
  DOCHECK_CONTEXT(mech->xcode.context, MechType(mech) == CLASS_BSUIT,
                  "You're standing already!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  MechType(mech) != CLASS_MECH && MechType(mech) != CLASS_MW,
                  "This vehicle cannot stand like a 'Mech.");
  DOCHECK_CONTEXT(mech->xcode.context, Jumping(mech),
                  "You're standing while jumping!");
  DOCHECK_CONTEXT(mech->xcode.context, OODing(mech),
                  "You're standing while flying!");

  /* set the number of dead legs we have */
  wcDeadLegs = CountDestroyedLegs(mech);

  DOCHECK_CONTEXT(mech->xcode.context,
                  ((MechIsQuad(mech) && (wcDeadLegs > 3)) ||
                   (!MechIsQuad(mech) && (wcDeadLegs > 1))),
                  "You have no legs to stand on!");
  DOCHECK_CONTEXT(mech->xcode.context, wcDeadLegs > 2,
                  "You'd be far too unstable!");
  DOCHECK_CONTEXT(mech->xcode.context, MechCritStatus(mech) & GYRO_DESTROYED,
                  "You cannot stand with a destroyed gyro!");

  DOCHECK_CONTEXT(mech->xcode.context, !Fallen(mech),
                  "You're already standing!");
  DOCHECK_CONTEXT(mech->xcode.context, mech_event_count(mech, EVENT_STANDFAIL),
                  "You're still recovering from your last attempt!");
  DOCHECK_CONTEXT(mech->xcode.context, IsHulldown(mech),
                  "You can not stand while hulldown");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");

  bth = MechPilotSkillRoll_BTH(mech, 0);

  /* Check to see if the user specified an argument for the command */
  if (proper_explodearguments(buffer, args, 2)) {
    if (strcmp(args[0], "check") == 0) {
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Your BTH to stand would be: %d", bth);
      for (i = 0; i < 2; i++) {
        if (args[i])
          free(args[i]);
      }
      return;
    } else if (strcmp(args[0], "anyway") == 0) {
      standanyway = 1;
    } else if ((strcmp(args[0], "careful") == 0) &&
               mech->xcode.context->configuration->btech_standcareful) {
      standcarefulmod = -2;
    } else {
      notify_printf(btech_context_evaluation(mech->xcode.context), player,
                    "Unknown argument! use 'stand check'%s",
                    mech->xcode.context->configuration->btech_standcareful
                        ? ", 'stand careful' or 'stand anyway'"
                        : " or 'stand anyway'");
      for (i = 0; i < 2; i++) {
        if (args[i])
          free(args[i]);
      }
      return;
    }
  }

  DOCHECK_CONTEXT(
      mech->xcode.context, !standanyway && bth > 12,
      "You would fail; use 'stand anyway' if you really want to stand.");

  mech_make_stand(mech);

  /*  quads with all 4 legs don't have to roll to stand */
  if (((wcDeadLegs == 0) && MechIsQuad(mech)) || (MechType(mech) == CLASS_MW)) {
    tNeedsPSkill = 0;
  }

  mech_los_broadcast(mech, "attempts to stand up.");

  if (mech_real_terrain_get(mech) == ICE && MechZ(mech) == -1)
    break_thru_ice(mech);

  if (tNeedsPSkill) {
    /* Changed to NoXP. Keeps people from doing pushups to gain Pilot XP */
    if (!MadePilotSkillRoll_NoXP(mech, standcarefulmod, 0)) {
      mech_notify(mech, MECHALL,
                  "You fail your attempt to stand and fall back on the ground");
      MechFalls(mech, 1, 1);
      mechstandtime = ((MechType(mech) == CLASS_MW) ? DROP_TO_STAND_RECYCLE / 3
                                                    : StandMechTime(mech));
      /* Not strictly FASA, but allows legged mechs to stand careful */
      if (standcarefulmod) {
        mechstandtime = MAX(30, mechstandtime * 2);
      }
      mech_event_schedule(mech, EVENT_STANDFAIL, mech_standfail_event,
                          mechstandtime, 0);
      tDoStand = 0;
    }
  }

  if (tDoStand) {
    /* Now we set a counter in goingy to keep him from moving or jumping until
     * he is finished standing */
    mech_notify(mech, MECHALL, "You begin to stand up.");
    mechstandtime = ((MechType(mech) == CLASS_MW) ? DROP_TO_STAND_RECYCLE / 3
                                                  : StandMechTime(mech));
    /* Not strictly FASA, but allows legged mechs to stand careful */
    if (standcarefulmod) {
      mechstandtime = mechstandtime * 2;
    }
    mech_event_schedule(mech, EVENT_STAND, mech_stand_event, mechstandtime, 0);
  }
  /* Free args */
  for (i = 0; i < 2; i++) {
    if (args[i])
      free(args[i]);
  }
}
