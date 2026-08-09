
/* Implements dynamic map state and updates. */

#include <stdio.h>
#include <stdlib.h>

#include "autopilot.h"
#include "btech_channel.h"
#include "econ_cmds_api.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_dynamic_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
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
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

static size_t battle_map_slot_capacity(const BattleMap *map) {
  return (size_t)(map->dynamic_size > map->first_free ? map->dynamic_size
                                                      : map->first_free);
}

static DbRef *battle_map_unit_slot(BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(map->mechsOnMap, battle_map_slot_capacity(map),
                            sizeof(*map->mechsOnMap), (size_t)index);
}

static const DbRef *battle_map_unit_slot_const(const BattleMap *map,
                                               int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(map->mechsOnMap,
                                  battle_map_slot_capacity(map),
                                  sizeof(*map->mechsOnMap), (size_t)index);
}

static char *battle_map_flag_slot(BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(map->mechflags, battle_map_slot_capacity(map),
                            sizeof(*map->mechflags), (size_t)index);
}

static const char *battle_map_flag_slot_const(const BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(map->mechflags, battle_map_slot_capacity(map),
                                  sizeof(*map->mechflags), (size_t)index);
}

static unsigned short **battle_map_los_row_slot(BattleMap *map, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(map->LOSinfo, battle_map_slot_capacity(map),
                            sizeof(*map->LOSinfo), (size_t)index);
}

static unsigned short *battle_map_los_cell(BattleMap *map, int row,
                                           int column) {
  if (column < 0)
    abort();
  unsigned short *values = *battle_map_los_row_slot(map, row);
  return checked_storage_at(values, battle_map_slot_capacity(map),
                            sizeof(*values), (size_t)column);
}

static void *resize_storage(void *storage, size_t count, size_t item_size) {
  void *resized = realloc(storage, count * item_size);
  if (resized == nullptr && count > 0)
    abort();
  return resized;
}

void battle_map_dynamic_destroy(BattleMap *map) {
  int allocated_slots;

  if (!map)
    return;
  allocated_slots = map->dynamic_size;
  /* Restored maps may have slot data before their allocation size is
   * updated. */
  if (allocated_slots < map->first_free)
    allocated_slots = map->first_free;
  if (map->LOSinfo)
    for (int index = 0; index < allocated_slots; index++)
      free(*battle_map_los_row_slot(map, index));
  free(map->LOSinfo);
  free(map->mechflags);
  free(map->mechsOnMap);
  map->LOSinfo = nullptr;
  map->mechflags = nullptr;
  map->mechsOnMap = nullptr;
  map->dynamic_size = 0;
}

int battle_map_unit_count(const BattleMap *map) { return map->first_free; }

DbRef battle_map_unit_dbref(const BattleMap *map, int index) {
  return *battle_map_unit_slot_const(map, index);
}

int battle_map_unit_flags(const BattleMap *map, int index) {
  return *battle_map_flag_slot_const(map, index);
}

DbRef battle_map_dbref(const BattleMap *map) { return map->mynum; }

BtechContext *battle_map_context(const BattleMap *map) {
  return map->xcode.context;
}

void battle_map_unit_slot_clear(BattleMap *map, int index) {
  *battle_map_unit_slot(map, index) = -1;
}

void battle_map_unit_moved_flags_clear(BattleMap *map) {
  for (int i = 0; i < map->first_free; i++)
    *battle_map_flag_slot(map, i) = 0;
}

void battle_map_unit_moved_set(BattleMap *map, int index) {
  *battle_map_flag_slot(map, index) = 1;
}

int battle_map_width(const BattleMap *map) { return map->map_width; }

int battle_map_height(const BattleMap *map) { return map->map_height; }

DbRef battle_map_parent_dbref(const BattleMap *map) { return map->onmap; }

bool battle_map_blocks_friendly_fire(const BattleMap *map) {
  return map->flags & MAPFLAG_NOFRIENDLYFIRE;
}

bool battle_map_blocks_physical_attacks(const BattleMap *map) {
  return map->flags & MAPFLAG_NOPHYSICALS;
}

bool battle_map_is_combat_safe(const BattleMap *map) {
  return map->buildflag & BUILDFLAG_CSI;
}

void battle_map_parent_dbref_set(BattleMap *map, DbRef parent) {
  map->onmap = parent;
}

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
  if (battle_map_unit_dbref(map, mech_map_slot(mech)) != mech_dbref(mech)) {
    fprintf(stderr,
            "#%ld on invalid map - removing .. (#2) -- mapindex: %ld "
            "mapnumber: %d mechsOnMap: %ld\n",
            mech_dbref(mech), mech_map_dbref(mech), mech_map_slot(mech),
            battle_map_unit_dbref(map, mech_map_slot(mech)));
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
    if (battle_map_unit_dbref(map, i) > 0)
      break;
  count = i + 1;
  if (count == (oldcount = map->first_free))
    return;
  fprintf(stderr, "Map #%ld contains empty entries ; removing %d (%d->%d)\n",
          map->mynum, oldcount - count, oldcount, count);
  if (i < 0)
    return;
  const size_t allocation_count = (size_t)count;
  for (j = count; j < oldcount; j++)
    free(*battle_map_los_row_slot(map, j));
  map->LOSinfo =
      resize_storage(map->LOSinfo, allocation_count, sizeof(*map->LOSinfo));
  map->mechsOnMap = resize_storage(map->mechsOnMap, allocation_count,
                                   sizeof(*map->mechsOnMap));
  map->mechflags =
      resize_storage(map->mechflags, allocation_count, sizeof(*map->mechflags));

  map->first_free = count;
  map->dynamic_size = count;
  econ_fix_stuff(map->xcode.context, GOD, map->mynum);
}

