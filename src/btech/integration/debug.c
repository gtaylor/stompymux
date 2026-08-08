/*
 * Debug.c
 *
 *  File for debug of the hardcode routines.
 *
 * Serious knifing / new functions by Markus Stenberg <fingon@iki.fi>
 */

#include "debug_api.h"

#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "map_bits_api.h"
#include "map_dynamic_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_lifecycle.h"
#include "mech_partnames_api.h"
#include "mech_position_api.h"
#include "mech_startup_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/formatting.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_settings.h"

void debug_list(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc == 0)
    return;
  else if (args[0][0] == 'M' || args[0][0] == 'm')
    if (args[0][1] == 'E' || args[0][1] == 'e')
      DumpMechs(debug->context, player);
  if (args[0][1] == 'A' || args[0][1] == 'a')
    DumpMaps(debug->context, player);
}

void debug_savedb(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;

  if (gamedb_dump(debug->context->persistence, DUMP_NORMAL) < 0)
    mecha_notify(
        btech_context_evaluation(debug->context), player,
        "SQLite checkpoint failed; the previous snapshot remains available.");
  else
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "SQLite checkpoint complete.");
}

typedef struct DebugMemoryContext DebugMemoryContext;
struct DebugMemoryContext {
  int *number;
  size_t *smallest;
  size_t *largest;
  size_t *total;
  DbRef detail_player;
};
static int debug_check_stuff(void *key, void *data, int depth, void *arg) {
  const DbRef key_val = (DbRef)key;
  BtechSpecialObject *const xcode_obj = data;
  DebugMemoryContext *memory = arg;

  const int type = (int)xcode_obj->type;
  size_t size = btech_special_object_storage_size(type);
  BattleMap *map;

  switch (xcode_obj->type) {
  case GTYPE_MECH:
  case GTYPE_DEBUG:
  case GTYPE_MECHREP:
  case GTYPE_MAP:
    map = (BattleMap *)xcode_obj;
    if (map->map) {
      size += sizeof(map->map[0][0]) * (size_t)map->map_width *
              (size_t)map->map_height;
      size += (size_t)bit_size(map);
      size += (size_t)obj_size(map);
      size += mech_size(map);
    }
    break;

  case GTYPE_AUTO:
  case GTYPE_TURRET:
  case GTYPE_UNUSED1:
    break;
  }

  if (memory->number[type] == 0 || size < memory->smallest[type])
    memory->smallest[type] = size;
  if (memory->number[type] == 0 || size > memory->largest[type])
    memory->largest[type] = size;
  memory->total[type] += size;
  memory->number[type]++;

  if (memory->detail_player > 0)
    notify_printf(
        btech_context_evaluation(xcode_obj->context), memory->detail_player,
        "#%5ld: %10s %5ld", key_val, btech_special_object_type_name(type),
        xcode_obj->type == GTYPE_AUTO ? ((Autopilot *)xcode_obj)->mymechnum
                                      : 0);

  return 1;
}

void debug_memory(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  int i;
  size_t gtotal = 0;
  int type_count = btech_special_object_type_count();
  DebugMemoryContext memory = {0};

  Create(memory.number, int, (size_t)type_count);
  Create(memory.smallest, size_t, (size_t)type_count);
  Create(memory.largest, size_t, (size_t)type_count);
  Create(memory.total, size_t, (size_t)type_count);

  for (i = 0; i < type_count; i++) {
    memory.number[i] = 0;
    memory.smallest[i] = 0;
    memory.largest[i] = 0;
    memory.total[i] = 0;
  }
  const char *request = buffer;
  while (request && *request && isspace((unsigned char)*request))
    request++;
  if (!request)
    request = "";
  if (strcmp(request, ""))
    memory.detail_player = player;
  else
    memory.detail_player = -1;
  red_black_tree_walk(context->special_objects, WALK_INORDER, debug_check_stuff,
                      &memory);
  for (i = 0; i < type_count; i++) {
    if (memory.number[i]) {
      if (memory.smallest[i] == memory.largest[i])
        notify_printf(btech_context_evaluation(debug->context), player,
                      "%4d %-20s: %zu bytes total, %zu each", memory.number[i],
                      btech_special_object_type_name(i), memory.total[i],
                      memory.total[i] / (size_t)memory.number[i]);
      else
        notify_printf(
            btech_context_evaluation(debug->context), player,
            "%4d %-20s: %zu bytes total, %zu avg, %zu/%zu small/large",
            memory.number[i], btech_special_object_type_name(i),
            memory.total[i], memory.total[i] / (size_t)memory.number[i],
            memory.smallest[i], memory.largest[i]);
    }
    gtotal += memory.total[i];
  }
  notify_printf(btech_context_evaluation(debug->context), player,
                "Grand total: %zu bytes.", gtotal);
  free(memory.number);
  free(memory.total);
  free(memory.smallest);
  free(memory.largest);
}

