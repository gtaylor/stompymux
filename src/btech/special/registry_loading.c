#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_api.h"
#include "map_terrain.h"
#include "mech_parts.h"               // IWYU pragma: keep
#include "mech_scan_api.h"            // IWYU pragma: keep
#include "mech_status_api.h"          // IWYU pragma: keep
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/*
 * $Id: registry.c,v 1.4 2005/08/08 09:43:09 murrayma Exp $
 *
 * Original author: unknown
 *
 * Copyright (c) 1996-2002 Markus Stenberg
 * Copyright (c) 1998-2002 Thomas Wouters
 * Copyright (c) 2000-2002 Cord Awtry
 *
 * Last modified: Thu Jul  9 02:40:16 1998 fingon
 *
 * This includes the basic code to allow objects to have hardcoded
 * commands / properties.
 *
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "ds_turret_api.h"
#include "legacy_macros.h"
#include "map_dynamic_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

#define FAST_WHICHSPECIAL

#define _GLUE_C

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "coolmenu.h"
#include "mech.h"
#include "mech_events.h"
#include "mech_partnames_api.h"
#include "mech_stat_api.h"
#include "mechrep.h"
#include "mechrep_api.h"
#include "mux/objects/powers.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "mycool.h"
#include "registry_internal.h"
#include "turret.h"
static int remove_from_all_maps_func(void *key, void *data, int depth,
                                     void *arg) {
  BtechSpecialObject *const xcode_obj = data;
  Mech *const mech = arg;

  if (xcode_obj->type == GTYPE_MAP) {
    BattleMap *map;
    int i;

    if (!(map = btech_context_get_map(mech->xcode.context, (DbRef)key)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps(Mech *mech) {
  red_black_tree_walk(mech->xcode.context->special_objects, WALK_INORDER,
                      remove_from_all_maps_func, mech);
}

typedef struct RemoveFromAllMapsContext {
  Mech *mech;
  DbRef except_map;
} RemoveFromAllMapsContext;

static int remove_from_all_maps_except_func(void *key, void *data, int depth,
                                            void *arg) {
  DbRef key_val = (DbRef)key;
  BtechSpecialObject *const xcode_obj = data;
  RemoveFromAllMapsContext *context = arg;
  Mech *const mech = context->mech;

  if (xcode_obj->type == GTYPE_MAP) {
    int i;
    BattleMap *map;

    if (key_val == context->except_map)
      return 1;
    if (!(map = btech_context_get_map(mech->xcode.context, key_val)))
      return 1;
    for (i = 0; i < map->first_free; i++)
      if (map->mechsOnMap[i] == mech->mynum)
        map->mechsOnMap[i] = -1;
  }
  return 1;
}

void mech_remove_from_all_maps_except(Mech *mech, int num) {
  RemoveFromAllMapsContext context = {
      .mech = mech,
      .except_map = num,
  };

  red_black_tree_walk(mech->xcode.context->special_objects, WALK_INORDER,
                      remove_from_all_maps_except_func, &context);
}

static int load_update2(void *key, void *data, int depth, void *arg) {
  BtechSpecialObject *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MECH)
    mech_map_consistency_check((void *)xcode_obj);
  return 1;
}

static int load_update4(void *key, void *data, int depth, void *arg) {
  BtechSpecialObject *const xcode_obj = data;
  BtechContext *const context = arg;

  if (xcode_obj->type == GTYPE_MECH) {
    Mech *const mech = (Mech *)xcode_obj;
    BattleMap *map;

    if (!(map = btech_context_get_map(context, mech->mapindex))) {
      /* Ugly kludge */
      if ((map = btech_context_get_map(
               context, game_object_location(context->database, mech->mynum))))
        mech_Rsetmapindex(GOD, mech,
                          tprintf("%ld", game_object_location(context->database,
                                                              mech->mynum)));
      if (!(map = btech_context_get_map(context, mech->mapindex)))
        return 1;
    }

    if (!Started(mech))
      return 1;
    mech_start_seeing(mech);
    mech_update_recycling(mech);
    mech_maybe_move(mech);
  }
  return 1;
}

static int load_update3(void *key, void *data, int depth, void *arg) {
  BtechSpecialObject *const xcode_obj = data;

  if (xcode_obj->type == GTYPE_MAP) {
    eliminate_empties((BattleMap *)xcode_obj);
    recalculate_minefields((BattleMap *)xcode_obj);
  }
  return 1;
}

/*
 * Read in autopilot data
 */
