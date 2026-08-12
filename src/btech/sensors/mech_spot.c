/* Implements BattleTech sensor mechanics for unit spot. */

#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_spot_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

typedef struct SpotLinkEventData {
  Mech *target;
  float observer_x;
  float observer_y;
  float target_x;
  float target_y;
} SpotLinkEventData;

static bool positions_differ(float first, float second) {
  return fabsf(first - second) > 0.0001F;
}

static float scaled_hex_elevation(int elevation) {
  return ZSCALE * (float)elevation;
}

static bool mech_is_in_water(Mech *mech) {
  const char TERRAIN = mech_real_terrain_get(mech);
  return (TERRAIN == BATTLE_TERRAIN_ICE || TERRAIN == BATTLE_TERRAIN_WATER ||
          TERRAIN == BATTLE_TERRAIN_BRIDGE) &&
         mech_position_z(mech) < 0;
}

bool mech_spot_has_artillery(Mech *mech) {
  int weapnum, weaptype = -2;

  for (weapnum = 0; weaptype != -1; weapnum++) {
    WeaponNumberLookupResult lookup = weapon_number_find(
        &(WeaponNumberLookupRequest){.mech = mech, .number = weapnum});
    weaptype = lookup.value;
    if (weapon_catalogue_is_artillery(weaptype))
      return 1;
  }
  return 0;
}

static void mech_check_range(MuxEvent *e) {
  Mech *spotter = (Mech *)e->data2, *mech = (Mech *)e->data;
  float range;

  if (!mech)
    return;

  if (mech_spotter_dbref(mech) == -1)
    return;

  if (!spotter) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  range = mech_range_to(mech, spotter);
  const int MAXIMUM_RANGE = 2 * mech_radio_range(spotter);
  if (range > (float)MAXIMUM_RANGE || mech_spotter_dbref(spotter) == -1 ||
      mech_map_dbref(spotter) != mech_map_dbref(mech)) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)spotter);
}

static void mech_spot_event(MuxEvent *e) {
  Mech *target, *mech = (Mech *)e->data;
  SpotLinkEventData *sd = (SpotLinkEventData *)e->data2;

  target = sd->target;

  if (positions_differ(mech_position_real_x(mech), sd->observer_x) &&
      positions_differ(mech_position_real_y(mech), sd->observer_y) &&
      positions_differ(mech_position_real_x(target), sd->target_x) &&
      positions_differ(mech_position_real_y(target), sd->target_y)) {
    mech_notify(target, MECHALL,
                "The data link was not established due to movement!");
    mech_notify(mech, MECHALL,
                "The data link was not established due to movement!");
    free(e->data2);
    return;
  }
  mech_printf(target, MECHALL, "Data link established with %s.",
              mech_to_mech_display_id(target, mech).text);
  mech_printf(mech, MECHALL,
              "Data link established with %s, you now have a forward observer.",
              mech_to_mech_display_id(target, mech).text);
  mech_spotter_dbref_set(mech, mech_dbref(target));
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)target);
  free(e->data2);
}

void mech_spot_clear_fire_adjustments(BattleMap *map, DbRef mech) {
  int i;
  Mech *m;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if (battle_map_unit_dbref(map, i) >= 0) {
      m = btech_context_get_mech(battle_map_context(map),
                                 battle_map_unit_dbref(map, i));
      if (!m)
        continue;
      if (mech_dbref(m) == mech)
        continue;
      if (mech_spotter_dbref(m) == mech)
        mech_fire_adjustment_set(m, 0);
    }
}

void mech_spot(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[5];
  char target_id[3];
  int argc;
  int los = 1;
  DbRef targetref;
  float range;
  SpotLinkEventData *dat;
  BattleMap *mech_map;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  argc = mech_parseattributes(buffer, args, 5);
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot spot while using a special movement mode.");
    return;
  }