void ShutDownMap(BtechContext *context, DbRef player, DbRef mapnumber) {
  BtechSpecialObject *xcode_obj;

  BattleMap *map;
  Mech *mech;
  int j;

  xcode_obj = red_black_tree_find(context->special_objects, (void *)mapnumber);
  if (xcode_obj) {
    map = (BattleMap *)xcode_obj;
    for (j = 0; j < map->first_free; j++)
      if (map->mechsOnMap[j] != -1) {
        mech = btech_context_get_mech(context, map->mechsOnMap[j]);
        if (mech) {
          notify_printf(
              btech_context_evaluation(context), player,
              "Shutting down Mech #%ld and resetting map index to -1....",
              map->mechsOnMap[j]);
          mech_shutdown(GOD, (void *)mech, "");
          mech_position_reset_origin(mech);
          remove_mech_from_map(map, mech);
        }
      }
    battle_map_dynamic_destroy(map);
    map->first_free = 0;
    mecha_notify(btech_context_evaluation(context), player, "Map Cleared");
    return;
  }
}

void debug_shutdown(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  long map_number;
  if (argc > 0 && parse_long_checked(args[0], &map_number)) {
    ShutDownMap(debug->context, player, map_number);
  } else {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid map number!");
  }
}

void debug_setvrt(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int vrt;
  int id, brand;

  if (mech_parseattributes(buffer, args, 3) != 2) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[1], &vrt)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid value!");
    return;
  }
  if (vrt <= 0) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "VRT needs to be >0");
    return;
  }
  if (vrt > 127) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "VRT can be at max 127");
    return;
  }
  if (!find_matching_vlong_part(debug->context, args[0], nullptr, &id,
                                &brand)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  if (!equipment_is_weapon(id)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  btech_weapon_settings_set_recycle_time(&debug->context->weapon_settings,
                                         weapon_from_equipment_index(id), vrt);
  notify_printf(btech_context_evaluation(debug->context), player,
                "VRT for %s set to %d.",
                MechWeapons[weapon_from_equipment_index(id)].name, vrt);
  log_error(debug->context->log, LOG_WIZARD, "WIZ", "CHANGE",
            "VRT for %s set to %d by #%ld",
            MechWeapons[weapon_from_equipment_index(id)].name, vrt, player);
}

void debug_setwbv(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int bv;
  int id, brand;

  if (mech_parseattributes(buffer, args, 3) != 2) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[1], &bv)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid value!");
    return;
  }
  if (bv < 0) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "BV needs to be >=0");
    return;
  }
  if (!find_matching_vlong_part(debug->context, args[0], nullptr, &id,
                                &brand)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  if (!equipment_is_weapon(id)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  btech_weapon_settings_set_battle_value(&debug->context->weapon_settings,
                                         weapon_from_equipment_index(id), bv);
  notify_printf(btech_context_evaluation(debug->context), player,
                "BV for %s set to %d.",
                MechWeapons[weapon_from_equipment_index(id)].name, bv);
}
