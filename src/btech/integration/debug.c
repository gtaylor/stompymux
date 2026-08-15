/* Implements debug support for BattleTech hardcode routines. */

#include "debug_api.h"

#include <stdlib.h>
#include <string.h>

#include "autopilot.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
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
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "special_object.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

static char *debug_argument(char **arguments, size_t count, size_t index) {
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), index);
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
  const size_t ARGUMENT_LENGTH = strlen(first_argument);
  const char FIRST_CHARACTER =
      ARGUMENT_LENGTH > 0 ? *checked_string_suffix(first_argument, 0) : '\0';
  const char SECOND_CHARACTER =
      ARGUMENT_LENGTH > 1 ? *checked_string_suffix(first_argument, 1) : '\0';
  if (FIRST_CHARACTER == 'M' || FIRST_CHARACTER == 'm')
    if (SECOND_CHARACTER == 'E' || SECOND_CHARACTER == 'e')
      dump_mechs(debug->context, player);
  if (SECOND_CHARACTER == 'A' || SECOND_CHARACTER == 'a')
    dump_maps(debug->context, player);
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
static int debug_check_stuff(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  void *arg = call->context;
  const DbRef KEY_VAL = (DbRef)key;
  BtechSpecialObject *const XCODE_OBJ = data;
  DebugMemoryContext *memory = arg;

  const int TYPE = (int)XCODE_OBJ->type;
  size_t size = btech_special_object_storage_size(TYPE);
  BattleMap *map;

  switch (XCODE_OBJ->type) {
  case GTYPE_MECH:
  case GTYPE_DEBUG:
  case GTYPE_MECHREP:
  case GTYPE_MAP:
    map = (BattleMap *)XCODE_OBJ;
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

  DebugMemoryStat *stat = debug_memory_stat(memory, TYPE);
  if (stat->number == 0 || size < stat->smallest)
    stat->smallest = size;
  if (stat->number == 0 || size > stat->largest)
    stat->largest = size;
  stat->total += size;
  stat->number++;

  if (memory->detail_player > 0) {
    notify_printf(
        btech_context_evaluation(XCODE_OBJ->context), memory->detail_player,
        "#%5ld: %10s %5ld", KEY_VAL, btech_special_object_type_name(TYPE),
        XCODE_OBJ->type == GTYPE_AUTO ? ((Autopilot *)XCODE_OBJ)->mymechnum
                                      : 0);
  }

  return 1;
}

void debug_memory(DbRef player, void *data, const char *buffer) {
  BtechSpecialObject *debug = data;
  BtechContext *context = debug->context;
  size_t gtotal = 0;
  int type_count = btech_special_object_type_count();
  DebugMemoryContext memory = {};

  memory.stat_count = (size_t)type_count;
  memory.stats = checked_storage_try_allocate_array(memory.stat_count,
                                                    sizeof(*memory.stats));
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
      if (stat->smallest == stat->largest) {
        notify_printf(btech_context_evaluation(debug->context), player,
                      "%4d %-20s: %zu bytes total, %zu each", stat->number,
                      btech_special_object_type_name(i), stat->total,
                      stat->total / (size_t)stat->number);
      } else {
        notify_printf(
            btech_context_evaluation(debug->context), player,
            "%4d %-20s: %zu bytes total, %zu avg, %zu/%zu small/large",
            stat->number, btech_special_object_type_name(i), stat->total,
            stat->total / (size_t)stat->number, stat->smallest, stat->largest);
      }
    }
    gtotal += stat->total;
  }
  notify_printf(btech_context_evaluation(debug->context), player,
                "Grand total: %zu bytes.", gtotal);
  free(memory.stats);
}

void map_shutdown_units(const MapShutdownRequest *request) {
  BtechContext *context = request->context;
  BtechSpecialObject *xcode_obj;

  BattleMap *map;
  Mech *mech;
  int j;

  xcode_obj =
      red_black_tree_find(context->special_objects, (void *)request->map);
  if (xcode_obj) {
    map = (BattleMap *)xcode_obj;
    for (j = 0; j < battle_map_unit_count(map); j++) {
      const DbRef UNIT_DBREF = battle_map_unit_dbref(map, j);
      if (UNIT_DBREF != -1) {
        mech = btech_context_get_mech(context, UNIT_DBREF);
        if (mech) {
          notify_printf(
              btech_context_evaluation(context), request->actor,
              "Shutting down Mech #%ld and resetting map index to -1....",
              UNIT_DBREF);
          mech_shutdown(GOD, (void *)mech, "");
          mech_position_reset_origin(mech);
          remove_mech_from_map(map, mech);
        }
      }
    }
    battle_map_dynamic_destroy(map);
    map->first_free = 0;
    mecha_notify(btech_context_evaluation(context), request->actor,
                 "Map Cleared");
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
    map_shutdown_units(&(MapShutdownRequest){
        .context = debug->context, .actor = player, .map = map_number});
  } else {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "Invalid map number!");
  }
}

void debug_setvrt(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int vrt;
  int id;

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
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = debug->context,
                                          .pattern = debug_argument(args, 3, 0),
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (!match.found) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  id = match.part.id;
  if (!equipment_is_weapon(id)) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  btech_weapon_settings_set_recycle_time(&debug->context->weapon_settings,
                                         weapon_from_equipment_index(id), vrt);
  const int WEAPON_INDEX = weapon_from_equipment_index(id);
  notify_printf(btech_context_evaluation(debug->context), player,
                "VRT for %s set to %d.", weapon_catalogue_name(WEAPON_INDEX),
                vrt);
  log_error((LogEntry){.log = debug->context->log,
                       .key = LOG_WIZARD,
                       .primary = "WIZ",
                       .secondary = "CHANGE"},
            "VRT for %s set to %d by #%ld", weapon_catalogue_name(WEAPON_INDEX),
            vrt, player);
}

void debug_setwbv(DbRef player, void *data, char *buffer) {
  BtechSpecialObject *debug = data;
  char *args[3];
  int bv;
  int id;

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
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = debug->context,
                                          .pattern = debug_argument(args, 3, 0),
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (!match.found) {
    mecha_notify(btech_context_evaluation(debug->context), player,
                 "That is no weapon!");
    return;
  }
  id = match.part.id;
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
