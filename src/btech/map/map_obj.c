#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_obj_internal.h"
#include "map_object_query_api.h"

#include "checked_conversion.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static MapObject **map_object_slot(BattleMap *map, int type) {
  if (type < 0)
    abort();
  return (MapObject **)checked_storage_at(
      (void *)map->map_object, NUM_MAPOBJTYPES, sizeof(*map->map_object),
      (size_t)type);
}

MapObject *next_mapobj(MapObject *object) { return object->next; }

MapObject *first_mapobj(BattleMap *map, int type) {
  return *map_object_slot(map, type);
}

MapObject *battle_map_object_first(BattleMap *map, int type) {
  return first_mapobj(map, type);
}

MapObject *battle_map_object_next(MapObject *object) {
  return next_mapobj(object);
}

int battle_map_object_x(const MapObject *object) { return object->x; }

int battle_map_object_y(const MapObject *object) { return object->y; }

DbRef battle_map_object_dbref(const MapObject *object) { return object->obj; }

MapEntranceResult find_entrance(BattleMap *map, char direction) {
  MapObject *tmp;

  for (tmp = first_mapobj(map, TYPE_ENTRANCE); tmp; tmp = next_mapobj(tmp)) {
    if (!direction || tmp->datac == direction) {
      return (MapEntranceResult){
          .found = true,
          .position = {.x = tmp->x, .y = tmp->y},
      };
    }
  }
  return (MapEntranceResult){0};
}

StructureName structure_name(GameDatabase *database, MapObject *mapo) {
  StructureName result = {0};

  (void)snprintf(result.text, sizeof(result.text), "the %s",
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

MapObject *find_mapobj(const MapObjectLookupRequest *request) {
  BattleMap *map = request->map;
  MapObject *tmp;
  int i;

  if (request->type >= 0) {
    for (tmp = first_mapobj(map, request->type); tmp; tmp = next_mapobj(tmp))
      if (tmp->x == request->position.x && tmp->y == request->position.y)
        return tmp;
  } else {
    for (i = 0; i < NUM_MAPOBJTYPES; i++)
      for (tmp = first_mapobj(map, i); tmp; tmp = next_mapobj(tmp))
        if (tmp->x == request->position.x && tmp->y == request->position.y)
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
        return clamp_int_to_char(m->datac);
  }
  return 0;
}

void del_mapobj(const MapObjectDeleteRequest *request) {
  BattleMap *map = request->map;
  MapObject *mapob = request->object;
  int type = request->type;
  /* Delete the specified mapobj */
  struct MapObject *tmp;

  BattleMap *tmap;
  if (!(map->flags & MAPFLAG_MAPO))
    return;
  MapObject **object_slot = map_object_slot(map, type);
  if (*object_slot != mapob) {
    for (tmp = *object_slot; tmp->next && tmp->next != mapob; tmp = tmp->next)
      ;
    if (!tmp->next)
      return;
    tmp->next = mapob->next;
  } else {
    *object_slot = mapob->next;
  }
  /* Then, the silly thing. Decorations, they suck */
  if (type <= TYPE_LAST_DEC) {
    /* Need to alter terrain back to 'usual' */
    if (!request->preserve_terrain)
      map_terrain_set(map, mapob->x, mapob->y, clamp_int_to_char(mapob->datac));
    if (request->cancel_event)
      mux_event_remove_type_data2(map->xcode.context->events, EVENT_DECORATION,
                                  mapob);
  }
  if (type == TYPE_BUILD) {

    tmap = btech_context_get_map(map->xcode.context, mapob->obj);
    if (tmap) {
      del_mapobjst(tmap, TYPE_LEAVE);
      tmap->onmap = 0;
    }
  }
  if (type == TYPE_BITS && mapob->payload.bits != nullptr) {
    unsigned char **bits = mapob->payload.bits;

    for (int y = 0; y < map->map_height; y++) {
      unsigned char **row = (unsigned char **)checked_storage_at(
          (void *)bits, (size_t)map->map_height, sizeof(*bits), (size_t)y);
      free(*row);
    }
    free((void *)bits);
  }
  free(mapob);
}

void del_mapobjst(BattleMap *map, int type) {
  if (!(map->flags & MAPFLAG_MAPO))
    return;
  MapObject **object_slot = map_object_slot(map, type);
  while (*object_slot) {
    del_mapobj(&(MapObjectDeleteRequest){
        .map = map,
        .object = *object_slot,
        .type = type,
        .preserve_terrain = true,
        .cancel_event = true,
    });
  }
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
  realto = checked_storage_allocate(sizeof(*realto));
  memmove(realto, from, sizeof(MapObject));
  *to = realto;
  return realto;
}

MapObject *add_mapobj_to_type(BattleMap *map, int type, MapObject *from,
                              int flag) {
  return add_mapobj(map, map_object_slot(map, type), from, flag);
}

static void smoke_dissipation_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;

  del_mapobj(&(MapObjectDeleteRequest){
      .map = map,
      .object = o,
      .type = TYPE_SMOKE,
  });
}

static void fire_dissipation_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;
  int x;
  int y;

  x = o->x;
  y = o->y;
  del_mapobj(&(MapObjectDeleteRequest){
      .map = map,
      .object = o,
      .type = TYPE_FIRE,
  });
  char terrain = map_real_terrain_get(map, x, y);
  if (terrain == LIGHT_FOREST || terrain == HEAVY_FOREST) {
    if (btech_random_range(map->xcode.context, 1, 6) < 3)
      map_terrain_set(map, x, y, GRASSLAND);
    else
      map_terrain_set(map, x, y, ROUGH);
  }
}

