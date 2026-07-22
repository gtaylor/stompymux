/*
 * Debug.c
 *
 *  File for debug of the hardcode routines.
 *
 * Serious knifing / new functions by Markus Stenberg <fingon@iki.fi>
 */

#include "debug.h"
#include "autopilot.h"
#include "glue.h"
#include "mech.h"
#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/objects/db.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/red_black_tree.h"
#include "p.map.obj.h"
#include "p.mech.partnames.h"
#include "p.mech.startup.h"

void debug_list(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
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
  XCODE *debug = data;

  if (gamedb_dump(debug->context->persistence, DUMP_NORMAL) < 0)
    notify(
        btech_context_evaluation(debug->context), player,
        "SQLite checkpoint failed; the previous snapshot remains available.");
  else
    notify(btech_context_evaluation(debug->context), player,
           "SQLite checkpoint complete.");
}

typedef struct DebugMemoryContext DebugMemoryContext;
struct DebugMemoryContext {
  int *number;
  int *smallest;
  int *largest;
  int *total;
  DbRef detail_player;
};
extern const int global_specials;
extern const SpecialObjectStruct SpecialObjects[];

static int debug_check_stuff(void *key, void *data, int depth, void *arg) {
  const DbRef key_val = (DbRef)key;
  XCODE *const xcode_obj = data;
  DebugMemoryContext *memory = arg;

  int size;
  MAP *map;

  size = SpecialObjects[xcode_obj->type].datasize;

  switch (xcode_obj->type) {
  case GTYPE_MAP:
    map = (MAP *)xcode_obj;
    if (map->map) {
      size += sizeof(map->map[0][0]) * map->map_width * map->map_height;
      size += bit_size(map);
      size += obj_size(map);
      size += mech_size(map);
    }
    break;

  default:
    break;
  }

  if (memory->smallest[xcode_obj->type] < 0 ||
      size < memory->smallest[xcode_obj->type])
    memory->smallest[xcode_obj->type] = size;
  if (memory->largest[xcode_obj->type] < 0 ||
      size > memory->largest[xcode_obj->type])
    memory->largest[xcode_obj->type] = size;
  memory->total[xcode_obj->type] += size;
  memory->number[xcode_obj->type]++;

  if (memory->detail_player > 0)
    notify_printf(
        btech_context_evaluation(xcode_obj->context), memory->detail_player,
        "#%5ld: %10s %5ld", key_val, SpecialObjects[xcode_obj->type].type,
        xcode_obj->type == GTYPE_AUTO ? ((AUTO *)xcode_obj)->mymechnum : 0);

  return 1;
}

void debug_memory(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  BtechContext *context = debug->context;
  int i, gtotal = 0;
  DebugMemoryContext memory = {0};

  Create(memory.number, int, global_specials);
  Create(memory.smallest, int, global_specials);
  Create(memory.largest, int, global_specials);
  Create(memory.total, int, global_specials);

  for (i = 0; i < global_specials; i++) {
    memory.number[i] = 0;
    memory.smallest[i] = -1;
    memory.largest[i] = -1;
    memory.total[i] = 0;
  }
  skipws(buffer);
  if (strcmp(buffer, ""))
    memory.detail_player = player;
  else
    memory.detail_player = -1;
  red_black_tree_walk(context->special_objects, WALK_INORDER, debug_check_stuff,
                      &memory);
  for (i = 0; i < global_specials; i++) {
    if (memory.number[i]) {
      if (memory.smallest[i] == memory.largest[i])
        notify_printf(btech_context_evaluation(debug->context), player,
                      "%4d %-20s: %d bytes total, %d each", memory.number[i],
                      SpecialObjects[i].type, memory.total[i],
                      memory.total[i] / memory.number[i]);
      else
        notify_printf(btech_context_evaluation(debug->context), player,
                      "%4d %-20s: %d bytes total, %d avg, %d/%d small/large",
                      memory.number[i], SpecialObjects[i].type, memory.total[i],
                      memory.total[i] / memory.number[i], memory.smallest[i],
                      memory.largest[i]);
    }
    gtotal += memory.total[i];
  }
  notify_printf(btech_context_evaluation(debug->context), player,
                "Grand total: %d bytes.", gtotal);
  free(memory.number);
  free(memory.total);
  free(memory.smallest);
  free(memory.largest);
}

