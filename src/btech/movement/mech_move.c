/* Implements BattleTech movement mechanics for unit move. */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_heat_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

static int mech_movement_maximum_int(int first, int second) {
  return first > second ? first : second;
}

static int mech_stand_time(const Mech *mech) {
  const float speed_factor = mech_maximum_speed(mech) / MP2;
  const float bounded_factor = fminf(fmaxf(1.0F, speed_factor), 30.0F);
  const float delay = 30.0F / bounded_factor;
  return (int)delay;
}

typedef struct LateralMode {
  const char *name;
  const char *full;
  int ofs;
} LateralMode;

static const LateralMode lateral_modes[] = {
    {"nw", "Front/Left", 300}, {"fl", "Front/Left", 300},
    {"ne", "Front/Right", 60}, {"fr", "Front/Right", 60},
    {"sw", "Rear/Left", 240},  {"rl", "Rear/Left", 240},
    {"se", "Rear/Right", 120}, {"rr", "Rear/Right", 120},
    {"-", "None", 0},          {nullptr, nullptr, 0}};

static const LateralMode *lateral_mode(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(lateral_modes, 10, sizeof(*lateral_modes),
                                  (size_t)index);
}

static char *move_argument(char **arguments, size_t count, int index) {
  if (index < 0)
    abort();
  char **slot =
      checked_storage_at(arguments, count, sizeof(*arguments), (size_t)index);
  return *slot;
}

static void move_arguments_destroy(char **arguments, size_t count) {
  for (size_t index = 0; index < count; index++) {
    char **slot =
        checked_storage_at(arguments, count, sizeof(*arguments), index);
    free(*slot);
    *slot = nullptr;
  }
}

bool mech_lateral_mode_details(int mode, const char **description,
                               int *offset) {
  if (mode < 0 || mode >= 10 || lateral_mode(mode)->name == nullptr)
    return false;
  *description = lateral_mode(mode)->full;
  *offset = lateral_mode(mode)->ofs;
  return true;
}

const char *mech_lateral_description(Mech *mech) {
  int i;

  for (i = 0; mech_lateral_movement(mech) != lateral_mode(i)->ofs; i++)
    ;
  return lateral_mode(i)->full;
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

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(((mech_movement_type(mech) == MOVE_QUAD) &&
         (CountDestroyedLegs(mech) == 0)) ||
        ((mech_class(mech) == CLASS_VTOL) ||
         (mech_movement_type(mech) == MOVE_HOVER)) ||
        ((HasBoolAdvantage(mech_context(mech), player, "maneuvering_ace") &&
          (mech_pilot_dbref(mech) == player))))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot alter your lateral movement!");
    return;
  }

  const char *mode = buffer ? buffer : "";
  mode = checked_storage_at_const(mode, strlen(mode) + 1, sizeof(*mode),
                                  strspn(mode, " \t\r\n\f\v"));

  for (i = 0; lateral_mode((int)i)->name; i++)
    if (!strcasecmp(lateral_mode((int)i)->name, mode))
      break;
  if (!lateral_mode((int)i)->name) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid mode!");
    return;
  }

  if (lateral_mode((int)i)->ofs == mech_lateral_movement(mech)) {
    if (!mech_event_count(mech, EVENT_LATERAL)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You are going that way already!");
      return;
    }
    mech_notify(mech, MECHALL, "Lateral mode change aborted.");
    mech_event_cancel(mech, EVENT_LATERAL);
    return;
  }

  mech_printf(mech, MECHALL,
              "Wanted lateral movement mode changed to %s (%d offset).",
              lateral_mode((int)i)->full, lateral_mode((int)i)->ofs);
  mech_event_cancel(mech, EVENT_LATERAL);
  mech_event_schedule(mech, EVENT_LATERAL, mech_lateral_event, LATERAL_TICK, i);
}

