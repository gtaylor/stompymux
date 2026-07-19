/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/file.h>

#include "autopilot.h"
#include "mech.events.h"
#include "mech.h"
#include "mux/network/mux_event_alloc.h"
#include "p.mech.build.h"
#include "p.mech.c3.h"
#include "p.mech.c3i.h"
#include "p.mech.utils.h"
#include "p.mechrep.h"

/* Selectors for new/free function */
#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

void clear_mech_from_LOS(MECH *mech) {
  MAP *map;
  int i;
  MECH *mek;

  /* if (mech->mapindex < 0)
     return;
   */
  if (!(map = btech_context_find_object(mech->xcode.context, mech->mapindex)))
    return;
#ifdef SENSOR_DEBUG
  SendSensor(mech->xcode.context,
             tprintf("LOS info for #%d cleared.", mech->mynum));
#endif
  for (i = 0; i < map->first_free; i++) {
    map->LOSinfo[mech->mapnumber][i] = 0;
    map->LOSinfo[i][mech->mapnumber] = 0;

    if (map->mechsOnMap[i] >= 0 && i != mech->mapnumber) {
      if (!(mek = btech_context_get_mech(mech->xcode.context,
                                         map->mechsOnMap[i])))
        continue;
      if ((MechStatus(mek) & LOCK_TARGET) && MechTarget(mek) == mech->mynum) {
        mech_notify(mek, MECHALL,
                    "Weapon system reports the lock has been lost.");
        LoseLock(mek);
      }
      if ((map->LOSinfo[i][mech->mapnumber] & MECHLOSFLAG_SEEN) &&
          MechTeam(mek) != MechTeam(mech))
        MechNumSeen(mek) = MAX(0, MechNumSeen(mek) - 1);
    }
  }
  if (MechStatus(mech) & LOCK_MODES) {
    mech_notify(mech, MECHALL, "Weapon system reports the lock has been lost.");
    LoseLock(mech);
  }
}

void mech_Rsetxy(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;
  MAP *mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  char *args[3];
  int x, y, z, argc;

  cch(MECH_MAP);
  argc = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(mech->xcode.context, argc != 2 && argc != 3,
                  "Invalid number of arguments to SETXY!");
  x = atoi(args[0]);
  y = atoi(args[1]);
  DOCHECK_CONTEXT(mech->xcode.context,
                  x >= mech_map->map_width || y >= mech_map->map_height ||
                      x < 0 || y < 0,
                  "Invalid coordinates!");
  MechX(mech) = x;
  MechLastX(mech) = x;
  MechY(mech) = y;
  MechLastY(mech) = y;
  MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
  MechTerrain(mech) = GetTerrain(mech_map, MechX(mech), MechY(mech));
  MarkForLOSUpdate(mech);
  if (argc == 2) {
    MechElev(mech) = GetElev(mech_map, MechX(mech), MechY(mech));
    MechZ(mech) = MechElev(mech) - 1;
    MechFZ(mech) = ZSCALE * MechZ(mech);
    DropSetElevation(mech, 0);
    z = MechZ(mech);
    if (!Landed(mech) && FlyingT(mech))
      MechStatus(mech) |= LANDED;
  } else {
    z = atoi(args[2]);
    MechZ(mech) = z;
    MechFZ(mech) = ZSCALE * MechZ(mech);
    MechElev(mech) = GetElev(mech_map, MechX(mech), MechY(mech));
  }
  clear_mech_from_LOS(mech);
  notify_printf(btech_context_evaluation(mech->xcode.context), player,
                "Pos changed to %d,%d,%d", x, y, z);
  SendLoc(
      tprintf("#%d set #%d's pos to %d,%d,%d.", player, mech->mynum, x, y, z));
}