void remove_mech_from_map(BattleMap *map, Mech *mech) {
  int loop = map->first_free;

  clear_mech_from_LOS(mech);
  mech_map_dbref_set(mech, -1);
  const int map_slot = mech_map_slot(mech);
  if (map->mechsOnMap == nullptr || map->mechflags == nullptr || map_slot < 0 ||
      map->first_free <= map_slot ||
      battle_map_unit_dbref(map, map_slot) != mech_dbref(mech)) {
    const DbRef indexed_dbref =
        map->mechsOnMap && map_slot >= 0 && map_slot < map->first_free
            ? battle_map_unit_dbref(map, map_slot)
            : -1;
    btech_channel_send(
        map->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
        tprintf("Map indexing error for mech #%ld: Map index %d contains "
                "data for #%ld instead.",
                mech_dbref(mech), map_slot, indexed_dbref));
    if (map->mechsOnMap)
      for (loop = 0; (loop < map->first_free) &&
                     (battle_map_unit_dbref(map, loop) != mech_dbref(mech));
           loop++)
        ;
  } else
    loop = map_slot;
  mech_map_slot_set(mech, 0);
  if (map->mechsOnMap != nullptr && map->mechflags != nullptr &&
      loop != map->first_free) {
    *battle_map_unit_slot(map, loop) = -1; /* clear it */
    *battle_map_flag_slot(map, loop) = 0;
    if (loop == (map->first_free - 1))
      map->first_free--; /* Who cares about some lost memory? In realloc
                                            we'll gain it back anyway */
  }
  if (mech_is_towed(mech)) {
    /* Check that the towing guy isn't left on the map */
    int i;
    Mech *t;

    for (i = 0; map->mechsOnMap != nullptr && i < map->first_free; i++)
      /* Release from towing if tow-guy ain't on same map already */
      if ((t = btech_context_get_mech(map->xcode.context,
                                      battle_map_unit_dbref(map, i))))
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
    if (battle_map_unit_dbref(newmap, loop) == mech_dbref(mech))
      break;
  if (loop != newmap->first_free)
    return;
  for (loop = 0; loop < newmap->first_free; loop++)
    if (battle_map_unit_dbref(newmap, loop) < 0)
      break;
  if (loop == newmap->first_free) {
    newmap->first_free++;
    count = newmap->first_free;
    const size_t allocation_count = (size_t)count;
    newmap->mechsOnMap = resize_storage(newmap->mechsOnMap, allocation_count,
                                        sizeof(*newmap->mechsOnMap));
    newmap->mechflags = resize_storage(newmap->mechflags, allocation_count,
                                       sizeof(*newmap->mechflags));
    newmap->LOSinfo = resize_storage(newmap->LOSinfo, allocation_count,
                                     sizeof(*newmap->LOSinfo));
    newmap->dynamic_size = count;

    *battle_map_los_row_slot(newmap, count - 1) = nullptr;
    for (i = 0; i < count; i++) {
      unsigned short **row_slot = battle_map_los_row_slot(newmap, i);
      *row_slot =
          resize_storage(*row_slot, allocation_count, sizeof(**row_slot));

      *battle_map_los_cell(newmap, i, loop) = 0;
    }
    for (i = 0; i < count; i++)
      *battle_map_los_cell(newmap, loop, i) = 0;
  }
  mech_map_dbref_set(mech, newmap->mynum);
  mech_map_slot_set(mech, loop);
  *battle_map_unit_slot(newmap, loop) = mech_dbref(mech);
  *battle_map_flag_slot(newmap, loop) = 0;

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
      if ((t = btech_context_get_mech(
               newmap->xcode.context,
               battle_map_unit_dbref(newmap, tow_index))))
        if (mech_carried_dbref(t) == mech_dbref(mech))
          break;
    if (tow_index == newmap->first_free)
      mech_towed_clear(mech);
  }
  MarkForLOSUpdate(mech);
  autopilot_resume_for_mech(mech);
  map_conditions_apply(mech, newmap);
  if (mech_is_dropship(mech))
    btech_channel_send(mech_context(mech), BTECH_CHANNEL_DS_INFO, "%s",
                       tprintf("DS #%ld has entered map #%ld", mech_dbref(mech),
                               newmap->mynum));
}

size_t mech_size(const BattleMap *map) {
  const size_t unit_count = (size_t)map->first_free;
  return unit_count * (sizeof(DbRef) + sizeof(char) + sizeof(unsigned short *) +
                       unit_count * sizeof(unsigned short));
}
