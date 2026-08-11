#include "btech_event.h" // IWYU pragma: keep
#include "map.h"         // IWYU pragma: keep
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_parts.h"      // IWYU pragma: keep
#include "mech_scan_api.h"   // IWYU pragma: keep
#include "mech_status_api.h" // IWYU pragma: keep
#include "mux/server/log.h"
#include "mux/server/runtime_clock.h" // IWYU pragma: keep

/* Implements loading for BattleTech special objects. */

#include <stdlib.h>
#include <time.h>

#include "autopilot_weapon_profile_api.h"
#include "btech/context.h"
#include "btechstats_api.h"
#include "map_dynamic_api.h"
#include "mech_lifecycle.h"
#include "mech_restrict_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/doubly_linked_list.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

/*** #include all the prototype here! ****/
#include "autopilot.h"
#include "btech/persistence.h"
#include "mech_api_types.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_partnames_api.h"
#include "mech_runtime_api.h"
#include "mech_stat_api.h"
#include "mechrep_api.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_internal.h"
static int remove_from_all_maps_func(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  void *arg = call->context;
  BtechSpecialObject *const XCODE_OBJ = data;
  Mech *const MECH = arg;

  if (XCODE_OBJ->type == GTYPE_MAP) {
    BattleMap *map;
    int i;

    map = btech_context_get_map(mech_context(MECH), (DbRef)key);
    if (!map)
      return 1;
    for (i = 0; i < battle_map_unit_count(map); i++)
      if (battle_map_unit_dbref(map, i) == mech_dbref(MECH))
        battle_map_unit_slot_clear(map, i);
  }
  return 1;
}

void mech_remove_from_all_maps(Mech *mech) {
  red_black_tree_walk(mech_context(mech)->special_objects, WALK_INORDER,
                      remove_from_all_maps_func, mech);
}

typedef struct RemoveFromAllMapsContext {
  Mech *mech;
  DbRef except_map;
} RemoveFromAllMapsContext;

static int remove_from_all_maps_except_func(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  void *arg = call->context;
  DbRef key_val = (DbRef)key;
  BtechSpecialObject *const XCODE_OBJ = data;
  RemoveFromAllMapsContext *context = arg;
  Mech *const MECH = context->mech;

  if (XCODE_OBJ->type == GTYPE_MAP) {
    int i;
    BattleMap *map;

    if (key_val == context->except_map)
      return 1;
    map = btech_context_get_map(mech_context(MECH), key_val);
    if (!map)
      return 1;
    for (i = 0; i < battle_map_unit_count(map); i++)
      if (battle_map_unit_dbref(map, i) == mech_dbref(MECH))
        battle_map_unit_slot_clear(map, i);
  }
  return 1;
}

void mech_remove_from_all_maps_except(Mech *mech, DbRef num) {
  RemoveFromAllMapsContext context = {
      .mech = mech,
      .except_map = num,
  };

  red_black_tree_walk(mech_context(mech)->special_objects, WALK_INORDER,
                      remove_from_all_maps_except_func, &context);
}

static int load_update2(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  BtechSpecialObject *const XCODE_OBJ = data;

  if (XCODE_OBJ->type == GTYPE_MECH)
    mech_map_consistency_check((void *)XCODE_OBJ);
  return 1;
}

static int load_update4(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  void *arg = call->context;
  BtechSpecialObject *const XCODE_OBJ = data;
  BtechContext *const CONTEXT = arg;

  if (XCODE_OBJ->type == GTYPE_MECH) {
    Mech *const MECH = (Mech *)XCODE_OBJ;
    BattleMap *map;

    map = btech_context_get_map(CONTEXT, mech_map_dbref(MECH));
    if (!map) {
      /* Ugly kludge */
      map = btech_context_get_map(
          CONTEXT, game_object_location(CONTEXT->database, mech_dbref(MECH)));
      if (map)
        mech_rsetmapindex(
            GOD, MECH,
            tprintf("%ld",
                    game_object_location(CONTEXT->database, mech_dbref(MECH))));
      map = btech_context_get_map(CONTEXT, mech_map_dbref(MECH));
      if (!map)
        return 1;
    }

    if (!mech_is_started(MECH))
      return 1;
    mech_start_seeing(MECH);
    mech_update_recycling(MECH);
    mech_maybe_move(MECH);
  }
  return 1;
}

