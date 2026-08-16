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
#include "map_coordinates.h"
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
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

static int mech_movement_maximum_int(int first, int second) {
  return first > second ? first : second;
}

static int mech_stand_time(const Mech *mech) {
  const float SPEED_FACTOR = mech_maximum_speed(mech) / MP2;
  const float BOUNDED_FACTOR = fminf(fmaxf(1.0F, SPEED_FACTOR), 30.0F);
  const float DELAY = 30.0F / BOUNDED_FACTOR;
  return (int)DELAY;
}

typedef struct LateralMode {
  const char *name;
  const char *full;
  int ofs;
} LateralMode;

static const LateralMode LATERAL_MODES[] = {
    {"nw", "Front/Left", 300}, {"fl", "Front/Left", 300},
    {"ne", "Front/Right", 60}, {"fr", "Front/Right", 60},
    {"sw", "Rear/Left", 240},  {"rl", "Rear/Left", 240},
    {"se", "Rear/Right", 120}, {"rr", "Rear/Right", 120},
    {"-", "None", 0},          {nullptr, nullptr, 0}};

static const LateralMode *lateral_mode(int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(LATERAL_MODES, 10, sizeof(*LATERAL_MODES),
                                  (size_t)index);
}

static char *move_argument(char **arguments, size_t count, int index) {
  if (index < 0)
    abort();
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), (size_t)index);
  return *slot;
}

