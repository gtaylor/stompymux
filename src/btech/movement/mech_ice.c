/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <stddef.h>

#include "btech/context.h"
#include "btech_event.h" // IWYU pragma: keep
#include "command_handlers_api.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

#define TMP_TERR '1'

static void swim_except(BattleMap *map, Mech *mech, int x, int y,
                        const char *msg, int isbridge) {
  int i;
  DbRef j;
  Mech *t;

  if (!battle_map_hex_elevation(map, x, y))
    return;
  for (i = 0; i < battle_map_unit_count(map); i++) {
    j = battle_map_unit_dbref(map, i);
    if (j < 0)
      continue;
    t = btech_context_get_mech(battle_map_context(map), j);
    if (!t || t == mech)
      continue;
    if (mech_position_x(t) != x || mech_position_y(t) != y)
      continue;
    mech_position_terrain_set(t, BATTLE_TERRAIN_WATER);
    if ((!isbridge && (mech_position_z(t) == 0) &&
         (mech_movement_type(t) != MOVE_HOVER)) ||
        (isbridge &&
         mech_position_z(t) == mech_position_elevation_magnitude(t))) {
      mech_los_broadcast(t, msg);
      mech_fall(t, mech_position_elevation_magnitude(t) + isbridge, 0);
      if (mech_class(t) == CLASS_VEH_GROUND && !mech_is_destroyed(t)) {
        mech_notify(t, MECHALL, "Water renders your vehicle inoperable.");
        mech_los_broadcast(t,
                           "fizzles and pops as water renders it inoperable.");
        mech_destroy(t, t, 0, KILL_TYPE_FLOOD);
      }
    }
  }
}

static void break_sub(BattleMap *map, Mech *mech, int x, int y,
                      const char *msg) {
  int isbridge = map_real_terrain_get(map, x, y) == BATTLE_TERRAIN_BRIDGE;

  map_terrain_set(map, x, y, BATTLE_TERRAIN_WATER);
  if (isbridge)
    map_elevation_set(map, x, y, 1);
  swim_except(map, mech, x, y, msg, isbridge);
}

/* Up -> down */
void drop_thru_ice(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  mech_notify(mech, MECHALL, "You break the ice!");
  mech_los_broadcast(mech, "breaks the ice!");
  if (mech_movement_type(mech) != MOVE_FOIL) {
    if (mech_position_elevation_magnitude(mech) > 0)
      mech_los_broadcast(mech, "vanishes into the waters!");
  }
  break_sub(map, mech, mech_position_x(mech), mech_position_y(mech),
            "goes swimming!");
  mech_position_terrain_set(mech, BATTLE_TERRAIN_WATER);
  if (mech_movement_type(mech) != MOVE_FOIL) {
    if (mech_position_elevation_magnitude(mech) > 0)
      mech_fall(mech, mech_position_elevation_magnitude(mech), 0);
  }
  if (mech_position_elevation_magnitude(mech) > 0 &&
      mech_class(mech) == CLASS_VEH_GROUND && !mech_is_destroyed(mech) &&
      !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {
    mech_notify(mech, MECHALL, "Water renders your vehicle inoperable.");
    mech_los_broadcast(mech,
                       "fizzles and pops as water renders it inoperable.");
    mech_destroy(mech, mech, 0, KILL_TYPE_ICE);
  }
}

/* Down -> up */
void break_thru_ice(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  MarkForLOSUpdate(mech);
  mech_notify(mech, MECHALL, "You break through the ice!");
  mech_los_broadcast(mech, "breaks through the ice!");
  break_sub(map, mech, mech_position_x(mech), mech_position_y(mech),
            "goes swimming!");
  mech_position_terrain_set(mech, BATTLE_TERRAIN_WATER);
}

/* CHANCE of dropping thru the ice based on 'mech weight */
int possibly_drop_thru_ice(Mech *mech) {
  if ((mech_movement_type(mech) == MOVE_HOVER) ||
      (mech_movement_type(mech) == MOVE_SUB) ||
      (mech_class(mech) == CLASS_BSUIT))
    return 0;
  if (btech_random_range(mech_context(mech), 1, 6) != 1)
    return 0;
  drop_thru_ice(mech);
  return 1;
}

static void growable_callback(BattleMap *map, int x, int y, void *context) {
  int *water_count = context;
  char terrain = map_real_terrain_get(map, x, y);

  if ((battle_terrain_is_water(terrain) && terrain != BATTLE_TERRAIN_ICE) ||
      map_real_terrain_get(map, x, y) == TMP_TERR)
    (*water_count)++;
}

int growable(BattleMap *map, int x, int y) {
  int water_count = 0;

  visit_neighbor_hexes(map, x, y, growable_callback, &water_count);

  if (water_count <= 4 &&
      (water_count < 2 ||
       (btech_random_range(battle_map_context(map), 1, 6) > water_count)))
    return 1;
  return 0;
}

static void meltable_callback(BattleMap *map, int x, int y, void *context) {
  int *water_count = context;

  if (map_real_terrain_get(map, x, y) == BATTLE_TERRAIN_ICE)
    (*water_count)++;
}

int meltable(BattleMap *map, int x, int y) {
  int water_count = 0;

  visit_neighbor_hexes(map, x, y, meltable_callback, &water_count);

  if (water_count > 4 && btech_random_range(battle_map_context(map), 1, 3) > 1)
    return 0;
  return 1;
}

void ice_growth(DbRef player, BattleMap *map, int num) {
  int x, y;
  int count = 0;

  for (x = 0; x < battle_map_width(map); x++)
    for (y = 0; y < battle_map_height(map); y++)
      if (map_real_terrain_get(map, x, y) == BATTLE_TERRAIN_WATER)
        if (btech_random_range(battle_map_context(map), 1, 100) <= num &&
            growable(map, x, y)) {
          map_terrain_set(map, x, y, TMP_TERR);
          count++;
        }
  for (x = 0; x < battle_map_width(map); x++)
    for (y = 0; y < battle_map_height(map); y++)
      if (map_real_terrain_get(map, x, y) == TMP_TERR)
        map_terrain_set(map, x, y, BATTLE_TERRAIN_ICE);
  if (count)
    notify_printf(btech_context_evaluation(battle_map_context(map)), player,
                  "%d hexes 'iced'.", count);
  else
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "No hexes 'iced'.");
}

