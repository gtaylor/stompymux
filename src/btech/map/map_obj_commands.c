#include "btech/context.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_coordinates.h"
#include "map_obj_api.h"
#include "map_obj_internal.h"

#include "checked_conversion.h"
#include "mech_classification_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

static const char *map_type_name(int type) {
  if (type < 0)
    abort();
  const char *const *name = (const char *const *)checked_storage_at_const(
      (const void *)MAP_TYPES, NUM_MAPOBJTYPES + 1, sizeof(*MAP_TYPES),
      (size_t)type);
  return *name;
}

void list_mapobjs(DbRef player, BattleMap *map) {
  MapObject *tmp;
  int i;

  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "X   Y   Type  obj   dc   ds     di");
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "--------------------------------------------");
  for (i = 0; i < NUM_MAPOBJTYPES; i++)
    for (tmp = first_mapobj(map, i); tmp; tmp = next_mapobj(tmp)) {
      if (i == TYPE_BITS)
        mecha_notify(btech_context_evaluation(map->xcode.context), player,
                     "--- MAP/HANGAR INFORMATION OBJECT ---");
      else
        notify_printf(btech_context_evaluation(map->xcode.context), player,
                      "%-3d %-3d %-5s %-5d %-4d %-6d %ld", tmp->x, tmp->y,
                      map_type_name(i), (int)tmp->obj, tmp->datac, tmp->datas,
                      tmp->payload.scalar);
    }
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "--------------------------------------------");
}

void map_addfire(DbRef player, void *data, char *buffer) {
  /* Entrance-checking code */
  BattleMap *map = (BattleMap *)data;
  char *args[4];
  int x;
  int y;
  int d;

  if (mech_parseattributes(buffer, args, 3) != 3) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to addfire command.");
    return;
  }
  if (!parse_int_checked(args[0], &x) || !parse_int_checked(args[1], &y) ||
      !parse_int_checked(args[2], &d)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid numeric addfire argument.");
    return;
  }
  add_decoration(&(MapDecorationRequest){
      .map = map,
      .position = {.x = x, .y = y},
      .type = TYPE_FIRE,
      .terrain_marker = FIRE,
      .duration = d,
  });
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Added: Fire at (%d,%d) with duration of %ds.", x, y, d);
}

void map_addsmoke(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  char *args[4];
  int x;
  int y;
  int d;

  if (mech_parseattributes(buffer, args, 3) != 3) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to addsmoke command.");
    return;
  }
  if (!parse_int_checked(args[0], &x) || !parse_int_checked(args[1], &y) ||
      !parse_int_checked(args[2], &d)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid numeric addsmoke argument.");
    return;
  }
  add_decoration(&(MapDecorationRequest){
      .map = map,
      .position = {.x = x, .y = y},
      .type = TYPE_SMOKE,
      .terrain_marker = SMOKE,
      .duration = d,
  });
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Added: Smoke at (%d,%d) with duration of %ds.", x, y, d);
}

/* x y dist */
void map_add_block(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int x;
  int y;
  int str;
  BattleMap *map = (BattleMap *)data;
  MapObject foo;
  int team = 0;

  if (!map)
    return;
  argc = mech_parseattributes(buffer, args, 4);
  if (argc < 3 || argc > 4) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid arguments!");
    return;
  }
  if (!parse_int_checked(args[0], &x)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if (!parse_int_checked(args[1], &y)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if (!parse_int_checked(args[2], &str)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if (argc == 4 && !parse_int_checked(args[3], &team)) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }

  if (!((x >= 0) && (x < map->map_width) && (y >= 0) &&
        (y < map->map_height))) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "X,Y out of range!");
    return;
  }

  memset(&foo, 0, sizeof(MapObject));
  foo.x = clamp_int_to_short(x);
  foo.y = clamp_int_to_short(y);
  foo.payload.scalar = str;
  foo.obj = player;
  foo.datac = team;
  add_mapobj_to_type(map, TYPE_B_LZ, &foo, 1);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Landingzone-block added to %d,%d (distance: %d)", x, y, str);
}

