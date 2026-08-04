#include "map_obj_internal.h"

MapObject *next_mapobj(MapObject *object) { return object->next; }

MapObject *first_mapobj(BattleMap *map, int type) {
  return map->MapObject[type];
}

int find_entrance(BattleMap *map, char dir, int *x, int *y) {
  MapObject *tmp;

  for (tmp = first_mapobj(map, TYPE_ENTRANCE); tmp; tmp = next_mapobj(tmp))
    if (!dir || tmp->datac == dir) {
      *x = tmp->x;
      *y = tmp->y;
      return 1;
    }
  return 0;
}

StructureName structure_name(GameDatabase *database, MapObject *mapo) {
  StructureName result = {0};

  snprintf(result.text, sizeof(result.text), "the %s",
           game_object_name(database, mapo->obj));
  return result;
}

MapObject *find_entrance_by_target(BattleMap *map, DbRef target) {
  MapObject *tmp;

  for (tmp = first_mapobj(map, TYPE_BUILD); tmp; tmp = next_mapobj(tmp))
    if (tmp->obj == target)
      return tmp;
  return NULL;
}

MapObject *find_entrance_by_xy(BattleMap *map, int x, int y) {
  MapObject *tmp;

  for (tmp = first_mapobj(map, TYPE_BUILD); tmp; tmp = next_mapobj(tmp))
    if (tmp->x == x && tmp->y == y)
      return tmp;
  return NULL;
}

MapObject *find_mapobj(BattleMap *map, int x, int y, int type) {
  MapObject *tmp;
  int i;

  if (type >= 0) {
    for (tmp = first_mapobj(map, type); tmp; tmp = next_mapobj(tmp))
      if (tmp->x == x && tmp->y == y)
        return tmp;
  } else {
    for (i = 0; i < NUM_MAPOBJTYPES; i++)
      for (tmp = first_mapobj(map, i); tmp; tmp = next_mapobj(tmp))
        if (tmp->x == x && tmp->y == y)
          return tmp;
  }
  return NULL;
}

char find_decorations(BattleMap *map, int x, int y) {
  int i;
  MapObject *m;

  for (i = 0; i <= TYPE_LAST_DEC; i++) {
    for (m = first_mapobj(map, i); m; m = next_mapobj(m))
      if (m->x == x && m->y == y)
        return m->datac;
  }
  return 0;
}

void del_mapobj(BattleMap *map, MapObject *mapob, int type, int zap) {
  /* Delete the specified mapobj */
  struct MapObject *tmp;

  BattleMap *tmap;
  if (!(map->flags & MAPFLAG_MAPO))
    return;
  if (map->MapObject[type] != mapob) {
    for (tmp = map->MapObject[type]; tmp->next && tmp->next != mapob;
         tmp = tmp->next)
      ;
    if (!tmp->next)
      return;
    tmp->next = mapob->next;
  } else
    map->MapObject[type] = mapob->next;
  /* Then, the silly thing. Decorations, they suck */
  if (type <= TYPE_LAST_DEC) {
    /* Need to alter terrain back to 'usual' */
    if (!(zap & 2))
      map_terrain_set(map, mapob->x, mapob->y, mapob->datac);
    if (zap)
      mux_event_remove_type_data2(map->xcode.context->events, EVENT_DECORATION,
                                  mapob);
  }
  if (type == TYPE_BUILD) {

    if ((tmap = btech_context_get_map(map->xcode.context, mapob->obj))) {
      del_mapobjst(tmap, TYPE_LEAVE);
      tmap->onmap = 0;
    }
  }
  if (type == TYPE_BITS && mapob->datai != 0) {
    unsigned char **bits = (unsigned char **)mapob->datai;

    for (int y = 0; y < map->map_height; y++)
      free(bits[y]);
    free(bits);
  }
  free(mapob);
}

void del_mapobjst(BattleMap *map, int type) {
  if (!(map->flags & MAPFLAG_MAPO))
    return;
  while (map->MapObject[type])
    del_mapobj(map, map->MapObject[type], type, 3);
}

void del_mapobjs(BattleMap *map) {
  int i;

  for (i = 0; i < NUM_MAPOBJTYPES; i++)
    del_mapobjst(map, i);
  if (map->flags & MAPFLAG_MAPO)
    map->flags &= ~MAPFLAG_MAPO;
}

