#include "btech/context.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_bits_api.h"
#include "map_obj_api.h"
#include "mine_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

typedef struct MapLinkUpdateStats {
  int builds;
  int leaves;
  int entrances;
} MapLinkUpdateStats;

typedef struct MapDirection {
  int x, y;
  char dir;
} MapDirection;

static const MapDirection DIRECTION_TABLE[4] = {
    {1, 0, 'n'}, {2, 1, 'e'}, {1, 2, 's'}, {0, 1, 'w'}};

static const MapDirection *direction_entry(int direction) {
  if (direction < 0)
    abort();
  return checked_storage_at_const(DIRECTION_TABLE, 4, sizeof(*DIRECTION_TABLE),
                                  (size_t)direction);
}

static char *link_argument(char **arguments, size_t count, int index) {
  if (index < 0)
    abort();
  char **slot =
      checked_storage_at(arguments, count, sizeof(*arguments), (size_t)index);
  return *slot;
}

static void recursively_update_links(BtechContext *context, DbRef from,
                                     DbRef loc, MapLinkUpdateStats *stats);

int parse_coord(BattleMap *map, int dir, char *data, int *x, int *y) {
  int tx, ty, tox, toy;
  int doh;

  if (strchr(data, ',')) {
    if (sscanf(data, "%d,%d", x, y) == 2)
      return 1;
    return 0;
  }
  doh = atoi(data);
  if (doh < 0)
    return 0;
  const MapDirection *direction = direction_entry(dir);
  tox = direction->x;
  toy = direction->y;
  tx = (map->map_width * tox) / 2;
  if (tx >= map->map_width)
    tx = map->map_width - 1;
  ty = (map->map_height * toy) / 2;
  if (ty >= map->map_height)
    ty = map->map_height - 1;
  if (tox == 1)
    ty += (toy > 1) ? (0 - doh) : (doh);
  if (toy == 1)
    tx += (tox > 1) ? (0 - doh) : (doh);
  if (tx < 0)
    tx = 0;
  if (ty < 0)
    ty = 0;
  if (tx >= map->map_width)
    tx = (map->map_width - 1);
  if (ty >= map->map_height)
    ty = (map->map_height - 1);
  *x = tx;
  *y = ty;
  return 1;
}

static void add_entrances(DbRef loc, BattleMap *map, char *data,
                          MapLinkUpdateStats *stats) {
  char *buf;
  char *args[4];
  int x, y, i;
  MapObject foo;

  bzero(&foo, sizeof(MapObject));

  buf = alloc_mbuf("add_entrances");

  strlcpy(buf, data, MBUF_SIZE);
  if (mech_parseattributes(buf, args, 4) == 4) {
    for (i = 0; i < 4; i++)
      if ((parse_coord(map, i, link_argument(args, 4, i), &x, &y))) {
        foo.datac = direction_entry(i)->dir;
        foo.x = clamp_int_to_short(x);
        foo.y = clamp_int_to_short(y);
        add_mapobj_to_type(map, TYPE_ENTRANCE, &foo, 1);
        if (stats != nullptr)
          stats->entrances++;
      }
  }
  free_mbuf(buf);
}

static void add_links(DbRef loc, BattleMap *map, char *data,
                      MapLinkUpdateStats *stats) {
  char *buf;
  char *args[500];
  int i, found, targ;
  char *tmps;
  int x, y;
  MapObject foo;

  bzero(&foo, sizeof(MapObject));

  buf = alloc_lbuf("add_links");

  strlcpy(buf, data, LBUF_SIZE);
  if ((found = mech_parseattributes(buf, args, 500)) > 0)
    for (i = 0; i < found; i++) {
      targ = atoi(link_argument(args, 500, i));
      if (targ < 0 || !btech_context_find_object(map->xcode.context, targ) ||
          targ == loc)
        continue;
      tmps = btech_attribute_read(map->xcode.context->database, targ,
                                  A_BUILDCOORD, (char[LBUF_SIZE]){0});
      if (!tmps)
        continue;
      if (sscanf(tmps, "%d,%d", &x, &y) != 2)
        continue;
      if (x < 0 || x >= map->map_width || y < 0 || y >= map->map_height)
        continue;
      set_hex_enterable(map, x, y);
      foo.x = clamp_int_to_short(x);
      foo.y = clamp_int_to_short(y);
      foo.obj = targ;
      add_mapobj_to_type(map, TYPE_BUILD, &foo, 1);
      if (stats != nullptr)
        stats->builds++;
      recursively_update_links(map->xcode.context, loc, targ, stats);
    }
  free_lbuf(buf);
}

static void recursively_update_links(BtechContext *context, DbRef from,
                                     DbRef loc, MapLinkUpdateStats *stats) {
  BattleMap *map;
  MapObject foo;
  char *tmps;

  bzero(&foo, sizeof(MapObject));
  if (!(map = btech_context_get_map(context, loc)))
    return;
  clear_hex_bits(map, 2);
  if (from >= 0) {
    map->onmap = from;
    /* Update leave exit */
    del_mapobjst(map, TYPE_LEAVE);
    if (stats != nullptr)
      stats->leaves++;
    foo.obj = from;
    add_mapobj_to_type(map, TYPE_LEAVE, &foo, 0);
    del_mapobjst(map, TYPE_ENTRANCE);
    /* Places you can enter this place from.. it's more or less
       directly taken from BUILDENTRANCE */
    tmps = btech_attribute_read(context->database, loc, A_BUILDENTRANCE,
                                (char[LBUF_SIZE]){0});
    if (tmps) {
      /* number number number number
         or
         x,y x,y x,y x,y
       */
      add_entrances(loc, map, tmps, stats);
    }
  }
  del_mapobjst(map, TYPE_BUILD);
  tmps = btech_attribute_read(context->database, loc, A_BUILDLINKS,
                              (char[LBUF_SIZE]){0});
  if (tmps)
    add_links(loc, map, tmps, stats);
}

void recursively_updatelinks(BtechContext *context, DbRef from, DbRef loc) {
  recursively_update_links(context, from, loc, nullptr);
}

void map_updatelinks(DbRef player, void *data, char *buffer) {
  BattleMap *map = data;
  MapLinkUpdateStats stats = {0};
  DbRef ourloc;

  ourloc = game_object_location(map->xcode.context->database, player);
  recursively_update_links(map->xcode.context, NOTHING, ourloc, &stats);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Updated %d BUILD objs, %d LEAVE objs, %d ENTRANCE objs.",
                stats.builds, stats.leaves, stats.entrances);
}

int map_linked(BtechContext *context, DbRef map_object) {
  BattleMap *map = btech_context_get_map(context, map_object);

  if (!map)
    return 0;
  return (first_mapobj(map, TYPE_LINKED)) ? 1 : 0;
}
