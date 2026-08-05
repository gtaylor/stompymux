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
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_heat_api.h"
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
#include "mech_specification_api.h"
#include "mech_stagger.h"
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

static int mech_movement_maximum_int(int first, int second) {
  return first > second ? first : second;
}

float mech_jump_speed_for_map(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (mech_is_under_gravity(mech) && map != nullptr) {
    speed =
        speed * 100 / mech_movement_maximum_int(50, battle_map_gravity(map));
  }
  return speed;
}

int mech_jump_speed_mp_for_map(const Mech *mech, const BattleMap *map) {
  return (int)(mech_jump_speed_for_map(mech, map) * MP_PER_KPH);
}

struct {
  char *name;
  char *full;
  int ofs;
} lateral_modes[] = {{"nw", "Front/Left", 300}, {"fl", "Front/Left", 300},
                     {"ne", "Front/Right", 60}, {"fr", "Front/Right", 60},
                     {"sw", "Rear/Left", 240},  {"rl", "Rear/Left", 240},
                     {"se", "Rear/Right", 120}, {"rr", "Rear/Right", 120},
                     {"-", "None", 0},          {nullptr, nullptr, 0}};

const char *mech_lateral_description(Mech *mech) {
  int i;

  for (i = 0; mech_lateral_movement(mech) != lateral_modes[i].ofs; i++)
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
      mech_context(mech),
      !(((mech_movement_type(mech) == MOVE_QUAD) &&
         (CountDestroyedLegs(mech) == 0)) ||
        ((mech_class(mech) == CLASS_VTOL) ||
         (mech_class(mech) == MOVE_HOVER)) ||
        ((HasBoolAdvantage(mech_context(mech), player, "maneuvering_ace") &&
          (mech_pilot_dbref(mech) == player)))),
      "You cannot alter your lateral movement!");

  skipws(buffer);

  for (i = 0; lateral_modes[i].name; i++)
    if (!strcasecmp(lateral_modes[i].name, buffer))
      break;
  DOCHECK_CONTEXT(mech_context(mech), !lateral_modes[i].name, "Invalid mode!");

  if (lateral_modes[i].ofs == mech_lateral_movement(mech)) {
    DOCHECK_CONTEXT(mech_context(mech), !mech_event_count(mech, EVENT_LATERAL),
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

  if (!mech_has_pilot(mech) || mech_pilot_dbref(mech) != player) {
    notify(btech_context_evaluation(mech_context(mech)), player,
           "You're not the pilot!");
    return;
  }

  if (!HasBoolAdvantage(mech_context(mech), player, "maneuvering_ace")) {
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
  float fMechSpeed = mech_current_speed(mech);
  int wMechTons = mech_tonnage(mech);
  char strLocation[50];
  char *args[1];

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech),
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  DOCHECK_CONTEXT(mech_context(mech), CountDestroyedLegs(mech) > 0,
                  "You can't perform a bootlegger with destroyed legs!");
  DOCHECK_CONTEXT(mech_context(mech), fMechSpeed < fMinSpeed,
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

  DOCHECK_CONTEXT(mech_context(mech), wHeadingChange == 0,
                  "Invalid turn direction!");

  for (i = 0; i < NUM_SECTIONS; i++) {
    if ((i == LLEG) || (i == RLEG) ||
        ((mech_movement_type(mech) == MOVE_QUAD) &&
         ((i == LARM) || (i == RARM)))) {
      ArmorStringFromIndex(i, strLocation, mech_class(mech),
                           mech_movement_type(mech));

      if (SectHasBusyWeap(mech, i)) {
        mech_printf(mech, MECHALL, "You have weapons recycling in your %s.",
                    strLocation);
        return;
      }

      if (mech_section_recycle_ticks(mech, i)) {
        mech_printf(mech, MECHALL,
                    "Your %s is still recovering from its last action.",
                    strLocation);
        return;
      }

      wBTHMod += mech_section_base_to_hit(mech, i);
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

  wBTHMod += (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
                      mech_position_z(mech) < 0
                  ? 2
                  : 0);

  wBTHMod = mech_movement_maximum_int(wBTHMod, 1);

  skipws(buffer);

  btech_channel_send(
      mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("#%ld attempts to do a bootlegger (mech). Tonnage: %d, "
              "Speed: %4.1f, BTHMod: %d",
              mech_dbref(mech), wMechTons, fMechSpeed, wBTHMod));

  if (MadePilotSkillRoll(mech, wBTHMod)) {
    wNewHeading = AcceptableDegree(mech_heading_degrees(mech) + wHeadingChange);

    SetFacing(mech, wNewHeading);
    mech_desired_heading_set(mech, wNewHeading);
    mech_current_speed_scale(mech, 0.5F);

    mech_printf(mech, MECHALL,
                "You plant a foot and swivel, changing your heading to %d.",
                wNewHeading);

    for (i = 0; i < NUM_SECTIONS; i++) {
      if ((i == LLEG) || (i == RLEG) ||
          ((mech_movement_type(mech) == MOVE_QUAD) &&
           ((i == LARM) || (i == RARM))))
        mech_set_recycle_limb(mech, i, 30);
    }

  } else {
    wFallLevels = mech_movement_maximum_int(wBTHMod, 1);

    mech_notify(mech, MECHALL, "You plant a foot and try to swivel...");
    mech_notify(
        mech, MECHALL,
        "... but realize a little late that this is harder than it looks!");
    mech_los_broadcast(mech,
                       "attempts to fight the forces of inertia but looses "
                       "the battle miserably!");

    if (wFallLevels > 2)
      mech_los_broadcast(mech, "tumbles over and over and over!");

    mech_fall(mech, wFallLevels, 1);
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
  DOCHECK_CONTEXT(mech_context(mech), argc == 1,
                  "Invalid number of arguments!");
  switch (argc) {
  case 0:
    DOCHECK_CONTEXT(mech_context(mech), !mech_targets_hex(mech),
                    "You have invalid default target for ETA!");
    eta_x = mech_target_hex_x(mech);
    eta_y = mech_target_hex_y(mech);
    break;
  case 2:
    eta_x = atoi(args[0]);
    eta_y = atoi(args[1]);
    break;
  default:
    notify(btech_context_evaluation(mech_context(mech)), player,
           "Invalid arguments!");
    return;
  }
  MapCoordToRealCoord(eta_x, eta_y, &fx, &fy);
  range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech), 0,
                    fx, fy, 0);
  if (fabs(mech_current_speed(mech)) < 0.1)
    mech_printf(mech, MECHALL,
                "Range to hex (%d,%d) is %.1f.  ETA: Never, mech not moving.",
                eta_x, eta_y, range);
  else {
    etamin = (int)fabs(range / (mech_current_speed(mech) / KPH_PER_MP));
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
  MechConditionSummary conditions = mech_condition_summary(mech);

  if (mech_carried_dbref(mech) > 0) { /* Ug-lee! */
    Mech *t;

    if ((t = btech_context_get_mech(mech_context(mech),
                                    mech_carried_dbref(mech))))
      if (!mech_weight_cache_is_valid(t))
        mech_load_cache_invalidate(mech);
  }

  /*! \todo {Fix this calculation to include gravity and TSM for
   * when BT_MOVMENT_MODES is enabled} */
  if (mech_load_cache_is_valid(mech) && mech_weight_cache_is_valid(mech) &&
      mech_speed_cache_is_valid(mech)) {

    mspeed = mech_cached_maximum_speed(mech);
    /* Is masc and/or scharge and/or sprinting on */
    if (conditions.masc_enabled || conditions.supercharger_enabled ||
        conditions.sprinting)
      mspeed = (2.0F * mspeed / 3.0F) *
               (1.5 + (conditions.masc_enabled ? 0.5 : 0.0) +
                (conditions.supercharger_enabled ? 0.5 : 0.0) +
                (conditions.sprinting ? 0.5 : 0.0));

    if ((mech_technology_flags(mech) & TRIPLE_MYOMER_TECH) &&
        (mech_excess_heat(mech) >= 9.)) {
      if (conditions.sprinting) {
        if (btech_context_uses_tsm_sprint_bonus(mech_context(mech)))
          mspeed = ceil((rint((mspeed / 1.5) / MP1) + 1) * 1.5) * MP1;

      } else {
        mspeed = ceil((rint((mspeed / 1.5) / MP1) + 1) * 1.5) * MP1;
      }
    }

    /* if the player has speed demon give him his boost in speed */
    if (!mech_event_count(mech, EVENT_MOVEMODE) && conditions.sprinting &&
        HasBoolAdvantage(mech_context(mech), mech_pilot_dbref(mech),
                         "speed_demon"))
      mspeed += MP1;

    if (mech_is_under_special_conditions(mech) && mech_is_under_gravity(mech))
      if ((map = btech_context_find_object(mech_context(mech),
                                           mech_map_dbref(mech))))
        mspeed = mspeed * 100.0 /
                 (float)(battle_map_gravity(map) > 50 ? battle_map_gravity(map)
                                                      : 50);

    return mspeed;
  }
  mech_cached_calculated_weight_set(mech, mech_calculated_weight(mech));

  /*! \todo {Check some of this math better} */
  if (!mech_load_cache_is_valid(mech)) {
    if (mech_carried_dbref(mech) > 0)
      if ((c = btech_context_get_mech(mech_context(mech),
                                      mech_carried_dbref(mech)))) {
        lugged = mech_calculated_weight(c) * 2;
        if (mech_technology_flags(mech) & SALVAGE_TECH)
          lugged = lugged / 2;
        if ((mech_technology_flags(mech) & TRIPLE_MYOMER_TECH) &&
            (mech_excess_heat(mech) >= 9.) &&
            btech_context_uses_tsm_tow_bonus(mech_context(mech)))
          lugged = lugged / 2;

        if (mech_technology_flags_secondary(mech) & CARRIER_TECH)
          lugged = lugged / 2;
      }

    if (mech_technology_flags(mech) & CARGO_TECH)
      mod = 1;

    if (mech_class(mech) == CLASS_MECH)
      mod = mod * 2;

    lugged += mech_carried_cargo_weight(mech) * mod / 2;
    mech_load_cache_record(mech, lugged);
  }
  if (mech_is_destroyed(mech))
    mspeed = 0.0;
  else {
    int mv = mech_cached_calculated_weight(mech);
    int sv = mech_tonnage(mech) * 1024;

    if (mv == 1 && !mech_is_destroyed(mech))
      mv = sv;
    else {
      if (mv > sv)
        mv = mv + (mv - sv) / 2;
      else
        mv = mv + (sv - mv) / 3;
    }
    if (3 * sv < (mech_cached_lugged_weight(mech) + mv))
      mspeed = 0.0;
    else
#ifdef WEIGHT_OVERSPEEDING
      mspeed = mech_maximum_speed(mech) * mech_tonnage(mech) * 1024.0 /
               mech_movement_maximum_int(
                   1024 * mech_real_tonnage(mech) +
                       mech_cached_lugged_weight(mech) / 3,
                   (mech_movement_maximum_int(
                       1024, mv + mech_cached_lugged_weight(mech))));
#else
      mspeed =
          mech_maximum_speed(mech) * mech_tonnage(mech) * 1024.0 /
          mech_movement_maximum_int(
              1024 * mech_tonnage(mech) + mech_cached_lugged_weight(mech) / 3,
              (mech_movement_maximum_int(
                  1024, mv + mech_cached_lugged_weight(mech))));
#endif /* WEIGHT_OVERSPEEDING */
  }
  mech_speed_cache_record(mech, mspeed,
                          mech_movement_maximum_int(1, (int)mspeed / MP1) * 2);
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
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
  DOCHECK_CONTEXT(mech_context(mech), mech_class(mech) == CLASS_BSUIT,
                  "No crawling!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "You can't prone in this!");
  DOCHECK_CONTEXT(mech_context(mech), mech_condition_summary(mech).fallen,
                  "You are already prone.");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_is_jumping(mech) || mech_is_out_of_control(mech),
                  "You can't prone in the air!");
  DOCHECK_CONTEXT(mech_context(mech), mech_event_count(mech, EVENT_STAND),
                  "You can't drop while trying to stand up!");

  s1 = mech_effective_maximum_speed(mech) / 3.0;

  if ((mech_class(mech) == CLASS_MECH) && CountSwarmers(mech))
    tHasSwarmers = 1;

  if (mech_class(mech) != CLASS_MW && fabs(mech_current_speed(mech)) > s1 * 2) {
    mech_notify(mech, MECHALL, "You attempt a controlled drop while running.");
    wDropLevels = 2;
    wDropBTH = 2;
  } else if (fabs(mech_current_speed(mech)) > s1) {
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
            btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
            mech, 0);

    } else {
      mech_notify(mech, MECHALL, "You fall to the ground hard");
      mech_los_broadcast(mech, "falls hard to the ground!");

      if (wDropLevels <= 0)
        wDropLevels = 1;

      if (tHasSwarmers)
        StopBSuitSwarmers(
            btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
            mech, 0);

      mech_fall(mech, wDropLevels, 1);
    }
  } else {
    mech_notify(mech, MECHALL, "You drop to the ground prone!");
    mech_los_broadcast(mech, "drops to the ground!");
  }

  mech_make_fall(mech);
  mech_movement_stop(mech);
  mech_flood(mech);
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
  DOCHECK_CONTEXT(mech_context(mech), mech_class(mech) == CLASS_BSUIT,
                  "You're standing already!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW,
                  "This vehicle cannot stand like a 'Mech.");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_jumping(mech),
                  "You're standing while jumping!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_out_of_control(mech),
                  "You're standing while flying!");

  /* set the number of dead legs we have */
  wcDeadLegs = CountDestroyedLegs(mech);

  DOCHECK_CONTEXT(
      mech_context(mech),
      (((mech_movement_type(mech) == MOVE_QUAD) && (wcDeadLegs > 3)) ||
       (!(mech_movement_type(mech) == MOVE_QUAD) && (wcDeadLegs > 1))),
      "You have no legs to stand on!");
  DOCHECK_CONTEXT(mech_context(mech), wcDeadLegs > 2,
                  "You'd be far too unstable!");
  DOCHECK_CONTEXT(mech_context(mech), mech_has_destroyed_gyro(mech),
                  "You cannot stand with a destroyed gyro!");

  DOCHECK_CONTEXT(mech_context(mech), !mech_condition_summary(mech).fallen,
                  "You're already standing!");
  DOCHECK_CONTEXT(mech_context(mech), mech_event_count(mech, EVENT_STANDFAIL),
                  "You're still recovering from your last attempt!");
  DOCHECK_CONTEXT(mech_context(mech), mech_condition_summary(mech).hull_down,
                  "You can not stand while hulldown");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");

  bth = mech_pilot_skill_roll_target(mech, 0);

  /* Check to see if the user specified an argument for the command */
  if (proper_explodearguments(buffer, args, 2)) {
    if (strcmp(args[0], "check") == 0) {
      notify_printf(btech_context_evaluation(mech_context(mech)), player,
                    "Your BTH to stand would be: %d", bth);
      for (i = 0; i < 2; i++) {
        if (args[i])
          free(args[i]);
      }
      return;
    } else if (strcmp(args[0], "anyway") == 0) {
      standanyway = 1;
    } else if ((strcmp(args[0], "careful") == 0) &&
               btech_context_stand_careful_modifier(mech_context(mech))) {
      standcarefulmod = -2;
    } else {
      notify_printf(btech_context_evaluation(mech_context(mech)), player,
                    "Unknown argument! use 'stand check'%s",
                    btech_context_stand_careful_modifier(mech_context(mech))
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
      mech_context(mech), !standanyway && bth > 12,
      "You would fail; use 'stand anyway' if you really want to stand.");

  mech_make_stand(mech);

  /*  quads with all 4 legs don't have to roll to stand */
  if (((wcDeadLegs == 0) && (mech_movement_type(mech) == MOVE_QUAD)) ||
      (mech_class(mech) == CLASS_MW)) {
    tNeedsPSkill = 0;
  }

  mech_los_broadcast(mech, "attempts to stand up.");

  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
      mech_position_z(mech) == -1)
    break_thru_ice(mech);

  if (tNeedsPSkill) {
    /* Changed to NoXP. Keeps people from doing pushups to gain Pilot XP */
    if (!MadePilotSkillRoll_NoXP(mech, standcarefulmod, 0)) {
      mech_notify(mech, MECHALL,
                  "You fail your attempt to stand and fall back on the ground");
      mech_fall(mech, 1, 1);
      mechstandtime =
          ((mech_class(mech) == CLASS_MW) ? DROP_TO_STAND_RECYCLE / 3
                                          : StandMechTime(mech));
      /* Not strictly FASA, but allows legged mechs to stand careful */
      if (standcarefulmod) {
        mechstandtime = mech_movement_maximum_int(30, mechstandtime * 2);
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
    mechstandtime = ((mech_class(mech) == CLASS_MW) ? DROP_TO_STAND_RECYCLE / 3
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
