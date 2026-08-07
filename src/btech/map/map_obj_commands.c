#include "map_obj_internal.h"

#include "mech_classification_api.h"
#include "registry_api.h"

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
                      map_types[i], (int)tmp->obj, tmp->datac, tmp->datas,
                      tmp->datai);
    }
  mecha_notify(btech_context_evaluation(map->xcode.context), player,
               "--------------------------------------------");
}

void map_addfire(DbRef player, void *data, char *buffer) {
  /* Entrance-checking code */
  BattleMap *map = (BattleMap *)data;
  char *args[4];
  int x, y, d;

  if (mech_parseattributes(buffer, args, 3) != 3) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to addfire command.");
    return;
  }
  x = atoi(args[0]);
  y = atoi(args[1]);
  d = atoi(args[2]);
  add_decoration(map, x, y, TYPE_FIRE, FIRE, d);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Added: Fire at (%d,%d) with duration of %ds.", x, y, d);
}

void map_addsmoke(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  char *args[4];
  int x, y, d;

  if (mech_parseattributes(buffer, args, 3) != 3) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to addsmoke command.");
    return;
  }
  x = atoi(args[0]);
  y = atoi(args[1]);
  d = atoi(args[2]);
  add_decoration(map, x, y, TYPE_SMOKE, SMOKE, d);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Added: Smoke at (%d,%d) with duration of %ds.", x, y, d);
}

/* x y dist */
void map_add_block(DbRef player, void *data, char *buffer) {
  char *args[4];
  int argc;
  int x, y, str;
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
  if ((!((x) = atoi(args[0])) && strcmp((args[0]), "0"))) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if ((!((y) = atoi(args[1])) && strcmp((args[1]), "0"))) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if ((!((str) = atoi(args[2])) && strcmp((args[2]), "0"))) {
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number!");
    return;
  }
  if (argc == 4)
    if ((!((team) = atoi(args[3])) && strcmp((args[3]), "0"))) {
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

  bzero(&foo, sizeof(MapObject));
  foo.x = x;
  foo.y = y;
  foo.datai = str;
  foo.obj = player;
  foo.datac = team;
  add_mapobj(map, &map->MapObject[TYPE_B_LZ], &foo, 1);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Landingzone-block added to %d,%d (distance: %d)", x, y, str);
}

int is_blocked_lz(Mech *mech, BattleMap *map, int x, int y) {
  MapObject *o;
  float fx, fy;
  float tx, ty;

  MapCoordToRealCoord(x, y, &fx, &fy);
  for (o = first_mapobj(map, TYPE_B_LZ); o; o = next_mapobj(o)) {
    // comment this out...That makes it a square BLZ, not round
    //		if(abs(x - o->x) > o->datai || abs(y - o->y) > o->datai)
    //			continue;
    if (o->datac && o->datac == mech_team(mech))
      continue;
    MapCoordToRealCoord(o->x, o->y, &tx, &ty);
    if (FindHexRange(fx, fy, tx, ty) <= o->datai)
      return 1;
  }
  return 0;
}

void map_setlinked(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  MapObject foo;

  bzero(&foo, sizeof(MapObject));
  foo.datac = 1;
  add_mapobj(map, &map->MapObject[TYPE_LINKED], &foo, 1);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Map set to linked.");
}

int mapobj_del(BattleMap *map, int x, int y, int tt) {
  int count = 0;
  MapObject *foo, *foo2;

  for (foo = first_mapobj(map, tt); foo; foo = foo2) {
    foo2 = next_mapobj(foo);
    if (foo->x == x && foo->y == y) {
      del_mapobj(map, foo, tt, 1);
      count++;
    }
  }
  return count;
}

void map_delobj(DbRef player, void *data, char *buffer) {
  BattleMap *map = (BattleMap *)data;
  char *args[5];
  MapObject *foo, *foo2;
  int tt, count = 0, mdel = 0;
  int x, y;

  switch (mech_parseattributes(buffer, args, 3)) {
  case 0:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Error: Invalid number of attributes to delobj command.");
    return;
  case 1:
    if ((tt = listmatch(map_types, args[0])) < 0) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid type!");
      return;
    }
    for (foo = map->MapObject[tt]; foo; foo = foo2) {
      foo2 = next_mapobj(foo);
      del_mapobj(map, foo, tt, 1);
      count++;
    }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d objects deleted!", count);
    if (tt == TYPE_MINE)
      mdel = 1;
    break;
  case 2:
    x = atoi(args[0]);
    y = atoi(args[1]);
    for (tt = 0; tt < NUM_MAPOBJTYPES; tt++)
      for (foo = first_mapobj(map, tt); foo; foo = foo2) {
        foo2 = next_mapobj(foo);
        if (foo->x == x && foo->y == y) {
          if (tt == TYPE_MINE)
            mdel = 1;
          del_mapobj(map, foo, tt, 1);
          count++;
        }
      }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d objects at (%d,%d) deleted.", count, x, y);
    break;
  case 3:
    if ((tt = listmatch(map_types, args[0])) < 0) {
      mecha_notify(btech_context_evaluation(map->xcode.context), player,
                   "Invalid type!");
      return;
    }
    x = atoi(args[1]);
    y = atoi(args[2]);
    for (foo = first_mapobj(map, tt); foo; foo = foo2) {
      foo2 = next_mapobj(foo);
      if (foo->x == x && foo->y == y) {
        if (tt == TYPE_MINE)
          mdel = 1;
        del_mapobj(map, foo, tt, 1);
        count++;
      }
    }
    notify_printf(btech_context_evaluation(map->xcode.context), player,
                  "%d %s at (%d,%d) deleted.", count, map_types[tt], x, y);
    break;
  default:
    mecha_notify(btech_context_evaluation(map->xcode.context), player,
                 "Invalid number of arguments!");
    return;
  }
  if (mdel)
    mine_fields_recalculate(map);
}