MapObject *add_mapobj(BattleMap *map, MapObject **to, MapObject *from,
                      int flag) {
  MapObject *realto;

  map->flags |= MAPFLAG_MAPO;
  from->next = *to;
  Create(realto, MapObject, 1);
  bcopy(from, realto, sizeof(MapObject));
  *to = realto;
  return realto;
}

static void smoke_dissipation_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;

  del_mapobj(map, o, TYPE_SMOKE, 0);
}

static void fire_dissipation_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;
  int x, y;

  x = o->x;
  y = o->y;
  del_mapobj(map, o, TYPE_FIRE, 0);
  if (IsForestHex(map, x, y)) {
    if (btech_random_range(map->xcode.context, 1, 6) < 3)
      map_terrain_set(map, x, y, GRASSLAND);
    else
      map_terrain_set(map, x, y, ROUGH);
  }
}

int FindXEven(int wind, int x) {
  switch (wind) {
  case 0:
    if (x == 0)
      return 0;
    if (x == 1)
      return -1;
    return 1;
  case 60:
    if (x == 0)
      return 1;
    if (x == 1)
      return 0;
    return 1;
  case 120:
    if (x == 0)
      return 1;
    if (x == 1)
      return 1;
    return 0;
  case 180:
    if (x == 0)
      return 0;
    if (x == 1)
      return 1;
    return -1;
  case 240:
    return x - 1;
  case 300:
    if (x == 0)
      return -1;
    if (x == 1)
      return 0;
    return -1;
  }
  return 0;
}

int FindYEven(int wind, int y) {
  switch (wind) {
  case 0:
    if (y == 0)
      return -1;
    if (y == 1)
      return 0;
    return 0;
  case 60:
    if (y == 0)
      return 0;
    if (y == 1)
      return -1;
    return 1;
  case 120:
    if (y == 0)
      return 1;
    if (y == 1)
      return 0;
    return 1;
  case 180:
    return 1;
  case 240:
    if (y == 0)
      return 1;
    if (y == 1)
      return 1;
    return 0;
  case 300:
    if (y == 0)
      return 0;
    if (y == 1)
      return -1;
    return 1;
  }
  return 0;
}

int FindXOdd(int wind, int x) {
  switch (wind) {
  case 0:
    if (x == 0)
      return 0;
    if (x == 1)
      return 1;
    return -1;
  case 60:
    if (x == 0)
      return 1;
    if (x == 1)
      return 0;
    return 1;
  case 120:
    if (x == 0)
      return 1;
    if (x == 1)
      return 1;
    return 0;
  case 180:
    if (x == 0)
      return 0;
    if (x == 1)
      return 1;
    return -1;
  case 240:
    return x - 1;
  case 300:
    if (x == 0)
      return -1;
    if (x == 1)
      return -1;
    return 0;
  }
  return 0;
}

int FindYOdd(int wind, int y) {
  switch (wind) {
  case 0:
    if (y == 0)
      return -1;
    if (y == 1)
      return -1;
    return -1;
  case 60:
    if (y == 0)
      return -1;
    if (y == 1)
      return -1;
    return 0;
  case 120:
    if (y == 0)
      return 0;
    if (y == 1)
      return -1;
    return 1;
  case 180:
    if (y == 0)
      return 1;
    if (y == 1)
      return 0;
    return 0;
  case 240:
    if (y == 0)
      return 0;
    if (y == 1)
      return 1;
    return -1;
  case 300:
    if (y == 0)
      return -1;
    if (y == 1)
      return 0;
    return -1;
  }
  return 0;
}

#define NUM_SPREAD_HEX 4

void CheckForFire(BattleMap *map, int x[], int y[]) {
  int i;

  for (i = 0; i < NUM_SPREAD_HEX; i++) {
    if (x[i] < 0 || y[i] < 0)
      continue;
    /* Cackle */
    if (IsForestHex(map, x[i], y[i]))
      add_decoration(map, x[i], y[i], TYPE_FIRE, FIRE,
                     btech_random_range(map->xcode.context, 60, 180));
  }
}

void CheckForSmoke(BattleMap *map, int x[], int y[]) {
  int i;

  for (i = 0; i < NUM_SPREAD_HEX; i++) {
    if (x[i] < 0 || y[i] < 0)
      continue;
    if (find_decorations(map, x[i], y[i]))
      continue;
    /* Cackle */
    switch (map_terrain_get(map, x[i], y[i])) {
    case BUILDING:
    case WALL:
      continue;
    default:
      break;
    }
    add_decoration(map, x[i], y[i], TYPE_SMOKE, SMOKE,
                   btech_random_range(map->xcode.context, 90, 150));
  }
}