void ice_melt(DbRef player, BattleMap *map, int num) {
  int x, y;
  int count = 0;

  for (x = 0; x < battle_map_width(map); x++)
    for (y = 0; y < battle_map_height(map); y++)
      if (map_real_terrain_get(map, x, y) == BATTLE_TERRAIN_ICE)
        if (btech_random_range(battle_map_context(map), 1, 100) <= num &&
            meltable(map, x, y)) {
          break_sub(map, nullptr, x, y, "goes swimming as ice breaks!");
          map_terrain_set(map, x, y, TMP_TERR);
          count++;
        }
  for (x = 0; x < battle_map_width(map); x++)
    for (y = 0; y < battle_map_height(map); y++)
      if (map_real_terrain_get(map, x, y) == TMP_TERR)
        map_terrain_set(map, x, y, BATTLE_TERRAIN_WATER);
  if (count)
    notify_printf(btech_context_evaluation(battle_map_context(map)), player,
                  "%d hexes melted.", count);
  else
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "No hexes melted.");
}

void map_addice(DbRef player, BattleMap *map, char *buffer) {
  char *args[2];
  int num;

  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid arguments!");
    return;
  }
  if ((!((num) = atoi(args[0])) && strcmp((args[0]), "0"))) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }
  ice_growth(player, map, num);
}

void map_delice(DbRef player, BattleMap *map, char *buffer) {
  char *args[2];
  int num;

  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid arguments!");
    return;
  }
  if ((!((num) = atoi(args[0])) && strcmp((args[0]), "0"))) {
    mecha_notify(btech_context_evaluation(battle_map_context(map)), player,
                 "Invalid number!");
    return;
  }
  ice_melt(player, map, num);
}

void possibly_blow_ice(Mech *mech, int weapindx, int x, int y) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (map_real_terrain_get(map, x, y) != BATTLE_TERRAIN_ICE)
    return;
  if (btech_random_range(mech_context(mech), 1, 15) >
      weapon_catalogue_damage(weapindx))
    return;
  HexLOSBroadcast(map, x, y, "The ice breaks from the blast!");
  break_sub(map, nullptr, x, y, "goes swimming as ice breaks!");
}

void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (map_real_terrain_get(map, x, y) != BATTLE_TERRAIN_BRIDGE)
    return;
  if (battle_map_bridges_have_capacity(map))
    return;
  if (btech_random_range(mech_context(mech), 1,
                         10 * (1 + map_elevation_get(map, x, y))) >
      weapon_catalogue_damage(weapindx)) {
    HexLOSBroadcast(map, x, y, "The bridge at $H shudders from direct hit!");
    return;
  }
  HexLOSBroadcast(map, x, y, "The bridge at $H is blown apart!");
  break_sub(map, nullptr, x, y, "goes swimming as the bridge is blown apart!");
}