static int load_update3(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  BtechSpecialObject *const XCODE_OBJ = data;

  if (XCODE_OBJ->type == GTYPE_MAP) {
    eliminate_empties((BattleMap *)XCODE_OBJ);
    mine_fields_recalculate((BattleMap *)XCODE_OBJ);
  }
  return 1;
}

/*
 * Read in autopilot data
 */
static int load_autopilot_data(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  void *arg = call->context;
  BtechSpecialObject *const XCODE_OBJ = data;
  BtechContext *const CONTEXT = arg;

  if (XCODE_OBJ->type == GTYPE_AUTO) {
    Autopilot *const AUTOPILOT = (Autopilot *)XCODE_OBJ;

    /* Commands and A* paths are restored before these derived caches. */
    AUTOPILOT->weaplist = NULL;
    autopilot_weapon_profiles_initialize(AUTOPILOT);

    if (AUTOPILOT->mymechnum)
      AUTOPILOT->mymech = btech_context_get_mech(CONTEXT, AUTOPILOT->mymechnum);
    if (!AUTOPILOT->mymechnum || !AUTOPILOT->mymech) {
      autopilot_gunning_stop(AUTOPILOT);
    } else {
      /*
       * Weapon lists and range profiles are caches derived from the restored
       * MECH definition. Rebuild them instead of persisting cache trees.
       */
      auto_update_profile_event(AUTOPILOT);

      /*
       * MUX event nodes are runtime-only. An autopilot that was engaged at
       * checkpoint time is identified by the durable MECH->AUTO link and by
       * the AUTO object being inside that MECH. Requeue its dispatcher from
       * the durable command list; it recreates goal-specific events itself.
       */
      if (mech_autopilot_dbref(AUTOPILOT->mymech) == AUTOPILOT->mynum &&
          game_object_location(CONTEXT->database, AUTOPILOT->mynum) ==
              AUTOPILOT->mymechnum &&
          AUTOPILOT->commands &&
          doubly_linked_list_size(AUTOPILOT->commands) > 0 &&
          !mux_event_count_type_data(CONTEXT->events, EVENT_AUTOCOM, AUTOPILOT))
        autopilot_event_schedule(AUTOPILOT, EVENT_AUTOCOM, auto_com_event,
                                 AUTOPILOT_NC_DELAY, 0);
      if (autopilot_is_gunning(AUTOPILOT))
        autopilot_gunning_start(AUTOPILOT);
    }
  }

  return 1;
}

void btech_special_objects_load(BtechContext *context) {
  DbRef i;
  int special_type;
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
      if (btech_special_object_data_size(
              btech_special_object_definition(type)) > 0)
        new_special_object(context, i, type);
    } else
      c_xcode(context->database, i); /* Reset the flag */
  }
  for (special_type = 0; special_type < BTECH_SPECIAL_OBJECT_COUNT;
       special_type++) {
    init_special_hash(context, special_type);
  }
  init_btechstats(context);
  if (!character_state_validate_all(context)) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Invalid BTech character state in the game database");
    exit(EXIT_FAILURE);
  }
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

static int update_special_object_func(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  void *arg = call->context;
  BtechSpecialObject *const XCODE_OBJ = data;
  BtechContext *const CONTEXT = arg;

  const BtechSpecialObjectDefinition *definition =
      btech_special_object_definition((int)XCODE_OBJ->type);
  if (!definition->update_time)
    return 1;
  if ((CONTEXT->clock->now % definition->update_time))
    return 1;
  definition->update((DbRef)key, XCODE_OBJ);
  return 1;
}

/* This is called once a second for each special object */

/* Note the new handling for calls being done at <1second intervals,
   or possibly at >1second intervals */

void btech_special_objects_update(BtechContext *context) {
  const char *cmdsave;
  time_t elapsed;
  int i;
  int times;

  elapsed = context->last_special_update
                ? context->clock->now - context->last_special_update
                : 1;

  if (elapsed > 20)
    times = 20; /* Machine's hopelessly lagged,
                           we don't want to make it [much] worse */
  else if (elapsed < 0)
    times = 0;
  else
    times = (int)elapsed;
  cmdsave = btech_context_command(context)->debug_command;
  for (i = 0; i < times; i++) {
    mux_event_run(context->events);
    btech_context_command(context)->debug_command =
        "< Generic hcode update handler>";
    red_black_tree_walk(context->special_objects, WALK_INORDER,
                        update_special_object_func, context);
  }
  context->last_special_update = context->clock->now;
  btech_context_command(context)->debug_command = cmdsave;
}