typedef struct WindOffsetRequest {
  int direction;
  int branch;
} WindOffsetRequest;

static int wind_x_even(const WindOffsetRequest *request) {
  int wind = request->direction;
  int x = request->branch;
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

static int wind_y_even(const WindOffsetRequest *request) {
  int wind = request->direction;
  int y = request->branch;
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

static int wind_x_odd(const WindOffsetRequest *request) {
  int wind = request->direction;
  int x = request->branch;
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

static int wind_y_odd(const WindOffsetRequest *request) {
  int wind = request->direction;
  int y = request->branch;
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

typedef MapHexPosition SpreadHex;

static SpreadHex *spread_hex(SpreadHex *hexes, int index) {
  if (index < 0)
    abort();
  return checked_storage_at(hexes, NUM_SPREAD_HEX, sizeof(*hexes),
                            (size_t)index);
}

static void check_for_fire(BattleMap *map, SpreadHex hexes[]) {
  int i;

  for (i = 0; i < NUM_SPREAD_HEX; i++) {
    SpreadHex *hex = spread_hex(hexes, i);
    if (hex->x < 0 || hex->y < 0)
      continue;
    /* Cackle */
    char terrain = map_real_terrain_get(map, hex->x, hex->y);
    if (terrain == LIGHT_FOREST || terrain == HEAVY_FOREST) {
      add_decoration(&(MapDecorationRequest){
          .map = map,
          .position = {.x = hex->x, .y = hex->y},
          .type = TYPE_FIRE,
          .terrain_marker = FIRE,
          .duration = btech_random_range_int(map->xcode.context, 60, 180),
      });
    }
  }
}

static void check_for_smoke(BattleMap *map, SpreadHex hexes[]) {
  int i;

  for (i = 0; i < NUM_SPREAD_HEX; i++) {
    SpreadHex *hex = spread_hex(hexes, i);
    if (hex->x < 0 || hex->y < 0)
      continue;
    if (find_decorations(map, hex->x, hex->y))
      continue;
    /* Cackle */
    switch (map_terrain_get(map, hex->x, hex->y)) {
    case BUILDING:
    case WALL:
      continue;
    default:
      break;
    }
    add_decoration(&(MapDecorationRequest){
        .map = map,
        .position = {.x = hex->x, .y = hex->y},
        .type = TYPE_SMOKE,
        .terrain_marker = SMOKE,
        .duration = btech_random_range_int(map->xcode.context, 90, 150),
    });
  }
}

typedef struct WindSpreadRequest {
  BattleMap *map;
  MapHexPosition origin;
  int branch;
  int direction;
} WindSpreadRequest;

static MapHexPosition wind_spread_position(const WindSpreadRequest *request) {
  int dx;
  int dy;

  WindOffsetRequest offset = {
      .direction = (((request->direction + 30) / 60) * 60) % 360,
      .branch = request->branch,
  };
  if (request->origin.x % 2) {
    dx = request->origin.x + wind_x_odd(&offset);
    dy = request->origin.y + wind_y_odd(&offset);
  } else {
    dx = request->origin.x + wind_x_even(&offset);
    dy = request->origin.y + wind_y_even(&offset);
  }
  if (dx < 0 || dy < 0 || dx >= request->map->map_width ||
      dy >= request->map->map_height)
    return (MapHexPosition){.x = -1, .y = -1};
  return (MapHexPosition){.x = dx, .y = dy};
}

static void fire_spreading_event(MuxEvent *e) {
  BattleMap *map = (BattleMap *)e->data;
  MapObject *o = (MapObject *)e->data2;
  int x;
  int y;
  int loop;
  int flaggo;
  SpreadHex new_fire_hexes[NUM_SPREAD_HEX];
  SpreadHex new_smoke_hexes[NUM_SPREAD_HEX];

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
    *spread_hex(new_fire_hexes, loop) = (SpreadHex){.x = -1, .y = -1};
    SpreadHex *smoke_hex = spread_hex(new_smoke_hexes, loop);
    *smoke_hex = wind_spread_position(&(WindSpreadRequest){
        .map = map,
        .origin = {.x = x, .y = y},
        .branch = loop,
        .direction = map->winddir,
    });
  }
  *spread_hex(new_fire_hexes, 3) = (SpreadHex){.x = -1, .y = -1};
  SpreadHex *first_smoke_hex = spread_hex(new_smoke_hexes, 0);
  SpreadHex *last_smoke_hex = spread_hex(new_smoke_hexes, 3);
  *last_smoke_hex = wind_spread_position(&(WindSpreadRequest){
      .map = map,
      .origin = *first_smoke_hex,
      .direction = map->winddir,
  });

  for (int candidate = 0; candidate < 4; candidate++) {
    const int THRESHOLD = candidate == 0 ? 9 : candidate == 3 ? 12 : 11;
    if (btech_random_roll(map->xcode.context) >= THRESHOLD &&
        btech_random_range(map->xcode.context, 1, 60) <= map->windspeed) {
      *spread_hex(new_fire_hexes, candidate) =
          *spread_hex(new_smoke_hexes, candidate);
    }
  }
  check_for_smoke(map, new_smoke_hexes);
  check_for_fire(map, new_fire_hexes);
  flaggo = (o->datas -= map_fire_speed(map));
  if (flaggo > map_fire_speed(map))
    map_event_schedule(map, EVENT_DECORATION, fire_spreading_event,
                       map_fire_speed(map), (intptr_t)o);
  else
    map_event_schedule(map, EVENT_DECORATION, fire_dissipation_event, flaggo,
                       (intptr_t)o);
}

void add_decoration(const MapDecorationRequest *request) {
  BattleMap *map = request->map;
  int x = request->position.x;
  int y = request->position.y;
  int type = request->type;
  int flaggo = request->duration;
  MapObject foo;
  MapObject *tmpo;

  memset(&foo, 0, sizeof(MapObject));
  foo.x = clamp_int_to_short(x);
  foo.y = clamp_int_to_short(y);

  if (foo.x < 0 || foo.y < 0 || foo.x >= map->map_width ||
      foo.y >= map->map_height)
    return;

  foo.datac = (unsigned char)map_real_terrain_get(map, x, y);
  /* if (foo.datac) */
  {
    MapObject *m;
    MapObject *m2;
    int i;

    for (i = 0; i <= TYPE_LAST_DEC; i++) {
      for (m = first_mapobj(map, i); m; m = m2) {
        m2 = next_mapobj(m);
        if (m->x == x && m->y == y) {
          del_mapobj(&(MapObjectDeleteRequest){
              .map = map,
              .object = m,
              .type = i,
              .cancel_event = true,
          });
        }
      }
    }
  }
  map_terrain_set(map, x, y, request->terrain_marker);
  foo.datas = clamp_int_to_short(flaggo);
  tmpo = add_mapobj(map, map_object_slot(map, type), &foo, 1);
  if (flaggo) {
    if (type == TYPE_SMOKE)
      map_event_schedule(map, EVENT_DECORATION, smoke_dissipation_event, flaggo,
                         (intptr_t)tmpo);
    if (type == TYPE_FIRE) {
      const int FIRE_DURATION = foo.datas * map_fire_speed(map) * 4 / 3 / 60;
      foo.datas = clamp_int_to_short(FIRE_DURATION);
      foo.datas = clamp_int_to_short(max(foo.datas, map_fire_speed(map) * 2));
      map_event_schedule(map, EVENT_DECORATION, fire_spreading_event,
                         map_fire_speed(map), (intptr_t)tmpo);
    }
  }
}