static void move_arguments_destroy(char **arguments, size_t count) {
  for (size_t index = 0; index < count; index++) {
    char **slot = (char **)checked_storage_at((void *)arguments, count,
                                              sizeof(*arguments), index);
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

void mech_lateral(DbRef player, Mech *mech, const char *buffer) {
  /* Rule Reference: BMR Revised, Page 82 (Quad Lateral) */
  /* Rule Reference: MaxTech Revised, Page 46 (All Units w/ Maneuvering Ace) */
  /* Rule Reference: MaxTech Revised, Page 29 (VTOL/Hover Lateral) */
  /* Rule Reference: Total Warfare, Page 50 (Quad Lateral) */
  /* Rule Reference: Total Warfare, Page 67 (VTOL/Hover Lateral, though doesn't
   * say intentional) */
  long i;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!(((mech_movement_type(mech) == MOVE_QUAD) &&
         (count_destroyed_legs(mech) == 0)) ||
        ((mech_class(mech) == CLASS_VTOL) ||
         (mech_movement_type(mech) == MOVE_HOVER)) ||
        ((has_bool_advantage(mech_context(mech), player, "maneuvering_ace") &&
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

void mech_turnmode(DbRef player, Mech *mech, char *buffer) {
  if (!mech_has_pilot(mech) || mech_pilot_dbref(mech) != player) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You're not the pilot!");
    return;
  }

  if (!has_bool_advantage(mech_context(mech), player, "maneuvering_ace")) {
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
}

void mech_bootlegger(DbRef player, Mech *mech, char *buffer) {
  float f_min_speed = (4 * MP1);
  int w_bth_mod = 0;
  int w_fall_levels = 0;
  int i;
  int w_heading_change = 0;
  int w_new_heading;
  float f_mech_speed = mech_current_speed(mech);
  int w_mech_tons = mech_tonnage(mech);
  char str_location[50];
  char *args[1];

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (mech_parseattributes(buffer, args, 1) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments!");
    return;
  }
  if (count_destroyed_legs(mech) > 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't perform a bootlegger with destroyed legs!");
    return;
  }
  if (f_mech_speed < f_min_speed) {
    mecha_notifyf(btech_context_evaluation(mech_context(mech)), player,
                  "You are going too slow to perform a bootlegger! The "
                  "required minimum speed is %4.1f KPH.",
                  (double)f_min_speed);
    return;
  }

  char *turn_argument = move_argument(args, 1, 0);
  const char *turn_character = checked_storage_at_const(
      turn_argument, strlen(turn_argument) + 1, sizeof(*turn_argument), 0);
  switch (ascii_to_upper(*turn_character)) {
  case 'R':
    w_heading_change = 90;
    break;
  case 'L':
    w_heading_change = -90;
    break;
  }

  if (w_heading_change == 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid turn direction!");
    return;
  }

  for (i = 0; i < NUM_SECTIONS; i++) {
    if ((i == LLEG) || (i == RLEG) ||
        ((mech_movement_type(mech) == MOVE_QUAD) &&
         ((i == LARM) || (i == RARM)))) {
      armor_string_from_index(i, str_location, mech_class(mech),
                              mech_movement_type(mech));

      if (sect_has_busy_weap(mech, i)) {
        mech_printf(mech, MECHALL, "You have weapons recycling in your %s.",
                    str_location);
        return;
      }

      if (mech_section_recycle_ticks(mech, i)) {
        mech_printf(mech, MECHALL,
                    "Your %s is still recovering from its last action.",
                    str_location);
        return;
      }

      w_bth_mod += mech_section_base_to_hit(mech, i);
    }
  }

  if (f_mech_speed <= (4 * MP1)) {
    w_bth_mod += 0;
  } else if (f_mech_speed <= (8 * MP1)) {
    w_bth_mod += 1;
  } else if (f_mech_speed <= (12 * MP1)) {
    w_bth_mod += 2;
  } else {
    w_bth_mod += 3;
  }

  if (w_mech_tons <= 35) {
    w_bth_mod += 0;
  } else if (w_mech_tons <= 55) {
    w_bth_mod += 1;
  } else if (w_mech_tons <= 75) {
    w_bth_mod += 2;
  } else {
    w_bth_mod += 3;
  }

  w_bth_mod += (battle_terrain_is_water(mech_real_terrain_get(mech)) &&
                        mech_position_z(mech) < 0
                    ? 2
                    : 0);

  w_bth_mod = mech_movement_maximum_int(w_bth_mod, 1);

  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_DEBUG,
                     "#%ld attempts to do a bootlegger (mech). Tonnage: %d, "
                     "Speed: %4.1f, BTHMod: %d",
                     mech_dbref(mech), w_mech_tons, (double)f_mech_speed,
                     w_bth_mod);

  if (made_pilot_skill_roll(mech, w_bth_mod)) {
    w_new_heading =
        acceptable_degree(mech_heading_degrees(mech) + w_heading_change);

    mech_heading_set(mech, w_new_heading);
    mech_desired_heading_set(mech, w_new_heading);
    mech_current_speed_scale(mech, 0.5F);

    mech_printf(mech, MECHALL,
                "You plant a foot and swivel, changing your heading to %d.",
                w_new_heading);

    for (i = 0; i < NUM_SECTIONS; i++) {
      if ((i == LLEG) || (i == RLEG) ||
          ((mech_movement_type(mech) == MOVE_QUAD) &&
           ((i == LARM) || (i == RARM))))
        mech_set_recycle_limb(mech, i, 30);
    }

  } else {
    w_fall_levels = mech_movement_maximum_int(w_bth_mod, 1);

    mech_notify(mech, MECHALL, "You plant a foot and try to swivel...");
    mech_notify(
        mech, MECHALL,
        "... but realize a little late that this is harder than it looks!");
    mech_los_broadcast(mech,
                       "attempts to fight the forces of inertia but looses "
                       "the battle miserably!");

    if (w_fall_levels > 2)
      mech_los_broadcast(mech, "tumbles over and over and over!");

    mech_fall(mech, w_fall_levels, true);
  }
}

void mech_eta(DbRef player, Mech *mech, char *buffer) {
  int argc;
  int eta_x;
  int eta_y;
  float fx;
  float fy;
  float range;
  int etahr;
  int etamin;
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
  map_coord_to_real_coord(eta_x, eta_y, &fx, &fy);
  range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech),
                .z = 0.0F},
      .end = {.x = fx, .y = fy, .z = 0.0F},
  });
  float const CURRENT_SPEED = mech_current_speed(mech);
  if (fabsf(CURRENT_SPEED) < 0.1F) {
    mech_printf(mech, MECHALL,
                "Range to hex (%d,%d) is %.1f.  ETA: Never, mech not moving.",
                eta_x, eta_y, (double)range);
  } else {
    float const ETA_MINUTES =
        fabsf(range / (CURRENT_SPEED / (float)KPH_PER_MP));
    etamin = clamp_float_to_int(ETA_MINUTES);
    etahr = etamin / 60;
    etamin = etamin % 60;
    mech_printf(mech, MECHALL, "Range to hex (%d,%d) is %.1f.  ETA: %.2d:%.2d.",
                eta_x, eta_y, (double)range, etahr, etamin);
  }
}

float mech_cargo_maximum_speed(Mech *mech, float mspeed) {
  int lugged = 0;
  int mod = 2;
  Mech *c;
  BattleMap *map;
  MechConditionSummary conditions = mech_condition_summary(mech);

  if (mech_carried_dbref(mech) > 0) { /* Ug-lee! */
    Mech *t;

    t = btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
    if (t)
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
        has_bool_advantage(mech_context(mech), mech_pilot_dbref(mech),
                           "speed_demon"))
      mspeed += MP1;

    if (mech_is_under_special_conditions(mech) && mech_is_under_gravity(mech)) {
      map = btech_context_find_object(mech_context(mech), mech_map_dbref(mech));
      if (map)
        mspeed = mspeed * 100.0F /
                 (float)(battle_map_gravity(map) > 50 ? battle_map_gravity(map)
                                                      : 50);
    }

    return mspeed;
  }
  mech_cached_calculated_weight_set(mech, mech_calculated_weight(mech));

  /*! \todo {Check some of this math better} */
  if (!mech_load_cache_is_valid(mech)) {
    if (mech_carried_dbref(mech) > 0) {
      c = btech_context_get_mech(mech_context(mech), mech_carried_dbref(mech));
      if (c) {
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
    }

    if (mech_technology_flags(mech) & CARGO_TECH)
      mod = 1;

    if (mech_class(mech) == CLASS_MECH)
      mod = mod * 2;

    lugged += mech_carried_cargo_weight(mech) * mod / 2;
    mech_load_cache_record(mech, lugged);
  }
  if (mech_is_destroyed(mech)) {
    mspeed = 0.0F;
  } else {
    int mv = mech_cached_calculated_weight(mech);
    int sv = mech_tonnage(mech) * 1024;

    if (mv == 1 && !mech_is_destroyed(mech)) {
      mv = sv;
    } else {
      if (mv > sv)
        mv = mv + ((mv - sv) / 2);
      else
        mv = mv + ((sv - mv) / 3);
    }
    if (3 * sv < (mech_cached_lugged_weight(mech) + mv)) {
      mspeed = 0.0F;
    } else {
      int const TONNAGE = mech_tonnage(mech);
      int const DENOMINATOR = mech_movement_maximum_int(
          (1024 * TONNAGE) + (mech_cached_lugged_weight(mech) / 3),
          mech_movement_maximum_int(1024,
                                    mv + mech_cached_lugged_weight(mech)));
      mspeed = mech_maximum_speed(mech) * (float)TONNAGE * 1024.0F /
               (float)DENOMINATOR;
    }
  }
  int const SPEED_IN_MOVEMENT_POINTS = clamp_float_to_int(mspeed / MP1);
  mech_speed_cache_record(&(MechSpeedCacheRecord){
      .mech = mech,
      .maximum_speed = mspeed,
      .walking_xp_factor =
          mech_movement_maximum_int(1, SPEED_IN_MOVEMENT_POINTS) * 2});
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
}

float mech_effective_maximum_speed(Mech *mech) {
  return mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
}

void mech_drop(DbRef player, Mech *mech, const char *buffer [[maybe_unused]]) {
  float s1;
  int w_drop_levels = 0;
  int w_drop_bth = 0;
  int t_has_swarmers = 0;

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
    t_has_swarmers = 1;

  float const CURRENT_SPEED = mech_current_speed(mech);
  if (mech_class(mech) != CLASS_MW && fabsf(CURRENT_SPEED) > s1 * 2.0F) {
    mech_notify(mech, MECHALL, "You attempt a controlled drop while running.");
    w_drop_levels = 2;
    w_drop_bth = 2;
  } else if (fabsf(CURRENT_SPEED) > s1) {
    mech_notify(mech, MECHALL,
                "You attempt a controlled drop from your fast walk.");
    w_drop_levels = 1;
  }

  if (mech_stagger_level(mech) > 0) {
    mech_notify(mech, MECHALL,
                "Still staggering, you try not to fall on your face.");
    w_drop_levels = (w_drop_levels == 0 ? 1 : w_drop_levels);
    w_drop_bth = w_drop_bth + mech_stagger_level(mech);
  }

  if (t_has_swarmers)
    mech_notify(mech, MECHALL,
                "The suits hanging off you make a controlled drop harder!");

  if ((w_drop_levels > 0) || t_has_swarmers) {
    if (made_pilot_skill_roll(mech, w_drop_bth)) {
      mech_notify(mech, MECHALL, "You hit the ground with minimal damage");
      mech_los_broadcast(mech, "drops to the ground!");

      if (t_has_swarmers)
        bsuit_swarmers_stop(
            btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
            mech, 0);

    } else {
      mech_notify(mech, MECHALL, "You fall to the ground hard");
      mech_los_broadcast(mech, "falls hard to the ground!");

      if (w_drop_levels <= 0)
        w_drop_levels = 1;

      if (t_has_swarmers)
        bsuit_swarmers_stop(
            btech_context_find_object(mech_context(mech), mech_map_dbref(mech)),
            mech, 0);

      mech_fall(mech, w_drop_levels, true);
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

void mech_stand(DbRef player, Mech *mech, char *buffer) {
  char *args[2] = {};
  int wc_dead_legs = 0;
  int t_needs_p_skill = 1;
  int t_do_stand = 1;
  int bth;
  int mechstandtime;
  int standanyway = 0;
  int standcarefulmod = 0;

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
  wc_dead_legs = count_destroyed_legs(mech);

  if ((((mech_movement_type(mech) == MOVE_QUAD) && (wc_dead_legs > 3)) ||
       (!(mech_movement_type(mech) == MOVE_QUAD) && (wc_dead_legs > 1)))) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You have no legs to stand on!");
    return;
  }
  if (wc_dead_legs > 2) {
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
    }
    if (strcmp(move_argument(args, 2, 0), "anyway") == 0) {
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
  if (((wc_dead_legs == 0) && (mech_movement_type(mech) == MOVE_QUAD)) ||
      (mech_class(mech) == CLASS_MW)) {
    t_needs_p_skill = 0;
  }

  mech_los_broadcast(mech, "attempts to stand up.");

  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
      mech_position_z(mech) == -1)
    break_thru_ice(mech);

  if (t_needs_p_skill) {
    /* Changed to NoXP. Keeps people from doing pushups to gain Pilot XP */
    if (!mech_pilot_skill_roll_without_experience(&(PilotSkillRollRequest){
            .mech = mech, .modifier = standcarefulmod})) {
      mech_notify(mech, MECHALL,
                  "You fail your attempt to stand and fall back on the ground");
      mech_fall(mech, 1, true);
      mechstandtime =
          ((mech_class(mech) == CLASS_MW) ? DROP_TO_STAND_RECYCLE / 3
                                          : mech_stand_time(mech));
      /* Not strictly FASA, but allows legged mechs to stand careful */
      if (standcarefulmod) {
        mechstandtime = mech_movement_maximum_int(30, mechstandtime * 2);
      }
      mech_event_schedule(mech, EVENT_STANDFAIL, mech_standfail_event,
                          mechstandtime, 0);
      t_do_stand = 0;
    }
  }

  if (t_do_stand) {
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
