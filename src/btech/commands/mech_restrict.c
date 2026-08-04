/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_dynamic_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_build_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_stagger.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mechrep_api.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"

/* Selectors for new/free function */
#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

void clear_mech_from_LOS(Mech *mech) {
  BattleMap *map;
  int i;
  Mech *mek;

  /* if (mech_map_dbref(mech) < 0)
     return;
   */
  if (!(map = btech_context_find_object(mech_context(mech),
                                        mech_map_dbref(mech))))
    return;
#ifdef SENSOR_DEBUG
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_SENSOR, "%s",
                     tprintf("LOS info for #%d cleared.", mech_dbref(mech)));
#endif
  for (i = 0; i < map->first_free; i++) {
    map->LOSinfo[mech_map_slot(mech)][i] = 0;
    map->LOSinfo[i][mech_map_slot(mech)] = 0;

    if (map->mechsOnMap[i] >= 0 && i != mech_map_slot(mech)) {
      if (!(mek =
                btech_context_get_mech(mech_context(mech), map->mechsOnMap[i])))
        continue;
      if (mech_targeting_has_lock_on(mek, mech_dbref(mech))) {
        mech_notify(mek, MECHALL,
                    "Weapon system reports the lock has been lost.");
        mech_lose_lock(mek);
      }
      if ((map->LOSinfo[i][mech_map_slot(mech)] & MECHLOSFLAG_SEEN) &&
          mech_team(mek) != mech_team(mech))
        mech_seen_count_decrement(mek);
    }
  }
  if (mech_targeting_lock_modes_active(mech)) {
    mech_notify(mech, MECHALL, "Weapon system reports the lock has been lost.");
    mech_lose_lock(mech);
  }
}

void mech_Rsetxy(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  BattleMap *mech_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  char *args[3];
  int x, y, z, argc;
  float fx, fy;
  int elevation;

  cch(MECH_MAP);
  argc = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(mech_context(mech), argc != 2 && argc != 3,
                  "Invalid number of arguments to SETXY!");
  x = atoi(args[0]);
  y = atoi(args[1]);
  DOCHECK_CONTEXT(mech_context(mech),
                  x >= mech_map->map_width || y >= mech_map->map_height ||
                      x < 0 || y < 0,
                  "Invalid coordinates!");
  mech_position_xy_set(mech, x, y);
  MapCoordToRealCoord(x, y, &fx, &fy);
  mech_position_real_xy_set(mech, fx, fy);
  mech_position_terrain_set(mech, map_terrain_get(mech_map, x, y));
  MarkForLOSUpdate(mech);
  if (argc == 2) {
    elevation = map_elevation_get(mech_map, x, y);
    mech_position_elevation_set(mech, elevation);
    mech_position_z_set(mech, elevation - 1);
    DropSetElevation(mech, 0);
    z = mech_position_z(mech);
    mech_position_land_if_flying(mech);
  } else {
    z = atoi(args[2]);
    mech_position_z_set(mech, z);
    mech_position_elevation_set(mech, map_elevation_get(mech_map, x, y));
  }
  clear_mech_from_LOS(mech);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Pos changed to %d,%d,%d", x, y, z);
}

