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
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "registry_api.h"

#define TMP_TERR '1'

static void swim_except(BattleMap *map, Mech *mech, int x, int y, char *msg,
                        int isbridge) {
  int i, j;
  Mech *t;

  if (!(Elevation(map, x, y)))
    return;
  for (i = 0; i < map->first_free; i++) {
    j = map->mechsOnMap[i];
    if (j < 0)
      continue;
    t = btech_context_get_mech(map->xcode.context, j);
    if (!t || t == mech)
      continue;
    if (MechX(t) != x || MechY(t) != y)
      continue;
    MechTerrain(t) = WATER;
    if ((!isbridge && (MechZ(t) == 0) && (MechMove(t) != MOVE_HOVER)) ||
        (isbridge && MechZ(t) == MechElev(t))) {
      MechLOSBroadcast(t, msg);
      MechFalls(t, MechElev(t) + isbridge, 0);
      if (MechType(t) == CLASS_VEH_GROUND && !Destroyed(t)) {
        mech_notify(t, MECHALL, "Water renders your vehicle inoperable.");
        MechLOSBroadcast(t, "fizzles and pops as water renders it inoperable.");
        DestroyMech(t, t, 0, KILL_TYPE_FLOOD);
      }
    }
  }
}

static void break_sub(BattleMap *map, Mech *mech, int x, int y, char *msg) {
  int isbridge = map_real_terrain_get(map, x, y) == BRIDGE;

  map_terrain_set(map, x, y, WATER);
  if (isbridge)
    map_elevation_set(map, x, y, 1);
  swim_except(map, mech, x, y, msg, isbridge);
}

/* Up -> down */
void drop_thru_ice(Mech *mech) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);

  mech_notify(mech, MECHALL, "You break the ice!");
  MechLOSBroadcast(mech, "breaks the ice!");
  if (MechMove(mech) != MOVE_FOIL) {
    if (MechElev(mech) > 0)
      MechLOSBroadcast(mech, "vanishes into the waters!");
  }
  break_sub(map, mech, MechX(mech), MechY(mech), "goes swimming!");
  MechTerrain(mech) = WATER;
  if (MechMove(mech) != MOVE_FOIL) {
    if (MechElev(mech) > 0)
      MechFalls(mech, MechElev(mech), 0);
  }
  if (MechElev(mech) > 0 && MechType(mech) == CLASS_VEH_GROUND &&
      !Destroyed(mech) && !(MechSpecials2(mech) & WATERPROOF_TECH)) {
    mech_notify(mech, MECHALL, "Water renders your vehicle inoperable.");
    MechLOSBroadcast(mech, "fizzles and pops as water renders it inoperable.");
    DestroyMech(mech, mech, 0, KILL_TYPE_ICE);
  }
}

/* Down -> up */
void break_thru_ice(Mech *mech) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);

  MarkForLOSUpdate(mech);
  mech_notify(mech, MECHALL, "You break through the ice!");
  MechLOSBroadcast(mech, "breaks through the ice!");
  break_sub(map, mech, MechX(mech), MechY(mech), "goes swimming!");
  MechTerrain(mech) = WATER;
}

/* CHANCE of dropping thru the ice based on 'mech weight */
int possibly_drop_thru_ice(Mech *mech) {
  if ((MechMove(mech) == MOVE_HOVER) || (MechMove(mech) == MOVE_SUB) ||
      (MechType(mech) == CLASS_BSUIT))
    return 0;
  if (btech_random_range(mech->xcode.context, 1, 6) != 1)
    return 0;
  drop_thru_ice(mech);
  return 1;
}

static void growable_callback(BattleMap *map, int x, int y, void *context) {
  int *water_count = context;
  int terrain = map_real_terrain_get(map, x, y);

  if ((IsWater(terrain) && terrain != ICE) ||
      map_real_terrain_get(map, x, y) == TMP_TERR)
    (*water_count)++;
}

int growable(BattleMap *map, int x, int y) {
  int water_count = 0;

  visit_neighbor_hexes(map, x, y, growable_callback, &water_count);

  if (water_count <= 4 &&
      (water_count < 2 ||
       (btech_random_range(map->xcode.context, 1, 6) > water_count)))
    return 1;
  return 0;
}