/* Team/Map commands */
void mech_Rsetmapindex(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;
  char *args[2], *tempstr;
  int newindex, nargs, notdone = 0;
  int loop;
  MAP *newmap = NULL;
  MAP *oldmap;
  MECH *tempMech;
  char targ[2];

  nargs = mech_parseattributes(buffer, args, 2);
  DOCHECK_CONTEXT(mech->xcode.context, nargs < 1,
                  "Invalid number of arguments to SETMAPINDX!");
  newindex = atoi(args[0]);
  DOCHECK_CONTEXT(mech->xcode.context, newindex < -1, "Invalid map index!");
  if (newindex != -1) {
    if (!(newmap = ValidMap(mech->xcode.context, player, newindex)))
      return;
  }
  /* Remove the mech from it's old map */
  if (mech->mapindex != -1) {
    if (!(oldmap = ValidMap(mech->xcode.context, player, mech->mapindex)))
      return;
    TAGTarget(mech) = -1;
    clearC3iNetwork(mech, 1);
    clearC3Network(mech, 1);
    remove_mech_from_map(oldmap, mech);
  }

  if (newindex == -1) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Mech removed from map.");
    SendLoc(tprintf("#%d removed #%d from map #%d.", player, mech->mynum,
                    oldmap->mynum));
    return;
  }

  /* Just make it random */
  /* Find a clear spot for this mech */
  if (nargs > 1 && strlen(args[1]) > 1) {
    targ[0] = args[1][0];
    targ[1] = args[1][1];
  } else if ((tempstr = btech_attribute_read(mech->xcode.context->database,
                                             mech->mynum, A_MECHPREFID,
                                             (char[LBUF_SIZE]){0})) &&
             strlen(tempstr) > 1) {
    targ[0] = tempstr[0];
    targ[1] = tempstr[1];
  } else {
    targ[0] = 65 + btech_random_range(mech->xcode.context, 0, 25);
    targ[1] = 65 + btech_random_range(mech->xcode.context, 0, 25);
  }
  targ[0] = BOUNDED('A', toupper(targ[0]), 'Z');
  targ[1] = BOUNDED('A', toupper(targ[1]), 'Z');
  for (loop = 0; (loop < newmap->first_free && !notdone); loop++) {
    if ((tempMech = (MECH *)btech_context_find_object(
             mech->xcode.context, newmap->mechsOnMap[loop])))
      if (MechID(tempMech)[0] == targ[0] && MechID(tempMech)[1] == targ[1])
        notdone = 1;
  }
  while (notdone) {
    targ[0] = 65 + btech_random_range(mech->xcode.context, 0, 25);
    targ[1] = 65 + btech_random_range(mech->xcode.context, 0, 25);
    notdone = 0;
    for (loop = 0; (loop < newmap->first_free && !notdone); loop++) {
      if ((tempMech = (MECH *)btech_context_find_object(
               mech->xcode.context, newmap->mechsOnMap[loop])))
        if (MechID(tempMech)[0] == targ[0] && MechID(tempMech)[1] == targ[1])
          notdone = 1;
    }
  }
  DOCHECK_CONTEXT(mech->xcode.context, loop == MAX_MECHS_PER_MAP,
                  "There are too many mechs on that map!");
  add_mech_to_map(newmap, mech);
  MechID(mech)[0] = targ[0];
  MechID(mech)[1] = targ[1];
  if (MechX(mech) > (newmap->map_width - 1) ||
      MechY(mech) > (newmap->map_height - 1)) {
    MechX(mech) = 0;
    MechLastX(mech) = 0;
    MechY(mech) = 0;
    MechLastY(mech) = 0;
    MapCoordToRealCoord(MechX(mech), MechY(mech), &MechFX(mech), &MechFY(mech));
    MechTerrain(mech) = GetTerrain(newmap, MechX(mech), MechY(mech));
    MechElev(mech) = GetElev(newmap, MechX(mech), MechY(mech));
    notify(btech_context_evaluation(mech->xcode.context), player,
           "You're current position is out of bounds, Pos changed to 0,0");
  }
  notify_printf(btech_context_evaluation(mech->xcode.context), player,
                "MapIndex changed to %d", newindex);
  notify_printf(btech_context_evaluation(mech->xcode.context), player,
                "Your ID: %c%c", MechID(mech)[0], MechID(mech)[1]);
  SendLoc(
      tprintf("#%d set #%d's mapindex to #%d.", player, mech->mynum, newindex));
  UnZombifyMech(mech);
}

void mech_Rsetteam(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data;
  char *args[1];
  int team;
  MAP *newmap;

  DOCHECK_CONTEXT(mech->xcode.context, mech->mapindex == -1,
                  "Mech is not on a map:  Can't set team");
  newmap = ValidMap(mech->xcode.context, player, mech->mapindex);
  if (!newmap) {
    notify(btech_context_evaluation(mech->xcode.context), player,
           "Map index reset!");
    mech->mapindex = -1;
    return;
  }
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 1) != 1,
                  "Invalid number of arguments!");
  team = atoi(args[0]);
  if (team < 0)
    team = 0;
  MechTeam(mech) = team;
  notify_printf(btech_context_evaluation(mech->xcode.context), player,
                "Team set to %d", team);
}

#define SPECIAL_FREE 0
#define SPECIAL_ALLOC 1

extern void auto_stop_pilot(AUTO *autopilot);
/* Alloc/free routine */
void newfreemech(DbRef key, void **data, int selector) {
  MECH *new = *data;
  MAP *map;
  int i;
  command_node *temp;

  switch (selector) {
  case SPECIAL_ALLOC:
    new->mynum = key;
    new->mapnumber = 1;
    new->mapindex = -1;
    MechID(new)[0] = ' ';
    MechID(new)[1] = ' ';
    clear_mech(new, 1);
    for (i = 0; i < NUM_SECTIONS; i++)
      FillDefaultCriticals(new, i);
    break;
  case SPECIAL_FREE:
    if (new->mapindex != -1 &&
        (map = btech_context_get_map(new->xcode.context, new->mapindex)))
      remove_mech_from_map(map, new);
    if (MechAuto(new) > 0) {
      AUTO *autopilot =
          btech_context_find_object(new->xcode.context, MechAuto(new));
      if (autopilot) {
        auto_stop_pilot(autopilot);
        /* Go through the list and remove any leftover nodes */
        while (doubly_linked_list_size(autopilot->commands)) {

          /* Remove the first node on the list and get the data
           * from it */
          temp = (command_node *)doubly_linked_list_remove(
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
      MechAuto(new) = -1;
    }
  }
}