#endif
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You may only use mech ID's to set spotter!");
    return;
  }
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Spot ? You ? What with, your pretty blue eyes ? Hah!");
    return;
  }
  target_id[0] = args[0][0];
  target_id[1] = *checked_string_suffix(*args, 1);
  target_id[2] = 0;
  targetref = find_target_dbref_from_map_number(mech, target_id);
  if (!strcmp(args[0], "-")) {
    if (mech_spotter_dbref(mech) == mech_dbref(mech)) {
      mech_notify(mech, MECHALL, "You spot no longer.");
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    } else {
      mech_notify(mech, MECHALL, "You disable the datalink to spotter.");
    }
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  if (!strcasecmp(target_id, mech_id(mech, false).text)) {
    if (mech_recycling_state(mech, CHECK_BOTH)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have weapons recycling!");
      return;
    }
    mech_spotter_dbref_set(mech, mech_dbref(mech));
    mech_notify(mech, MECHALL, "You are now set as a spotter.");
    return;
  }
  target = btech_context_get_mech(mech_context(mech), targetref);
  if (target)
    los = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), mech_range_to(mech, target));
  if (!target || (targetref == -1) || mech_team(target) != mech_team(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target does not exist!");
    return;
  }

  if (mech_class(target) == CLASS_MW) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Spot ? That puny being ?! What with, those clear brown eyes ? Hah!");
    return;
  }
  if (mech_spotter_dbref(target) != mech_dbref(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That 'mech is not set up as spotter!");
    return;
  }

  if (mech_spot_has_artillery(mech) && !los) {
    mech_notify(target, MECHALL,
                "Someone is trying to establish a data link with you!");
    mech_notify(mech, MECHALL,
                "You attempt to establish a data link..... please stand by.");
    range = mech_range_to(mech, target);
    const int MAXIMUM_RANGE = 2 * mech_radio_range(target);
    if (range > (float)MAXIMUM_RANGE) {
      mech_notify(mech, MECHALL, "That target is our of data link range!");
      return;
    }
    dat = checked_storage_allocate(sizeof(*dat));
    dat->observer_y = mech_position_real_y(mech);
    dat->observer_x = mech_position_real_x(mech);
    dat->target_x = mech_position_real_x(target);
    dat->target_y = mech_position_real_y(target);
    dat->target = target;
    // NOLINTNEXTLINE(clang-analyzer-unix.Malloc)
    mech_event_schedule(mech, EVENT_SPOT_LOCK, mech_spot_event,
                        WEAPON_TICK * ((int)range / 10 + 5), (intptr_t)dat);
    return;
  }
  if (!los) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You do not have LOS to that target!");
    return;
  }
  mech_spotter_dbref_set(mech, targetref);
  mech_fire_adjustment_set(mech, 0);
  mech_printf(mech, MECHALL, "%s set as spotter.",
              mech_to_mech_display_id(mech, target).text);
}