/* Team/Map commands */
void mech_Rsetmapindex(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[2], *tempstr;
  int newindex, nargs, notdone = 0;
  int loop;
  BattleMap *newmap = NULL;
  BattleMap *oldmap;
  Mech *tempMech;
  char targ[2];

  nargs = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech_context(mech), nargs < 1,
                  "Invalid number of arguments to SETMAPINDX!");
  newindex = atoi(args[0]);
  DOCHECK_CONTEXT(mech_context(mech), newindex < -1, "Invalid map index!");
  if (newindex != -1) {
    if (!(newmap = ValidMap(mech_context(mech), player, newindex)))
      return;
  }
  /* Remove the mech from it's old map */
  if (mech_map_dbref(mech) != -1) {
    if (!(oldmap = ValidMap(mech_context(mech), player, mech_map_dbref(mech))))
      return;
    mech_targeting_tag_clear(mech);
    clearC3iNetwork(mech, 1);
    clearC3Network(mech, 1);
    remove_mech_from_map(oldmap, mech);
  }

  if (newindex == -1) {
    notify(btech_context_evaluation(mech_context(mech)), player,
           "Mech removed from map.");
    return;
  }

  /* Just make it random */
  /* Find a clear spot for this mech */
  if (nargs > 1 && strlen(args[1]) > 1) {
    targ[0] = args[1][0];
    targ[1] = args[1][1];
  } else if ((tempstr = btech_attribute_read(mech_context(mech)->database,
                                             mech_dbref(mech), A_MECHPREFID,
                                             (char[LBUF_SIZE]){0})) &&
             strlen(tempstr) > 1) {
    targ[0] = tempstr[0];
    targ[1] = tempstr[1];
  } else {
    targ[0] = 65 + btech_random_range(mech_context(mech), 0, 25);
    targ[1] = 65 + btech_random_range(mech_context(mech), 0, 25);
  }
  targ[0] = BOUNDED('A', toupper(targ[0]), 'Z');
  targ[1] = BOUNDED('A', toupper(targ[1]), 'Z');
  for (loop = 0; (loop < newmap->first_free && !notdone); loop++) {
    if ((tempMech = (Mech *)btech_context_find_object(
             mech_context(mech), newmap->mechsOnMap[loop]))) {
      MechUnitId const id = mech_unit_id(tempMech);
      if (id.first == targ[0] && id.second == targ[1])
        notdone = 1;
    }
  }
  while (notdone) {
    targ[0] = 65 + btech_random_range(mech_context(mech), 0, 25);
    targ[1] = 65 + btech_random_range(mech_context(mech), 0, 25);
    notdone = 0;
    for (loop = 0; (loop < newmap->first_free && !notdone); loop++) {
      if ((tempMech = (Mech *)btech_context_find_object(
               mech_context(mech), newmap->mechsOnMap[loop]))) {
        MechUnitId const id = mech_unit_id(tempMech);
        if (id.first == targ[0] && id.second == targ[1])
          notdone = 1;
      }
    }
  }
  DOCHECK_CONTEXT(mech_context(mech), loop == MAX_MECHS_PER_MAP,
                  "There are too many mechs on that map!");
  add_mech_to_map(newmap, mech);
  mech_unit_id_set(mech, targ[0], targ[1]);
  if (mech_position_x(mech) > (newmap->map_width - 1) ||
      mech_position_y(mech) > (newmap->map_height - 1)) {
    float fx, fy;
    mech_position_reset_origin(mech);
    MapCoordToRealCoord(0, 0, &fx, &fy);
    mech_position_real_xy_set(mech, fx, fy);
    mech_position_terrain_set(mech, map_terrain_get(newmap, 0, 0));
    mech_position_elevation_set(mech, map_elevation_get(newmap, 0, 0));
    notify(btech_context_evaluation(mech_context(mech)), player,
           "You're current position is out of bounds, Pos changed to 0,0");
  }
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "MapIndex changed to %d", newindex);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Your ID: %c%c", mech_unit_id(mech).first,
                mech_unit_id(mech).second);
  autopilot_resume_for_mech(mech);
}

void mech_Rsetteam(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  char *args[1];
  int team;
  BattleMap *newmap;

  DOCHECK_CONTEXT(mech_context(mech), mech_map_dbref(mech) == -1,
                  "Mech is not on a map:  Can't set team");
  newmap = ValidMap(mech_context(mech), player, mech_map_dbref(mech));
  if (!newmap) {
    notify(btech_context_evaluation(mech_context(mech)), player,
           "Map index reset!");
    mech_map_dbref_set(mech, NOTHING);
    return;
  }
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  team = atoi(args[0]);
  if (team < 0)
    team = 0;
  mech_team_set(mech, team);
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "Team set to %d", team);
}

#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

extern void auto_stop_pilot(Autopilot *autopilot);
/* Alloc/free routine */
void newfreemech(DbRef key, void **data, int selector) {
  Mech *new = *data;
  BattleMap *map;
  int i;
  AutopilotCommand *temp;

  switch (selector) {
  case SPECIAL_ALLOC:
    mech_identity_initialize(new, key);
    clear_mech(new, 1);
    for (i = 0; i < NUM_SECTIONS; i++)
      FillDefaultCriticals(new, i);
    break;
  case SPECIAL_FREE:
    mech_stagger_damage_clear(new);
    if (mech_map_dbref(new) != -1 &&
        (map = btech_context_get_map(mech_context(new), mech_map_dbref(new))))
      remove_mech_from_map(map, new);
    if (mech_autopilot_dbref(new) > 0) {
      Autopilot *autopilot = btech_context_find_object(
          mech_context(new), mech_autopilot_dbref(new));
      if (autopilot) {
        auto_stop_pilot(autopilot);
        /* Go through the list and remove any leftover nodes */
        while (doubly_linked_list_size(autopilot->commands)) {

          /* Remove the first node on the list and get the data
           * from it */
          temp = (AutopilotCommand *)doubly_linked_list_remove(
              autopilot->commands,
              doubly_linked_list_head(autopilot->commands));

          /* Destroy the command node */
          auto_destroy_command_node(temp);
        }

        /* Destroy the list */
        doubly_linked_list_destroy_list(autopilot->commands);
        autopilot->commands = NULL;

        /* Destroy any astar path list thats on the AI */
        auto_destroy_astar_path(autopilot);

        /* Destroy profile array */
        for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
          if (autopilot->profile[i]) {
            red_black_tree_destroy(autopilot->profile[i]);
          }
          autopilot->profile[i] = NULL;
        }

        /* Destroy weaponlist */
        auto_destroy_weaplist(autopilot);

        autopilot->mymechnum = -1;
      }
      mech_autopilot_dbref_set(new, -1);
    }
  }
}