void mech_turnmode(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;

  if (!mech_has_pilot(mech) || mech_pilot_dbref(mech) != player) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're not the pilot!");
    return;
  }

  if (!HasBoolAdvantage(mech_context(mech), player, "maneuvering_ace")) {
    mech_notify(mech, MECHPILOT, "You're not skilled enough to do that.");
    return;
  }

  if (buffer && !strcasecmp(buffer, "tight")) {
    mech_tight_turn_mode_set(mech, true);
    mech_notify(mech, MECHALL, "You brace for tighter turns.");
    return;
  }
  if (buffer && !strcasecmp(buffer, "normal")) {
    mech_tight_turn_mode_set(mech, false);
    mech_notify(mech, MECHALL, "You assume a normal turn mode.");
    return;
  }
  mech_printf(mech, MECHALL, "Your turning type is : %s",
              mech_condition_summary(mech).tight_turn_mode ? "TIGHT"
                                                           : "NORMAL");
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

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }
  if (CountDestroyedLegs(mech) > 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't perform a bootlegger with destroyed legs!");
    return;
  }
  if (fMechSpeed < fMinSpeed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 tprintf("You are going too slow to perform a bootlegger! The "
                         "required minimum speed is %4.1f KPH.",
                         (double)fMinSpeed));
    return;
  }

  char *turn_argument = move_argument(args, 1, 0);
  const char *turn_character = checked_storage_at_const(
      turn_argument, strlen(turn_argument) + 1, sizeof(*turn_argument), 0);
  switch (ascii_to_upper(*turn_character)) {
  case 'R':
    wHeadingChange = 90;
    break;
  case 'L':
    wHeadingChange = -90;
    break;
  }

  if (wHeadingChange == 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid turn direction!");
    return;
  }

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

  btech_channel_send(
      mech_context(mech), BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("#%ld attempts to do a bootlegger (mech). Tonnage: %d, "
              "Speed: %4.1f, BTHMod: %d",
              mech_dbref(mech), wMechTons, (double)fMechSpeed, wBTHMod));

  if (MadePilotSkillRoll(mech, wBTHMod)) {
    wNewHeading = AcceptableDegree(mech_heading_degrees(mech) + wHeadingChange);

    mech_heading_set(mech, wNewHeading);
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

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  argc = mech_parseattributes(buffer, args, 2);
  if (argc == 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }
  switch (argc) {
  case 0:
    if (!mech_targets_hex(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have invalid default target for ETA!");
      return;
    }
    eta_x = mech_target_hex_x(mech);
    eta_y = mech_target_hex_y(mech);
    break;
  case 2:
    if (!parse_int_checked(move_argument(args, 3, 0), &eta_x) ||
        !parse_int_checked(move_argument(args, 3, 1), &eta_y)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    break;
  default:
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid arguments!");
    return;
  }
  MapCoordToRealCoord(eta_x, eta_y, &fx, &fy);
  range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech), 0,
                    fx, fy, 0);
  float const current_speed = mech_current_speed(mech);
  if (fabsf(current_speed) < 0.1F)
    mech_printf(mech, MECHALL,
                "Range to hex (%d,%d) is %.1f.  ETA: Never, mech not moving.",
                eta_x, eta_y, (double)range);
  else {
    float const eta_minutes =
        fabsf(range / (current_speed / (float)KPH_PER_MP));
    etamin = clamp_float_to_int(eta_minutes);
    etahr = etamin / 60;
    etamin = etamin % 60;
    mech_printf(mech, MECHALL, "Range to hex (%d,%d) is %.1f.  ETA: %.2d:%.2d.",
                eta_x, eta_y, (double)range, etahr, etamin);
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
               (1.5F + (conditions.masc_enabled ? 0.5F : 0.0F) +
                (conditions.supercharger_enabled ? 0.5F : 0.0F) +
                (conditions.sprinting ? 0.5F : 0.0F));

    if ((mech_technology_flags(mech) & TRIPLE_MYOMER_TECH) &&
        (mech_excess_heat(mech) >= 9.0F)) {
      if (conditions.sprinting) {
        if (btech_context_uses_tsm_sprint_bonus(mech_context(mech)))
          mspeed = ceilf((rintf((mspeed / 1.5F) / MP1) + 1.0F) * 1.5F) * MP1;

      } else {
        mspeed = ceilf((rintf((mspeed / 1.5F) / MP1) + 1.0F) * 1.5F) * MP1;
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
        mspeed = mspeed * 100.0F /
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
            (mech_excess_heat(mech) >= 9.0F) &&
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
    mspeed = 0.0F;
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
      mspeed = 0.0F;
    else {
      int const tonnage = mech_tonnage(mech);
      int const denominator = mech_movement_maximum_int(
          1024 * tonnage + mech_cached_lugged_weight(mech) / 3,
          mech_movement_maximum_int(1024,
                                    mv + mech_cached_lugged_weight(mech)));
      mspeed = mech_maximum_speed(mech) * (float)tonnage * 1024.0F /
               (float)denominator;
    }
  }
  int const speed_in_movement_points = clamp_float_to_int(mspeed / MP1);
  mech_speed_cache_record(
      mech, mspeed, mech_movement_maximum_int(1, speed_in_movement_points) * 2);
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
}

float mech_effective_maximum_speed(Mech *mech) {
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
}

void mech_drop(DbRef player, void *data, const char *buffer) {
  Mech *mech = (Mech *)data;
  float s1;
  int wDropLevels = 0;
  int wDropBTH = 0;
  int tHasSwarmers = 0;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "No crawling!");
    return;
  }
  if (mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't prone in this!");
    return;
  }
  if (mech_condition_summary(mech).fallen) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are already prone.");
    return;
  }
  if (mech_is_jumping(mech) || mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't prone in the air!");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't drop while trying to stand up!");
    return;
  }

  s1 = mech_effective_maximum_speed(mech) / 3.0F;

  if ((mech_class(mech) == CLASS_MECH) && bsuit_swarmer_count(mech))
    tHasSwarmers = 1;

  float const current_speed = mech_current_speed(mech);
  if (mech_class(mech) != CLASS_MW && fabsf(current_speed) > s1 * 2.0F) {
    mech_notify(mech, MECHALL, "You attempt a controlled drop while running.");
    wDropLevels = 2;
    wDropBTH = 2;
  } else if (fabsf(current_speed) > s1) {
    mech_notify(mech, MECHALL,
                "You attempt a controlled drop from your fast walk.");
    wDropLevels = 1;
  }

  if (mech_stagger_level(mech) > 0) {
    mech_notify(mech, MECHALL,
                "Still staggering, you try not to fall on your face.");
    wDropLevels = (wDropLevels == 0 ? 1 : wDropLevels);
    wDropBTH = wDropBTH + mech_stagger_level(mech);
  }

  if (tHasSwarmers)
    mech_notify(mech, MECHALL,
                "The suits hanging off you make a controlled drop harder!");

  if ((wDropLevels > 0) || tHasSwarmers) {
    if (MadePilotSkillRoll(mech, wDropBTH)) {
      mech_notify(mech, MECHALL, "You hit the ground with minimal damage");
      mech_los_broadcast(mech, "drops to the ground!");

      if (tHasSwarmers)
        bsuit_swarmers_stop(
            btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
            mech, 0);

    } else {
      mech_notify(mech, MECHALL, "You fall to the ground hard");
      mech_los_broadcast(mech, "falls hard to the ground!");

      if (wDropLevels <= 0)
        wDropLevels = 1;

      if (tHasSwarmers)
        bsuit_swarmers_stop(
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
  mech_inferno_extinguish_in_water(mech);

  // as per ps, prone clears stagger
  if (btech_context_stagger_mode(mech_context(mech)))
    mech_stagger_damage_clear(mech);

  mine_field_trigger(mech, MINE_STEP);
}

void mech_stand(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2] = {0};
  int wcDeadLegs = 0;
  int tNeedsPSkill = 1;
  int tDoStand = 1;
  int bth, mechstandtime, standanyway = 0, standcarefulmod = 0;

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_class(mech) == CLASS_BSUIT) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're standing already!");
    return;
  }
  if (mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This vehicle cannot stand like a 'Mech.");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're standing while jumping!");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're standing while flying!");
    return;
  }

  /* set the number of dead legs we have */
  wcDeadLegs = CountDestroyedLegs(mech);

  if ((((mech_movement_type(mech) == MOVE_QUAD) && (wcDeadLegs > 3)) ||
       (!(mech_movement_type(mech) == MOVE_QUAD) && (wcDeadLegs > 1)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You have no legs to stand on!");
    return;
  }
  if (wcDeadLegs > 2) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You'd be far too unstable!");
    return;
  }
  if (mech_has_destroyed_gyro(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot stand with a destroyed gyro!");
    return;
  }

  if (!mech_condition_summary(mech).fallen) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're already standing!");
    return;
  }
  if (mech_event_count(mech, EVENT_STANDFAIL)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're still recovering from your last attempt!");
    return;
  }
  if (mech_condition_summary(mech).hull_down) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can not stand while hulldown");
    return;
  }
  if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are busy changing your hulldown mode");
    return;
  }

  bth = mech_pilot_skill_roll_target(mech, 0);

  /* Check to see if the user specified an argument for the command */
  if (proper_explodearguments(buffer, args, 2)) {
    if (strcmp(move_argument(args, 2, 0), "check") == 0) {
      notify_printf(btech_context_evaluation(mech_context(mech)), player,
                    "Your BTH to stand would be: %d", bth);
      move_arguments_destroy(args, 2);
      return;
    } else if (strcmp(move_argument(args, 2, 0), "anyway") == 0) {
      standanyway = 1;
    } else if ((strcmp(move_argument(args, 2, 0), "careful") == 0) &&
               btech_context_stand_careful_modifier(mech_context(mech))) {
      standcarefulmod = -2;
    } else {
      notify_printf(btech_context_evaluation(mech_context(mech)), player,
                    "Unknown argument! use 'stand check'%s",
                    btech_context_stand_careful_modifier(mech_context(mech))
                        ? ", 'stand careful' or 'stand anyway'"
                        : " or 'stand anyway'");
      move_arguments_destroy(args, 2);
      return;
    }
  }

  if (!standanyway && bth > 12) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "You would fail; use 'stand anyway' if you really want to stand.");
    return;
  }

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
                                          : mech_stand_time(mech));
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
                                                    : mech_stand_time(mech));
    /* Not strictly FASA, but allows legged mechs to stand careful */
    if (standcarefulmod) {
      mechstandtime = mechstandtime * 2;
    }
    mech_event_schedule(mech, EVENT_STAND, mech_stand_event, mechstandtime, 0);
  }
  /* Free args */
  move_arguments_destroy(args, 2);
}