int is_blocked_lz(Mech *mech, BattleMap *map, int x, int y) {
  MapObject *o;
  float fx;
  float fy;
  float tx;
  float ty;

  map_coord_to_real_coord(x, y, &fx, &fy);
  for (o = first_mapobj(map, TYPE_B_LZ); o; o = next_mapobj(o)) {
    // comment this out...That makes it a square BLZ, not round
    //		if(abs(x - o->x) > o->payload.scalar ||
    //		   abs(y - o->y) > o->payload.scalar)
    //			continue;
    if (o->datac && o->datac == mech_team(mech))
      continue;
    map_coord_to_real_coord(o->x, o->y, &tx, &ty);
    if (map_real_range(&(MapRealSegment){
            .start = {.x = fx, .y = fy},
            .end = {.x = tx, .y = ty},
        }) <= (float)o->payload.scalar)
      return 1;
  }
  return 0;
}

void map_setlinked(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  MapObject foo;

  memset(&foo, 0, sizeof(MapObject));
  foo.datac = 1;
  add_mapobj_to_type(map, TYPE_LINKED, &foo, 1);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Map set to linked.");
}

int map_objects_delete(const MapObjectLookupRequest *request) {
  BattleMap *map = request->map;
  int count = 0;
  MapObject *foo;
  MapObject *foo2;

  for (foo = first_mapobj(map, request->type); foo; foo = foo2) {
    foo2 = next_mapobj(foo);
    if (foo->x == request->position.x && foo->y == request->position.y) {
      del_mapobj(&(MapObjectDeleteRequest){
          .map = map,
          .object = foo,
          .type = request->type,
          .cancel_event = true,
      });
      count++;
    }
  }
  return count;
}

void map_delobj(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  char *args[5];
  MapObject *foo;
  MapObject *foo2;
  int tt;
  int count = 0;
  int mdel = 0;
  int x;
  int y;

  switch (mech_parseattributes(buffer, args, 3)) {
  case 0:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to delobj command.");
    return;
  case 1:
    tt = listmatch(MAP_TYPES, NUM_MAPOBJTYPES, args[0]);
    if (tt < 0) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid type!");
      return;
    }
    for (foo = first_mapobj(map, tt); foo; foo = foo2) {
      foo2 = next_mapobj(foo);
      del_mapobj(&(MapObjectDeleteRequest){
          .map = map,
          .object = foo,
          .type = tt,
          .cancel_event = true,
      });
      count++;
    }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d objects deleted!", count);
    if (tt == TYPE_MINE)
      mdel = 1;
    break;
  case 2:
    if (!parse_int_checked(args[0], &x) || !parse_int_checked(args[1], &y)) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid coordinates!");
      return;
    }
    for (tt = 0; tt < NUM_MAPOBJTYPES; tt++)
      for (foo = first_mapobj(map, tt); foo; foo = foo2) {
        foo2 = next_mapobj(foo);
        if (foo->x == x && foo->y == y) {
          if (tt == TYPE_MINE)
            mdel = 1;
          del_mapobj(&(MapObjectDeleteRequest){
              .map = map,
              .object = foo,
              .type = tt,
              .cancel_event = true,
          });
          count++;
        }
      }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d objects at (%d,%d) deleted.", count, x, y);
    break;
  case 3:
    tt = listmatch(MAP_TYPES, NUM_MAPOBJTYPES, args[0]);
    if (tt < 0) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid type!");
      return;
    }
    if (!parse_int_checked(args[1], &x) || !parse_int_checked(args[2], &y)) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid coordinates!");
      return;
    }
    for (foo = first_mapobj(map, tt); foo; foo = foo2) {
      foo2 = next_mapobj(foo);
      if (foo->x == x && foo->y == y) {
        if (tt == TYPE_MINE)
          mdel = 1;
        del_mapobj(&(MapObjectDeleteRequest){
            .map = map,
            .object = foo,
            .type = tt,
            .cancel_event = true,
        });
        count++;
      }
    }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d %s at (%d,%d) deleted.", count, map_type_name(tt), x, y);
    break;
  default:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  if (mdel)
    mine_fields_recalculate(map);
}