static void meltable_callback(BattleMap *map, int x, int y, void *context) {
  int *water_count = context;

  if (map_real_terrain_get(map, x, y) == ICE)
    (*water_count)++;
}

int meltable(BattleMap *map, int x, int y) {
  int water_count = 0;

  visit_neighbor_hexes(map, x, y, meltable_callback, &water_count);

  if (water_count > 4 && btech_random_range(map->xcode.context, 1, 3) > 1)
    return 0;
  return 1;
}

void ice_growth(DbRef player, BattleMap *map, int num) {
  int x, y;
  int count = 0;

  for (x = 0; x < map->map_width; x++)
    for (y = 0; y < map->map_height; y++)
      if (map_real_terrain_get(map, x, y) == WATER)
        if (btech_random_range(map->xcode.context, 1, 100) <= num &&
            growable(map, x, y)) {
          map_terrain_set(map, x, y, TMP_TERR);
          count++;
        }
  for (x = 0; x < map->map_width; x++)
    for (y = 0; y < map->map_height; y++)
      if (map_real_terrain_get(map, x, y) == TMP_TERR)
        map_terrain_set(map, x, y, ICE);
  if (count)
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d hexes 'iced'.", count);
  else
    notify(btech_context_evaluation(map->xcode.context), player,
           "No hexes 'iced'.");
}

void ice_melt(DbRef player, BattleMap *map, int num) {
  int x, y;
  int count = 0;

  for (x = 0; x < map->map_width; x++)
    for (y = 0; y < map->map_height; y++)
      if (map_real_terrain_get(map, x, y) == ICE)
        if (btech_random_range(map->xcode.context, 1, 100) <= num &&
            meltable(map, x, y)) {
          break_sub(map, NULL, x, y, "goes swimming as ice breaks!");
          map_terrain_set(map, x, y, TMP_TERR);
          count++;
        }
  for (x = 0; x < map->map_width; x++)
    for (y = 0; y < map->map_height; y++)
      if (map_real_terrain_get(map, x, y) == TMP_TERR)
        map_terrain_set(map, x, y, WATER);
  if (count)
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d hexes melted.", count);
  else
    notify(btech_context_evaluation(map->xcode.context), player,
           "No hexes melted.");
}

void map_addice(DbRef player, BattleMap *map, char *buffer) {
  char *args[2];
  int num;

  DOCHECK_CONTEXT(map->xcode.context,
                  mech_parseattributes(buffer, args, 2) != 1,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(map->xcode.context, Readnum(num, args[0]), "Invalid number!");
  ice_growth(player, map, num);
}

void map_delice(DbRef player, BattleMap *map, char *buffer) {
  char *args[2];
  int num;

  DOCHECK_CONTEXT(map->xcode.context,
                  mech_parseattributes(buffer, args, 2) != 1,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(map->xcode.context, Readnum(num, args[0]), "Invalid number!");
  ice_melt(player, map, num);
}

void possibly_blow_ice(Mech *mech, int weapindx, int x, int y) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);

  if (map_real_terrain_get(map, x, y) != ICE)
    return;
  if (btech_random_range(mech->xcode.context, 1, 15) >
      MechWeapons[weapindx].damage)
    return;
  HexLOSBroadcast(map, x, y, "The ice breaks from the blast!");
  break_sub(map, NULL, x, y, "goes swimming as ice breaks!");
}

void possibly_blow_bridge(Mech *mech, int weapindx, int x, int y) {
  BattleMap *map =
      btech_context_find_object(mech->xcode.context, mech->mapindex);

  if (map_real_terrain_get(map, x, y) != BRIDGE)
    return;
  if (MapBridgesCS(map))
    return;
  if (btech_random_range(mech->xcode.context, 1,
                         10 * (1 + map_elevation_get(map, x, y))) >
      MechWeapons[weapindx].damage) {
    HexLOSBroadcast(map, x, y, "The bridge at $H shudders from direct hit!");
    return;
  }
  HexLOSBroadcast(map, x, y, "The bridge at $H is blown apart!");
  break_sub(map, NULL, x, y, "goes swimming as the bridge is blown apart!");
}
