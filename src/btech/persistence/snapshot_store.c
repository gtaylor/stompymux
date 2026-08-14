#include "autopilot.h"
#include "btech/context.h"
#include "btech_event.h"
#include "context_internal.h" // IWYU pragma: keep
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mux/network/mux_event.h"
#include "mux/server/platform.h"
#include "mux/support/red_black_tree.h"
#include "special_object.h"
#include "sqlite_internal.h"

#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_identity_api.h"
#include "mux/support/checked_storage.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static unsigned char *const *stored_bits_row(unsigned char **bits, int height,
                                             int row) {
  if (row < 0)
    abort();
  return (unsigned char *const *)checked_storage_at_const(
      (const void *)bits, (size_t)height, sizeof(*bits), (size_t)row);
}

static unsigned char stored_bits_byte(const unsigned char *row, int count,
                                      int index) {
  if (index < 0)
    abort();
  const unsigned char *value =
      checked_storage_at_const(row, (size_t)count, sizeof(*row), (size_t)index);
  return *value;
}

int btech_store_map(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  int depth = call->depth;
  void *argument = call->context;
  BtechMapStoreContext *context = argument;
  BtechSpecialObject *xcode = data;
  BattleMap *map;
  int index;
  int target;
  int object_type;
  int ordinal;
  int byte_index;
  int bytes_per_row;
  int x;
  int y;
  MapObject *object;
  unsigned char **bits;

  (void)depth;
  if (context->result < 0 || xcode->type != GTYPE_MAP)
    return context->result == 0;
  map = (BattleMap *)xcode;
  if (btech_special_bind_int(context->map, 1, (DbRef)key) < 0 ||
      sqlite3_bind_text(context->map, 2, map->mapname, -1, SQLITE_TRANSIENT) !=
          SQLITE_OK ||
      btech_special_bind_int(context->map, 3, map->map_width) < 0 ||
      btech_special_bind_int(context->map, 4, map->map_height) < 0 ||
      btech_special_bind_int(context->map, 5, map->temp) < 0 ||
      btech_special_bind_int(context->map, 6, map->grav) < 0 ||
      btech_special_bind_int(context->map, 7, map->cloudbase) < 0 ||
      btech_special_bind_int(context->map, 8, map->mapvis) < 0 ||
      btech_special_bind_int(context->map, 9, map->maxvis) < 0 ||
      btech_special_bind_int(context->map, 10, map->maplight) < 0 ||
      btech_special_bind_int(context->map, 11, map->winddir) < 0 ||
      btech_special_bind_int(context->map, 12, map->windspeed) < 0 ||
      btech_special_bind_int(context->map, 13, map->unused_char) < 0 ||
      btech_special_bind_int(context->map, 14, map->flags) < 0 ||
      btech_special_bind_int(context->map, 15, map->cf) < 0 ||
      btech_special_bind_int(context->map, 16, map->cfmax) < 0 ||
      btech_special_bind_int(context->map, 17, map->onmap) < 0 ||
      btech_special_bind_int(context->map, 18, map->buildflag) < 0 ||
      btech_special_bind_int(context->map, 19, map->first_free) < 0 ||
      btech_special_bind_int(context->map, 20, map->moves) < 0 ||
      btech_special_bind_int(context->map, 21, map->movemod) < 0 ||
      btech_special_bind_int(context->map, 22, map->sensorflags) < 0 ||
      btech_special_bind_int(context->map, 23, map->regen_factor) < 0 ||
      btech_special_write_step(context->fault, context->map) < 0) {
    context->result = -1;
    return 0;
  }
  if (!map->map) {
    context->result = -1;
    return 0;
  }
  for (y = 0; context->result == 0 && y < map->map_height; y++) {
    for (x = 0; context->result == 0 && x < map->map_width; x++) {
      if (btech_special_bind_int(context->hex, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->hex, 2, x) < 0 ||
          btech_special_bind_int(context->hex, 3, y) < 0 ||
          btech_special_bind_int(context->hex, 4,
                                 battle_map_encoded_hex(map, x, y)) < 0 ||
          btech_special_write_step(context->fault, context->hex) < 0)
        context->result = -1;
    }
  }
  for (index = 0; context->result == 0 && index < map->first_free; index++) {
    if (btech_special_bind_int(context->slot, 1, (DbRef)key) < 0 ||
        btech_special_bind_int(context->slot, 2, index) < 0 ||
        btech_special_bind_int(context->slot, 3,
                               battle_map_unit_dbref(map, index)) < 0 ||
        btech_special_bind_int(context->slot, 4,
                               battle_map_unit_flags(map, index)) < 0 ||
        btech_special_write_step(context->fault, context->slot) < 0) {
      context->result = -1;
      break;
    }
    for (target = 0; context->result == 0 && target < map->first_free;
         target++) {
      if (btech_special_bind_int(context->los, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->los, 2, index) < 0 ||
          btech_special_bind_int(context->los, 3, target) < 0 ||
          btech_special_bind_int(
              context->los, 4, battle_map_los_flags(map, index, target)) < 0 ||
          btech_special_write_step(context->fault, context->los) < 0)
        context->result = -1;
    }
  }
  for (object_type = 0; context->result == 0 && object_type < NUM_MAPOBJTYPES;
       object_type++) {
    if (object_type == TYPE_BITS)
      continue;
    ordinal = 0;
    for (object = first_mapobj(map, object_type);
         context->result == 0 && object; object = object->next, ordinal++) {
      if (btech_special_bind_int(context->object, 1, (DbRef)key) < 0 ||
          btech_special_bind_int(context->object, 2, object_type) < 0 ||
          btech_special_bind_int(context->object, 3, ordinal) < 0 ||
          btech_special_bind_int(context->object, 4, object->x) < 0 ||
          btech_special_bind_int(context->object, 5, object->y) < 0 ||
          btech_special_bind_int(context->object, 6, object->obj) < 0 ||
          btech_special_bind_int(context->object, 7, object->datac) < 0 ||
          btech_special_bind_int(context->object, 8, object->datas) < 0 ||
          btech_special_bind_int(context->object, 9, object->payload.scalar) <
              0 ||
          btech_special_write_step(context->fault, context->object) < 0)
        context->result = -1;
    }
  }
  MapObject *bits_object = first_mapobj(map, TYPE_BITS);
  if (context->result == 0 && bits_object) {
    bits = bits_object->payload.bits;
    bytes_per_row = (map->map_width / 4) + (map->map_width % 4 ? 1 : 0);
    for (index = 0; context->result == 0 && index < map->map_height; index++) {
      unsigned char *const *row = stored_bits_row(bits, map->map_height, index);
      if (!*row)
        continue;
      for (byte_index = 0; byte_index < bytes_per_row; byte_index++) {
        if (btech_special_bind_int(context->bits, 1, (DbRef)key) < 0 ||
            btech_special_bind_int(context->bits, 2, index) < 0 ||
            btech_special_bind_int(context->bits, 3, byte_index) < 0 ||
            btech_special_bind_int(
                context->bits, 4,
                stored_bits_byte(*row, bytes_per_row, byte_index)) < 0 ||
            btech_special_write_step(context->fault, context->bits) < 0)
          context->result = -1;
      }
    }
  }
  return context->result == 0;
}

