/* Implements debug support for BattleTech hardcode routines. */

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
#include "map_units_api.h"
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
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

static char *debug_argument(char **arguments, size_t count, size_t index) {
  char **slot = checked_storage_at(arguments, count, sizeof(*arguments), index);
  return *slot;
}

void debug_list(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc == 0)
    return;
  char *first_argument = debug_argument(args, 3, 0);
  const size_t argument_length = strlen(first_argument);
  const char first_character =
      argument_length > 0 ? *checked_string_suffix(first_argument, 0) : '\0';
  const char second_character =
      argument_length > 1 ? *checked_string_suffix(first_argument, 1) : '\0';
  if (first_character == 'M' || first_character == 'm')
    if (second_character == 'E' || second_character == 'e')
      DumpMechs(debug->context, player);
  if (second_character == 'A' || second_character == 'a')
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

typedef struct DebugMemoryStat {
  int number;
  size_t smallest;
  size_t largest;
  size_t total;
} DebugMemoryStat;

typedef struct DebugMemoryContext DebugMemoryContext;
struct DebugMemoryContext {
  DebugMemoryStat *stats;
  size_t stat_count;
  DbRef detail_player;
};

static DebugMemoryStat *debug_memory_stat(DebugMemoryContext *memory,
                                          int type) {
  if (type < 0)
    abort();
  return checked_storage_at(memory->stats, memory->stat_count,
                            sizeof(*memory->stats), (size_t)type);
}
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
      size += sizeof(unsigned char) * (size_t)map->map_width *
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

  DebugMemoryStat *stat = debug_memory_stat(memory, type);
  if (stat->number == 0 || size < stat->smallest)
    stat->smallest = size;
  if (stat->number == 0 || size > stat->largest)
    stat->largest = size;
  stat->total += size;
  stat->number++;

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
  size_t gtotal = 0;
  int type_count = btech_special_object_type_count();
  DebugMemoryContext memory = {0};

  memory.stat_count = (size_t)type_count;
  memory.stats = calloc(memory.stat_count, sizeof(*memory.stats));
  if (memory.stats == nullptr)
    abort();
  const char *request = buffer;
  if (request != nullptr)
    request =
        checked_storage_at_const(request, strlen(request) + 1, sizeof(*request),
                                 strspn(request, " \t\r\n\f\v"));
  if (!request)
    request = "";
  if (strcmp(request, ""))
    memory.detail_player = player;
  else
    memory.detail_player = -1;
  red_black_tree_walk(context->special_objects, WALK_INORDER, debug_check_stuff,
                      &memory);
  for (int i = 0; i < type_count; i++) {
    DebugMemoryStat *stat = debug_memory_stat(&memory, i);
    if (stat->number) {
      if (stat->smallest == stat->largest)
        notify_printf(btech_context_evaluation(debug->context), player,
                      "%4d %-20s: %zu bytes total, %zu each", stat->number,
                      btech_special_object_type_name(i), stat->total,
                      stat->total / (size_t)stat->number);
      else
        notify_printf(
            btech_context_evaluation(debug->context), player,
            "%4d %-20s: %zu bytes total, %zu avg, %zu/%zu small/large",
            stat->number, btech_special_object_type_name(i), stat->total,
            stat->total / (size_t)stat->number, stat->smallest, stat->largest);
    }
    gtotal += stat->total;
  }
  notify_printf(btech_context_evaluation(debug->context), player,
                "Grand total: %zu bytes.", gtotal);
  free(memory.stats);
}

void ShutDownMap(BtechContext *context, DbRef player, DbRef mapnumber) {
  BtechSpecialObject *xcode_obj;

  BattleMap *map;
  Mech *mech;
  int j;

  xcode_obj = red_black_tree_find(context->special_objects, (void *)mapnumber);
  if (xcode_obj) {
    map = (BattleMap *)xcode_obj;
    for (j = 0; j < battle_map_unit_count(map); j++) {
      const DbRef unit_dbref = battle_map_unit_dbref(map, j);
      if (unit_dbref != -1) {
        mech = btech_context_get_mech(context, unit_dbref);
        if (mech) {
          notify_printf(
              btech_context_evaluation(context), player,
              "Shutting down Mech #%ld and resetting map index to -1....",
              unit_dbref);
          mech_shutdown(GOD, (void *)mech, "");
          mech_position_reset_origin(mech);
          remove_mech_from_map(map, mech);
        }
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
  if (argc > 0 && parse_long_checked(debug_argument(args, 3, 0), &map_number)) {
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
  if (!parse_int_checked(debug_argument(args, 3, 1), &vrt)) {
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
  if (!find_matching_vlong_part(debug->context, debug_argument(args, 3, 0),
                                nullptr, &id, &brand)) {
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
  const int weapon_index = weapon_from_equipment_index(id);
  notify_printf(btech_context_evaluation(debug->context), player,
                "VRT for %s set to %d.", weapon_catalogue_name(weapon_index),
                vrt);
  log_error(debug->context->log, LOG_WIZARD, "WIZ", "CHANGE",
            "VRT for %s set to %d by #%ld", weapon_catalogue_name(weapon_index),
            vrt, player);
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
  if (!parse_int_checked(debug_argument(args, 3, 1), &bv)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid value!");
    return;
  }
  if (bv < 0) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "BV needs to be >=0");
    return;
  }
  if (!find_matching_vlong_part(debug->context, debug_argument(args, 3, 0),
                                nullptr, &id, &brand)) {
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
                weapon_catalogue_name(weapon_from_equipment_index(id)), bv);
}
