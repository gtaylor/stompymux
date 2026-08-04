
/*
 * $Id: map.dynamic.c,v 1.1.1.1 2005/01/11 21:18:08 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Oct 13 19:38:31 1996 fingon
 * Last modified: Sun Jun 14 14:54:11 1998 fingon
 *
 */

#include <stdio.h>
#include <stdlib.h>

#include "autopilot.h"
#include "btech_channel.h"
#include "econ_cmds_api.h"
#include "map_conditions_api.h"
#include "map_dynamic_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_runtime_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

void mech_map_consistency_check(Mech *mech) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

  if (!map) {
    if (mech_map_dbref(mech) > 0) {
      mech_map_dbref_set(mech, -1);
      fprintf(stderr, "#%ld on nonexistent map - removing..\n",
              mech_dbref(mech));
    }
    return;
  }
  if (map->first_free <= mech_map_slot(mech)) {
    /* Invalid: possible corruption of data, therefore un-hosing it */
    mech_map_dbref_set(mech, -1);
    mech_remove_from_all_maps(mech);
    fprintf(stderr, "#%ld on invalid map - removing.. (#1)\n",
            mech_dbref(mech));
    return;
  }
  if (map->mechsOnMap[mech_map_slot(mech)] != mech_dbref(mech)) {
    fprintf(stderr,
            "#%ld on invalid map - removing .. (#2) -- mapindex: %ld "
            "mapnumber: %d mechsOnMap: %ld\n",
            mech_dbref(mech), mech_map_dbref(mech), mech_map_slot(mech),
            map->mechsOnMap[mech_map_slot(mech)]);
    mech_map_dbref_set(mech, -1);
    mech_remove_from_all_maps(mech);
    return;
  }
  mech_remove_from_all_maps_except(mech, map->mynum);
}

void eliminate_empties(BattleMap *map) {
  int i;
  int j;
  int count, oldcount;
  if (!map)
    return;
  for (i = map->first_free - 1; i >= 0; i--)
    if (map->mechsOnMap[i] > 0)
      break;
  count = i + 1;
  if (count == (oldcount = map->first_free))
    return;
  fprintf(stderr, "Map #%ld contains empty entries ; removing %d (%d->%d)\n",
          map->mynum, oldcount - count, oldcount, count);
  if (i < 0)
    return;
  for (j = count; j < oldcount; j++)
    free((void *)map->LOSinfo[j]);
  ReCreate(map->LOSinfo, unsigned short *, count);

  ReCreate(map->mechsOnMap, DbRef, count);
  ReCreate(map->mechflags, char, count);

  map->first_free = count;
  map->dynamic_size = count;
  econ_fix_stuff(map->xcode.context, GOD, map->mynum);
}

void remove_mech_from_map(BattleMap *map, Mech *mech) {
  int loop = map->first_free;

  clear_mech_from_LOS(mech);
  mech_map_dbref_set(mech, -1);
  if (map->first_free <= mech_map_slot(mech) ||
      map->mechsOnMap[mech_map_slot(mech)] != mech_dbref(mech)) {
    btech_channel_send(
        map->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Map indexing error for mech #%ld: Map index %d contains "
                "data for #%ld instead.",
                mech_dbref(mech), mech_map_slot(mech),
                map->mechsOnMap ? map->mechsOnMap[mech_map_slot(mech)] : -1));
    if (map->mechsOnMap)
      for (loop = 0; (loop < map->first_free) &&
                     (map->mechsOnMap[loop] != mech_dbref(mech));
           loop++)
        ;
  } else
    loop = mech_map_slot(mech);
  mech_map_slot_set(mech, 0);
  if (loop != (map->first_free)) {
    map->mechsOnMap[loop] = -1; /* clear it */
    map->mechflags[loop] = 0;
    if (loop == (map->first_free - 1))
      map->first_free--; /* Who cares about some lost memory? In realloc
                                            we'll gain it back anyway */
  }
  if (mech_is_towed(mech)) {
    /* Check that the towing guy isn't left on the map */
    int i;
    Mech *t;

    for (i = 0; i < map->first_free; i++)
      /* Release from towing if tow-guy ain't on same map already */
      if ((t = btech_context_get_mech(map->xcode.context, map->mechsOnMap[i])))
        if (mech_carried_dbref(t) == mech_dbref(mech)) {
          mech_carried_dbref_set(t, -1);
          mech_towed_clear(mech);
          break;
        }
  }
  mech_seen_count_reset(mech);
  if (mech_is_dropship(mech))
    btech_channel_send(
        map->xcode.context, BTECH_CHANNEL_DS_INFO, "%s",
        tprintf("DS #%ld has left map #%ld", mech_dbref(mech), map->mynum));
}

void add_mech_to_map(BattleMap *newmap, Mech *mech) {
  int loop, count, i;

  for (loop = 0; loop < newmap->first_free; loop++)
    if (newmap->mechsOnMap[loop] == mech_dbref(mech))
      break;
  if (loop != newmap->first_free)
    return;
  for (loop = 0; loop < newmap->first_free; loop++)
    if (newmap->mechsOnMap[loop] < 0)
      break;
  if (loop == newmap->first_free) {
    newmap->first_free++;
    count = newmap->first_free;
    ReCreate(newmap->mechsOnMap, DbRef, count);
    ReCreate(newmap->mechflags, char, count);
    ReCreate(newmap->LOSinfo, unsigned short *, count);

    newmap->LOSinfo[count - 1] = nullptr;
    for (i = 0; i < count; i++) {
      ReCreate(newmap->LOSinfo[i], unsigned short, count);

      newmap->LOSinfo[i][loop] = 0;
    }
    for (i = 0; i < count; i++)
      newmap->LOSinfo[loop][i] = 0;
    newmap->dynamic_size = count;
  }
  mech_map_dbref_set(mech, newmap->mynum);
  mech_map_slot_set(mech, loop);
  newmap->mechsOnMap[loop] = mech_dbref(mech);
  newmap->mechflags[loop] = 0;

  /* Is there an autopilot */
  if (mech_autopilot_dbref(mech) > 0) {

    Autopilot *a = btech_context_find_object(mech_context(mech),
                                             mech_autopilot_dbref(mech));

    /* Reset the AI's comtitle */
    if (a)
      auto_set_comtitle(a, mech);
  }

  if (mech_is_towed(mech)) {
    int tow_index;
    Mech *t;

    for (tow_index = 0; tow_index < newmap->first_free; tow_index++)
      /* Release from towing if tow-guy ain't on same map already */
      if ((t = btech_context_get_mech(newmap->xcode.context,
                                      newmap->mechsOnMap[tow_index])))
        if (mech_carried_dbref(t) == mech_dbref(mech))
          break;
    if (tow_index == newmap->first_free)
      mech_towed_clear(mech);
  }
  MarkForLOSUpdate(mech);
  UnZombifyMech(mech);
  map_conditions_apply(mech, newmap);
  if (mech_is_dropship(mech))
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
                       tprintf("DS #%ld has entered map #%ld", mech_dbref(mech),
                               newmap->mynum));
}

int mech_size(BattleMap *map) {
  return map->first_free *
         (sizeof(DbRef) + sizeof(char) + sizeof(unsigned short *) +
          map->first_free * sizeof(unsigned short));
}