int mech_spot_fire(DbRef player, Mech *mech, BattleMap *mech_map, int weaponnum,
                   int weapontype, int sight, int section, int critical) {
  /* Nim 9/11/96 */

  float spot_range, range;
  float enemy_x, enemy_y, enemy_z = 0;
  int los, mapx = 0, mapy = 0;
  Mech *target = nullptr, *spotter;
  int spot_terrain;
  bool found_target = false;

  /* No spotter or not IDF weapon lets get outta here */
  if (mech_spotter_dbref(mech) == -1 ||
      !weapon_catalogue_supports_indirect_fire(weapontype))
    return 0;

  spotter =
      btech_context_get_mech(mech_context(mech), mech_spotter_dbref(mech));
  if (!spotter) {
    mech_notify(mech, MECHPILOT, "There is no spotter avilable to IDF with!");
    return 1;
  }

  if (mech_spotter_dbref(spotter) != mech_dbref(spotter)) {
    mech_notify(mech, MECH_PILOT, "You do not have a spotter!");
    mech_spotter_dbref_set(mech, -1);
    return 1;
  }
  if (mech_pilot_is_unconscious(spotter)) {
    mech_notify(mech, MECHPILOT, "Your spotter is unconscious!");
    return 1;
  }
  if (mech_is_blinded(spotter)) {
    mech_notify(mech, MECHPILOT, "Your spotter can't see a thing!");
    return 1;
  }

  /* Is the spotter set to a Mech or to a Hex? */
  if (mech_target_dbref(spotter) != -1) {
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(spotter));
    if (!target) {
      mech_notify(mech, MECHPILOT, "Your spotter has invalid target!");
      return 1;
    }
    mapx = mech_position_x(target);
    mapy = mech_position_y(target);
    spot_range = mech_range_to(spotter, target);
    los = mech_los_check(spotter, target, mapx, mapy, spot_range);
    if (!los) {
      mech_notify(mech, MECHPILOT,
                  "You spotter does not have a target in LOS!");
      return 1;
    }
    range = mech_range_to(mech, target);
    if (mech_is_in_water(target) && !mech_is_in_water(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You can't fire into water with that weapon from here.");
      return 0;
    }

    spot_terrain =
        weapon_catalogue_is_artillery(weapontype)
            ? 2
            : (1 +
               mech_los_terrain_modifier(&(MechLosTerrainRequest){
                   .observer = spotter,
                   .target = target,
                   .map = mech_map,
               }) +
               mech_attacker_movement_modifier(spotter) +
               ((mech_event_count(spotter, EVENT_LOCK) &&
                 mech_targeting_computer_type(spotter) != TARGCOMP_MULTI)
                    ? 2
                    : 0));
    if (weapon_catalogue_is_artillery(weapontype) && target) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You can only target hexes with this kind of artillery.");
      return -1;
    }
    if (!sight) {
      accumulate_spot_xp(mech_pilot_dbref(spotter), spotter, target);
      accumulate_arty_xp(mech_pilot_dbref(mech), mech, target);
    }
    mech_weapon_fire(&(WeaponFireRequest){
        .mech = mech,
        .map = mech_map,
        .target = target,
        .weapon_index = weapontype,
        .weapon_number = weaponnum,
        .weapon = {.section = section, .critical = critical},
        .target_hex = {.x = mapx, .y = mapy},
        .range = range,
        .indirect_fire = spot_terrain,
        .sight = sight != 0,
        .target_kind = 2});
    return 1;
  }
  if (!(mech_target_hex_x(spotter) >= 0 && mech_target_hex_y(spotter) >= 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your spotter has no target set!");
    return 1;
  }
  if (!weapon_catalogue_is_artillery(weapontype)) {
    target = find_mech_in_hex(mech, mech_map, mech_target_hex_x(spotter),
                              mech_target_hex_y(spotter), 0);
    if (target) {
      enemy_x = mech_position_real_x(target);
      enemy_y = mech_position_real_y(target);
      enemy_z = mech_position_real_z(target);
      mapx = mech_position_x(target);
      mapy = mech_position_y(target);
      found_target = true;
    }
  }
  if (!found_target) {
    target = nullptr;
    mapx = mech_target_hex_x(spotter);
    mapy = mech_target_hex_y(spotter);
    const int TARGET_HEX_Z = mech_target_hex_z(spotter);
    enemy_z = scaled_hex_elevation(TARGET_HEX_Z);
    map_coord_to_real_coord(mapx, mapy, &enemy_x, &enemy_y);
  }
  spot_range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = mech_position_real_x(spotter),
                .y = mech_position_real_y(spotter),
                .z = mech_position_real_z(spotter)},
      .end = {.x = enemy_x, .y = enemy_y, .z = enemy_z},
  });
  los = mech_los_check(spotter, target, mapx, mapy, spot_range);
  if (!los) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target is not in your spotters line of sight!");
    return 0;
  }
  range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech),
                .z = mech_position_real_z(mech)},
      .end = {.x = enemy_x, .y = enemy_y, .z = enemy_z},
  });
  spot_terrain =
      weapon_catalogue_is_artillery(weapontype)
          ? 2
          : (1 + mech_attacker_movement_modifier(spotter) +
             ((mech_event_count(spotter, EVENT_LOCK) &&
               mech_targeting_computer_type(spotter) != TARGCOMP_MULTI)
                  ? 2
                  : 0));
  mech_weapon_fire(
      &(WeaponFireRequest){.mech = mech,
                           .map = mech_map,
                           .target = target,
                           .weapon_index = weapontype,
                           .weapon_number = weaponnum,
                           .weapon = {.section = section, .critical = critical},
                           .target_hex = {.x = mapx, .y = mapy},
                           .range = range,
                           .indirect_fire = spot_terrain,
                           .sight = sight != 0,
                           .target_kind = 2});
  return 1;
}