/* Capture one queued repair event in its durable SQLite representation. */
typedef struct BtechRepairStoreContext {
  BtechSpecialWriteContext *fault;
  sqlite3_stmt *statement;
  int type;
  int result;
} BtechRepairStoreContext;

static void btech_store_repair_event(MuxEvent *event, void *context_argument) {
  BtechRepairStoreContext *context = context_argument;
  Mech *mech = event->data;
  long remaining = event->tick - event->scheduler->tick;

  if (context->result < 0 || !mech)
    return;
  if (remaining < 1)
    remaining = 1;
  if (event->function == mech_event_failure_marker)
    remaining = -remaining;
  if (btech_special_bind_int(context->statement, 1, mech_dbref(mech)) < 0 ||
      btech_special_bind_int(context->statement, 2, context->type) < 0 ||
      btech_special_bind_int(context->statement, 3,
                             remaining < 0 ? -remaining : remaining) < 0 ||
      btech_special_bind_int(context->statement, 4, (long)event->data2) < 0 ||
      btech_special_bind_int(context->statement, 5, remaining < 0) < 0 ||
      btech_special_write_step(context->fault, context->statement) < 0)
    context->result = -1;
}

/* Store map dynamic state and repair queues in the SQLite snapshot. */
int btech_persistence_store_special_state(sqlite3 *sqlite,
                                          PersistenceContext *persistence,
                                          void *extension_context) {
  BtechContext *btech = extension_context;
  (void)persistence;
  BtechSpecialWriteContext fault;
  BtechMapStoreContext maps = {.result = -1};
  BtechObjectStoreContext objects;
  sqlite3_stmt *repairs = NULL;
  BtechRepairStoreContext repair_context;
  int type;
  int result;

  btech_special_write_context_init(&fault);
  maps.fault = &fault;
  memset(&objects, 0, sizeof(objects));
  objects.fault = &fault;
  objects.result = -1;
  if (btech_special_exec(sqlite, BTECH_SPECIAL_SCHEMA_SQL) < 0)
    return -1;
  if (btech_special_store_metadata(&fault, sqlite) < 0)
    return -1;
  if (!btech->special_objects)
    return 0;
  if (btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_maps VALUES (?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &maps.map, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_map_hexes VALUES (?, ?, ?, ?);",
          -1, &maps.hex, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_map_slots VALUES (?, ?, ?, ?);",
          -1, &maps.slot, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_map_los VALUES (?, ?, ?, ?);", -1,
          &maps.los, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_map_objects VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &maps.object, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_map_bits VALUES (?, ?, ?, ?);", -1,
          &maps.bits, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (?, ?, ?, ?, ?);",
          -1, &repairs, NULL) != SQLITE_OK) {
    sqlite3_finalize(maps.map);
    sqlite3_finalize(maps.hex);
    sqlite3_finalize(maps.slot);
    sqlite3_finalize(maps.los);
    sqlite3_finalize(maps.object);
    sqlite3_finalize(maps.bits);
    sqlite3_finalize(repairs);
    return -1;
  }
  if (btech_special_write_prepare(&fault, sqlite,
                                  "INSERT INTO btech_mechrep VALUES (?, ?);",
                                  -1, &objects.mechrep, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_turrets VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);", -1,
          &objects.turret, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_turret_tics VALUES (?, ?, ?);", -1,
          &objects.turret_tic, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_autopilots VALUES (?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.autopilot, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mechs VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.mech, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_sections VALUES (?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.section, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_criticals VALUES (?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?);",
          -1, &objects.critical, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_positions VALUES (?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.position, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_mech_bays VALUES (?, ?, ?);", -1,
          &objects.bay, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_mech_turrets VALUES (?, ?, ?);",
          -1, &objects.mech_turret, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_c3 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);", -1,
          &objects.c3, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_c3_nodes VALUES (?, ?, ?, ?);", -1,
          &objects.c3node, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_mech_tics VALUES (?, ?, ?, ?);",
          -1, &objects.tic, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_frequencies VALUES (?, ?, ?, ?, ?);", -1,
          &objects.frequency, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_runtime VALUES ("
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &objects.runtime, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite, "INSERT INTO btech_mech_unit_aux VALUES (?, ?, ?);",
          -1, &objects.unit_aux, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_mech_stagger_damage VALUES (?, ?, ?, ?, ?, ?);",
          -1, &objects.stagger_damage, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_autopilot_commands VALUES (?, ?, ?, ?);", -1,
          &objects.autopilot_command, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_autopilot_command_args VALUES (?, ?, ?, ?);", -1,
          &objects.autopilot_command_arg, NULL) != SQLITE_OK ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_autopilot_path VALUES (?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?);",
          -1, &objects.autopilot_path, NULL) != SQLITE_OK) {
    sqlite3_finalize(maps.map);
    sqlite3_finalize(maps.hex);
    sqlite3_finalize(maps.slot);
    sqlite3_finalize(maps.los);
    sqlite3_finalize(maps.object);
    sqlite3_finalize(maps.bits);
    sqlite3_finalize(repairs);
    btech_finalize_object_statements(&objects);
    return -1;
  }
  maps.result = 0;
  red_black_tree_walk(btech->special_objects, WALK_INORDER, btech_store_map,
                      &maps);
  objects.result = 0;
  red_black_tree_walk(btech->special_objects, WALK_INORDER,
                      btech_store_simple_object, &objects);
  repair_context = (BtechRepairStoreContext){
      .fault = &fault,
      .statement = repairs,
      .result = 0,
  };
  for (type = FIRST_TECH_EVENT;
       type <= LAST_TECH_EVENT && repair_context.result == 0; type++) {
    repair_context.type = type;
    mux_event_visit_type(btech->events, type, btech_store_repair_event,
                         &repair_context);
  }
  result = maps.result < 0 || objects.result < 0 || repair_context.result < 0
               ? -1
               : 0;
  sqlite3_finalize(maps.map);
  sqlite3_finalize(maps.hex);
  sqlite3_finalize(maps.slot);
  sqlite3_finalize(maps.los);
  sqlite3_finalize(maps.object);
  sqlite3_finalize(maps.bits);
  sqlite3_finalize(repairs);
  btech_finalize_object_statements(&objects);
  return result;
}