static void FindMyCoord(BattleMap *map, int tx, int ty, int i, int wdir, int *x,
                        int *y) {
  int dx, dy;

  wdir = (((wdir + 30) / 60) * 60) % 360;
  if (tx % 2) {
    dx = tx + FindXOdd(wdir, i);
    dy = ty + FindYOdd(wdir, i);
  } else {
    dx = tx + FindXEven(wdir, i);
    dy = ty + FindYEven(wdir, i);
  }
  if (dx < 0 || dy < 0 || dx >= map->map_width || dy >= map->map_height) {
    *x = -1;
    *y = -1;
    return;
  }
  *x = dx;
  *y = dy;
}

static void fire_spreading_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;
  int x, y, loop;
  int flaggo;
  int new_fire_hex_x[4];
  int new_fire_hex_y[4];
  int new_smoke_hex_x[4];
  int new_smoke_hex_y[4];

  /*   if (btech_random_range(map->xcode.context, 1, 10) == 3) */

  /*     { */

  /*       x = o->x; */

  /*       y = o->y; */

  /*       fire_dissipation_event(e); */

  /*       return; */

  /*     } */
  x = o->x;
  y = o->y;
  for (loop = 0; loop < 3; loop++) {
    new_fire_hex_x[loop] = -1;
    new_fire_hex_y[loop] = -1;
    FindMyCoord(map, x, y, loop, map->winddir, &new_smoke_hex_x[loop],
                &new_smoke_hex_y[loop]);
  }
  new_fire_hex_x[3] = -1;
  new_fire_hex_y[3] = -1;
  FindMyCoord(map, new_smoke_hex_x[0], new_smoke_hex_y[0], 0, map->winddir,
              &new_smoke_hex_x[3], &new_smoke_hex_y[3]);

#define Spr(n, ch)                                                             \
  if (btech_random_roll(map->xcode.context) >= ch &&                           \
      btech_random_range(map->xcode.context, 1, 60) <= map->windspeed) {       \
    new_fire_hex_x[n] = new_smoke_hex_x[n];                                    \
    new_fire_hex_y[n] = new_smoke_hex_y[n];                                    \
  }
  Spr(0, 9);
  Spr(1, 11);
  Spr(2, 11);
  Spr(3, 12); /* 2 hexes 'downwind' */
#undef Spr
  CheckForSmoke(map, new_smoke_hex_x, new_smoke_hex_y);
  CheckForFire(map, new_fire_hex_x, new_fire_hex_y);
  flaggo = (o->datas -= FIRESPEED(map));
  if (flaggo > FIRESPEED(map))
    map_event_schedule(map, EVENT_DECORATION, fire_spreading_event,
                       FIRESPEED(map), (intptr_t)o);
  else
    map_event_schedule(map, EVENT_DECORATION, fire_dissipation_event, flaggo,
                       (intptr_t)o);
}

void add_decoration(BattleMap *map, int x, int y, int type, char data,
                    int flaggo) {
  MapObject foo;
  MapObject *tmpo;

  bzero(&foo, sizeof(MapObject));
  foo.x = x;
  foo.y = y;

  if (foo.x < 0 || foo.y < 0 || foo.x >= map->map_width ||
      foo.y >= map->map_height)
    return;

  foo.datac = map_real_terrain_get(map, x, y);
  /* if (foo.datac) */
  {
    MapObject *m, *m2;
    int i;

    for (i = 0; i <= TYPE_LAST_DEC; i++) {
      for (m = first_mapobj(map, i); m; m = m2) {
        m2 = next_mapobj(m);
        if (m->x == x && m->y == y)
          del_mapobj(map, m, i, 1);
      }
    }
  }
  map_terrain_set(map, x, y, data);
  foo.datas = (short)flaggo;
  tmpo = add_mapobj(map, &map->MapObject[type], &foo, 1);
  if (flaggo) {
    if (type == TYPE_SMOKE)
      map_event_schedule(map, EVENT_DECORATION, smoke_dissipation_event, flaggo,
                         (intptr_t)tmpo);
    if (type == TYPE_FIRE) {
      foo.datas = foo.datas * FIRESPEED(map) * 4 / 3 / 60;
      foo.datas = MAX(foo.datas, FIRESPEED(map) * 2);
      map_event_schedule(map, EVENT_DECORATION, fire_spreading_event,
                         FIRESPEED(map), (intptr_t)tmpo);
    }
  }
}
