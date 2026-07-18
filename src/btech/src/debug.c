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
#include "mux/database/db.h"
#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/support/red_black_tree.h"
#include "p.map.obj.h"
#include "p.mech.partnames.h"
#include "p.mech.startup.h"

void debug_list(DbRef player, void *data, char *buffer) {
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc == 0)
    return;
  else if (args[0][0] == 'M' || args[0][0] == 'm')
    if (args[0][1] == 'E' || args[0][1] == 'e')
      DumpMechs(player);
  if (args[0][1] == 'A' || args[0][1] == 'a')
    DumpMaps(player);
}

void debug_savedb(DbRef player, void *data, char *buffer) {
  if (gamedb_dump(btech_context_active()->persistence, DUMP_NORMAL) < 0)
    notify(
        BTECH_EVALUATION_CONTEXT, player,
        "SQLite checkpoint failed; the previous snapshot remains available.");
  else
    notify(BTECH_EVALUATION_CONTEXT, player, "SQLite checkpoint complete.");
}

static int *number;
static int *smallest;
static int *largest;
static int *total;
static DbRef cheat_player;
extern RedBlackTree xcode_tree;
extern int global_specials;
extern SpecialObjectStruct SpecialObjects[];

static int debug_check_stuff(void *key, void *data, int depth, void *arg) {
  const DbRef key_val = (DbRef)key;
  XCODE *const xcode_obj = data;

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

  if (smallest[xcode_obj->type] < 0 || size < smallest[xcode_obj->type])
    smallest[xcode_obj->type] = size;
  if (largest[xcode_obj->type] < 0 || size > largest[xcode_obj->type])
    largest[xcode_obj->type] = size;
  total[xcode_obj->type] += size;
  number[xcode_obj->type]++;

  if (cheat_player > 0)
    notify_printf(BTECH_EVALUATION_CONTEXT, cheat_player, "#%5ld: %10s %5ld",
                  key_val, SpecialObjects[xcode_obj->type].type,
                  xcode_obj->type == GTYPE_AUTO ? ((AUTO *)xcode_obj)->mymechnum
                                                : 0);

  return 1;
}

void debug_memory(DbRef player, void *data, char *buffer) {
  int i, gtotal = 0;

  Create(number, int, global_specials);
  Create(smallest, int, global_specials);
  Create(largest, int, global_specials);
  Create(total, int, global_specials);

  for (i = 0; i < global_specials; i++) {
    number[i] = 0;
    smallest[i] = -1;
    largest[i] = -1;
    total[i] = 0;
  }
  cheat_player = player;
  skipws(buffer);
  if (strcmp(buffer, ""))
    cheat_player = player;
  else
    cheat_player = -1;
  red_black_tree_walk(xcode_tree, WALK_INORDER, debug_check_stuff, NULL);
  for (i = 0; i < global_specials; i++) {
    if (number[i]) {
      if (smallest[i] == largest[i])
        notify_printf(BTECH_EVALUATION_CONTEXT, player,
                      "%4d %-20s: %d bytes total, %d each", number[i],
                      SpecialObjects[i].type, total[i], total[i] / number[i]);
      else
        notify_printf(BTECH_EVALUATION_CONTEXT, player,
                      "%4d %-20s: %d bytes total, %d avg, %d/%d small/large",
                      number[i], SpecialObjects[i].type, total[i],
                      total[i] / number[i], smallest[i], largest[i]);
    }
    gtotal += total[i];
  }
  notify_printf(BTECH_EVALUATION_CONTEXT, player, "Grand total: %d bytes.",
                gtotal);
  free((void *)number);
  free((void *)total);
  free((void *)smallest);
  free((void *)largest);
}

void ShutDownMap(DbRef player, DbRef mapnumber) {
  XCODE *xcode_obj;

  MAP *map;
  MECH *mech;
  int j;

  xcode_obj = red_black_tree_find(xcode_tree, (void *)mapnumber);
  if (xcode_obj) {
    map = (MAP *)xcode_obj;
    for (j = 0; j < map->first_free; j++)
      if (map->mechsOnMap[j] != -1) {
        mech = getMech(map->mechsOnMap[j]);
        if (mech) {
          notify_printf(
              BTECH_EVALUATION_CONTEXT, player,
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
    notify(BTECH_EVALUATION_CONTEXT, player, "Map Cleared");
    return;
  }
}

void debug_shutdown(DbRef player, void *data, char *buffer) {
  char *args[3];
  int argc;

  argc = mech_parseattributes(buffer, args, 3);
  if (argc > 0)
    ShutDownMap(player, atoi(args[0]));
}

void debug_setvrt(DbRef player, void *data, char *buffer) {
  char *args[3];
  int vrt;
  int id, brand;

  DOCHECK(mech_parseattributes(buffer, args, 3) != 2, "Invalid arguments!");
  DOCHECK(Readnum(vrt, args[1]), "Invalid value!");
  DOCHECK(vrt <= 0, "VRT needs to be >0");
  DOCHECK(vrt > 127, "VRT can be at max 127");
  DOCHECK(!find_matching_vlong_part(args[0], NULL, &id, &brand),
          "That is no weapon!");
  DOCHECK(!IsWeapon(id), "That is no weapon!");
  MechWeapons[Weapon2I(id)].vrt = vrt;
  notify_printf(BTECH_EVALUATION_CONTEXT, player, "VRT for %s set to %d.",
                MechWeapons[Weapon2I(id)].name, vrt);
  log_error(btech_context_active()->log, LOG_WIZARD, "WIZ", "CHANGE",
            "VRT for %s set to %d by #%ld", MechWeapons[Weapon2I(id)].name, vrt,
            player);
}

void debug_setwbv(DbRef player, void *data, char *buffer) {
  char *args[3];
  int bv;
  int id, brand;

  DOCHECK(mech_parseattributes(buffer, args, 3) != 2, "Invalid arguments!");
  DOCHECK(Readnum(bv, args[1]), "Invalid value!");
  DOCHECK(bv < 0, "BV needs to be >=0");
  DOCHECK(!find_matching_vlong_part(args[0], NULL, &id, &brand),
          "That is no weapon!");
  DOCHECK(!IsWeapon(id), "That is no weapon!");
  MechWeapons[Weapon2I(id)].battlevalue = bv;
  notify_printf(BTECH_EVALUATION_CONTEXT, player, "BV for %s set to %d.",
                MechWeapons[Weapon2I(id)].name, bv);
}