void ShutDownMap(BtechContext *context, DbRef player, DbRef mapnumber) {
  XCODE *xcode_obj;

  MAP *map;
  MECH *mech;
  int j;

  xcode_obj = red_black_tree_find(context->special_objects, (void *)mapnumber);
  if (xcode_obj) {
    map = (MAP *)xcode_obj;
    for (j = 0; j < map->first_free; j++)
      if (map->mechsOnMap[j] != -1) {
        mech = btech_context_get_mech(context, map->mechsOnMap[j]);
        if (mech) {
          notify_printf(
              btech_context_evaluation(context), player,
              "Shutting down Mech #%ld and resetting map index to -1....",
              map->mechsOnMap[j]);
          mech_shutdown(GOD, (void *)mech, "");
          MechLastX(mech) = 0;
          MechLastY(mech) = 0;
          MechX(mech) = 0;
          MechY(mech) = 0;
          remove_mech_from_map(map, mech);
        }
      }
    map->first_free = 0;
    notify(btech_context_evaluation(context), player, "Map Cleared");
    return;
  }
}

void debug_shutdown(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc > 0)
    ShutDownMap(debug->context, player, atoi(args[0]));
}

void debug_setvrt(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  char *args[3];
  int vrt;
  int id, brand;

  DOCHECK_CONTEXT(debug->context, mech_parseattributes(buffer, args, 3) != 2,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(debug->context, Readnum(vrt, args[1]), "Invalid value!");
  DOCHECK_CONTEXT(debug->context, vrt <= 0, "VRT needs to be >0");
  DOCHECK_CONTEXT(debug->context, vrt > 127, "VRT can be at max 127");
  DOCHECK_CONTEXT(
      debug->context,
      !find_matching_vlong_part(debug->context, args[0], nullptr, &id, &brand),
      "That is no weapon!");
  DOCHECK_CONTEXT(debug->context, !IsWeapon(id), "That is no weapon!");
  btech_weapon_settings_set_recycle_time(&debug->context->weapon_settings,
                                         Weapon2I(id), vrt);
  notify_printf(btech_context_evaluation(debug->context), player,
                "VRT for %s set to %d.", MechWeapons[Weapon2I(id)].name, vrt);
  log_error(debug->context->log, LOG_WIZARD, "WIZ", "CHANGE",
            "VRT for %s set to %d by #%ld", MechWeapons[Weapon2I(id)].name, vrt,
            player);
}

void debug_setwbv(DbRef player, void *data, char *buffer) {
  XCODE *debug = data;
  char *args[3];
  int bv;
  int id, brand;

  DOCHECK_CONTEXT(debug->context, mech_parseattributes(buffer, args, 3) != 2,
                  "Invalid arguments!");
  DOCHECK_CONTEXT(debug->context, Readnum(bv, args[1]), "Invalid value!");
  DOCHECK_CONTEXT(debug->context, bv < 0, "BV needs to be >=0");
  DOCHECK_CONTEXT(
      debug->context,
      !find_matching_vlong_part(debug->context, args[0], nullptr, &id, &brand),
      "That is no weapon!");
  DOCHECK_CONTEXT(debug->context, !IsWeapon(id), "That is no weapon!");
  btech_weapon_settings_set_battle_value(&debug->context->weapon_settings,
                                         Weapon2I(id), bv);
  notify_printf(btech_context_evaluation(debug->context), player,
                "BV for %s set to %d.", MechWeapons[Weapon2I(id)].name, bv);
}