static int load_autopilot_data(void *key, void *data, int depth, void *arg) {
  BtechSpecialObject *const xcode_obj = data;
  BtechContext *const context = arg;

  if (xcode_obj->type == GTYPE_AUTO) {
    Autopilot *const autopilot = (Autopilot *)xcode_obj;

    int i;

    /* Commands and A* paths are restored before these derived caches. */
    autopilot->weaplist = NULL;
    for (i = 0; i < AUTO_PROFILE_MAX_SIZE; i++) {
      autopilot->profile[i] = NULL;
    }

    if (!autopilot->mymechnum || !(autopilot->mymech = btech_context_get_mech(
                                       context, autopilot->mymechnum))) {
      DoStopGun(autopilot);
    } else {
      /*
       * Weapon lists and range profiles are caches derived from the restored
       * MECH definition. Rebuild them instead of persisting cache trees.
       */
      auto_update_profile_event(autopilot);

      /*
       * MUX event nodes are runtime-only. An autopilot that was engaged at
       * checkpoint time is identified by the durable MECH->AUTO link and by
       * the AUTO object being inside that MECH. Requeue its dispatcher from
       * the durable command list; it recreates goal-specific events itself.
       */
      if (MechAuto(autopilot->mymech) == autopilot->mynum &&
          game_object_location(context->database, autopilot->mynum) ==
              autopilot->mymechnum &&
          autopilot->commands &&
          doubly_linked_list_size(autopilot->commands) > 0 &&
          !mux_event_count_type_data(context->events, EVENT_AUTOCOM, autopilot))
        AUTO_COM(autopilot, AUTOPILOT_NC_DELAY);
      if (Gunning(autopilot))
        DoStartGun(autopilot);
    }
  }

  return 1;
}

void btech_special_objects_load(BtechContext *context) {
  DbRef i;
  int type;

  btech_registry_tree_initialize(context);
  context->special_commands =
      calloc(BTECH_SPECIAL_OBJECT_COUNT, sizeof(*context->special_commands));
  if (context->special_commands == nullptr)
    exit(EXIT_FAILURE);
  context->special_command_count = BTECH_SPECIAL_OBJECT_COUNT;

  mux_event_initialize(context->events);
  init_stat(context);
  initialize_partname_tables(context);
  if (!btech_weapon_settings_initialize(&context->weapon_settings))
    exit(EXIT_FAILURE);
  if (!missile_hit_registry_initialize(&context->missile_hits, context))
    exit(EXIT_FAILURE);
  /* Loop through the entire database, and if it has the special */
  /* object flag, add it to our linked list. */
  DO_WHOLE_DB(context->database, i)
  if (is_xcode(context->database, i) && !is_going(context->database, i) &&
      !is_halted(context->database, i)) {
    type = btech_context_which_special_attribute(context, i);
    if (type >= 0) {
      if (SpecialObjects[type].datasize > 0)
        NewSpecialObject(context, i, type);
    } else
      c_xcode(context->database, i); /* Reset the flag */
  }
  for (i = 0; i < (int)(BTECH_SPECIAL_OBJECT_COUNT); i++) {
    InitSpecialHash(context, i);
  }
  init_btechstats(context);
#ifdef BTMUX_PERSISTENCE_TESTING
  /* The integration fixture creates its initial SQLite special-state rows. */
  if (getenv("BTMUX_TEST_BTECH_BOOTSTRAP")) {
    btech_heartbeat_start(context);
    return;
  }
#endif
  if (btech_persistence_load_special_state_path(
          context, context->configuration->database.gamedb) < 0) {
    exit(EXIT_FAILURE);
  }
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update2,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update3,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER, load_update4,
                      context);
  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      load_autopilot_data, context);
  btech_heartbeat_start(context);
}

static int UpdateSpecialObject_func(void *key, void *data, int depth,
                                    void *arg) {
  BtechSpecialObject *const xcode_obj = data;
  BtechContext *const context = arg;

  if (!SpecialObjects[xcode_obj->type].updateTime)
    return 1;
  if ((context->clock->now % SpecialObjects[xcode_obj->type].updateTime))
    return 1;
  ((void (*)(DbRef, void *))SpecialObjects[xcode_obj->type].updatefunc)(
      (DbRef)key, xcode_obj);
  return 1;
}

/* This is called once a second for each special object */

/* Note the new handling for calls being done at <1second intervals,
   or possibly at >1second intervals */

void btech_special_objects_update(BtechContext *context) {
  const char *cmdsave;
  int i;
  int times = context->last_special_update
                  ? (context->clock->now - context->last_special_update)
                  : 1;

  if (times > 20)
    times = 20; /* Machine's hopelessly lagged,
                           we don't want to make it [much] worse */
  cmdsave = btech_context_command(context)->debug_command;
  for (i = 0; i < times; i++) {
    mux_event_run(context->events);
    btech_context_command(context)->debug_command =
        (char *)"< Generic hcode update handler>";
    red_black_tree_walk(context->special_objects, WALK_INORDER,
                        UpdateSpecialObject_func, context);
  }
  context->last_special_update = context->clock->now;
  btech_context_command(context)->debug_command = cmdsave;
}
