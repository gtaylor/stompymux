#include "btech/context.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
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
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct MapLinkUpdateStats {
  int builds;
  int leaves;
  int entrances;
  int skipped_links;
} MapLinkUpdateStats;

typedef struct MapLinkTraversalState {
  /*
   * This set belongs to the whole command, not one recursion path. A map
   * reachable from multiple parents is processed once, so its first parent
   * owns the surviving LEAVE exit; unwinding a path would re-expand dense
   * DAGs and restore the old repeated-work behavior.
   */
  RedBlackTree visited;
  char *attribute_buffer;
} MapLinkTraversalState;

typedef struct MapLinkUpdateRequest {
  BtechContext *context;
  DbRef source;
  DbRef location;
  MapLinkUpdateStats *stats;
  MapLinkTraversalState *traversal;
  size_t depth;
} MapLinkUpdateRequest;

typedef struct MapDirection {
  int x, y;
  char dir;
} MapDirection;

static const MapDirection DIRECTION_TABLE[4] = {
    {1, 0, 'n'}, {2, 1, 'e'}, {1, 2, 's'}, {0, 1, 'w'}};

enum {
  MAP_LINK_ARGUMENT_CAPACITY = 500,
  MAP_LINK_MAX_DEPTH = 1024,
};

static const MapDirection *direction_entry(int direction) {
  if (direction < 0)
    abort();
  return checked_storage_at_const(DIRECTION_TABLE, 4, sizeof(*DIRECTION_TABLE),
                                  (size_t)direction);
}

static char *link_argument(char **arguments, size_t count, int index) {
  if (index < 0)
    abort();
  char **slot = (char **)checked_storage_at((void *)arguments, count,
                                            sizeof(*arguments), (size_t)index);
  return *slot;
}

static void recursively_update_links(const MapLinkUpdateRequest *request);

static int compare_dbrefs(const RedBlackTreeCompareCall *call) {
  const DbRef LEFT = (DbRef)call->lhs;
  const DbRef RIGHT = (DbRef)call->rhs;

  if (LEFT < RIGHT)
    return -1;
  if (LEFT > RIGHT)
    return 1;
  return 0;
}

static void map_link_update_skip(MapLinkUpdateStats *stats) {
  if (stats != nullptr)
    stats->skipped_links++;
}

static bool map_link_update_visit(const MapLinkUpdateRequest *request) {
  if (request->depth >= MAP_LINK_MAX_DEPTH ||
      red_black_tree_exists(request->traversal->visited,
                            (void *)request->location)) {
    map_link_update_skip(request->stats);
    return false;
  }
  red_black_tree_insert(request->traversal->visited, (void *)request->location,
                        request->traversal);
  return true;
}

static bool parse_coordinate_pair(char *text, int *x, int *y) {
  char *separator = strchr(text, ',');
  if (separator == nullptr)
    return false;
  const size_t SEPARATOR_OFFSET = (size_t)(separator - text);
  char *second = checked_mutable_string_suffix(text, SEPARATOR_OFFSET + 1);
  *separator = '\0';
  const bool PARSED =
      (parse_int_checked(text, x) && parse_int_checked(second, y)) != 0;
  *separator = ',';
  return PARSED;
}

bool parse_coord(BattleMap *map, int dir, char *data, int *x, int *y) {
  int tx;
  int ty;
  int tox;
  int toy;
  int doh;

  if (strchr(data, ',')) {
    return parse_coordinate_pair(data, x, y);
  }
  if (!parse_int_checked(data, &doh) || doh < 0)
    return false;
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
  return true;
}

static void add_entrances(DbRef loc [[maybe_unused]], BattleMap *map,
                          char *data, MapLinkUpdateStats *stats) {
  char *buf;
  char *args[4];
  int x;
  int y;
  int i;
  MapObject foo;

  memset(&foo, 0, sizeof(MapObject));

  buf = alloc_mbuf("add_entrances");

  (void)string_copy_bounded(buf, MBUF_SIZE, data);
  if (mech_parseattributes(buf, args, 4) == 4) {
    for (i = 0; i < 4; i++) {
      if ((parse_coord(map, i, link_argument(args, 4, i), &x, &y))) {
        foo.datac = (unsigned char)direction_entry(i)->dir;
        foo.x = clamp_int_to_short(x);
        foo.y = clamp_int_to_short(y);
        add_mapobj_to_type(map, TYPE_ENTRANCE, &foo, 1);
        if (stats != nullptr)
          stats->entrances++;
      }
    }
  }
  free_buf(buf);
}

static void add_links(const MapLinkUpdateRequest *request, BattleMap *map,
                      char *data) {
  char *buf;
  char **args;
  int i;
  int found;
  int targ;
  char *tmps;
  int x;
  int y;
  MapObject foo;

  memset(&foo, 0, sizeof(MapObject));

  buf = alloc_lbuf("add_links");
  args = (char **)checked_storage_allocate_array(MAP_LINK_ARGUMENT_CAPACITY,
                                                 sizeof(*args));

  (void)string_copy_bounded(buf, LBUF_SIZE, data);
  found = mech_parseattributes(buf, args, MAP_LINK_ARGUMENT_CAPACITY);
  if (found > 0) {
    for (i = 0; i < found; i++) {
      if (!parse_int_checked(link_argument(args, MAP_LINK_ARGUMENT_CAPACITY, i),
                             &targ))
        continue;
      if (targ < 0 || !btech_context_find_object(map->xcode.context, targ) ||
          targ == request->location) {
        continue;
      }
      tmps =
          btech_attribute_read(map->xcode.context->database, targ, A_BUILDCOORD,
                               request->traversal->attribute_buffer);
      if (!tmps)
        continue;
      if (!parse_coordinate_pair(tmps, &x, &y))
        continue;
      if (x < 0 || x >= map->map_width || y < 0 || y >= map->map_height)
        continue;
      set_hex_enterable(map, x, y);
      foo.x = clamp_int_to_short(x);
      foo.y = clamp_int_to_short(y);
      foo.obj = targ;
      add_mapobj_to_type(map, TYPE_BUILD, &foo, 1);
      if (request->stats != nullptr)
        request->stats->builds++;
      MapLinkUpdateRequest nested = *request;
      nested.source = request->location;
      nested.location = targ;
      nested.depth++;
      recursively_update_links(&nested);
    }
  }
  free((void *)args);
  free_buf(buf);
}

static void recursively_update_links(const MapLinkUpdateRequest *request) {
  BtechContext *context = request->context;
  const DbRef FROM = request->source;
  const DbRef LOC = request->location;
  MapLinkUpdateStats *stats = request->stats;
  BattleMap *map;
  MapObject foo;
  char *tmps;

  if (!map_link_update_visit(request))
    return;
  memset(&foo, 0, sizeof(MapObject));
  map = btech_context_get_map(context, LOC);
  if (!map)
    return;
  clear_hex_bits(map, 2);
  if (FROM >= 0) {
    map->onmap = FROM;
    /* Update leave exit */
    del_mapobjst(map, TYPE_LEAVE);
    if (stats != nullptr)
      stats->leaves++;
    foo.obj = FROM;
    add_mapobj_to_type(map, TYPE_LEAVE, &foo, 0);
    del_mapobjst(map, TYPE_ENTRANCE);
    /* Places you can enter this place from.. it's more or less
       directly taken from BUILDENTRANCE */
    tmps = btech_attribute_read(context->database, LOC, A_BUILDENTRANCE,
                                request->traversal->attribute_buffer);
    if (tmps) {
      /* number number number number
         or
         x,y x,y x,y x,y
       */
      add_entrances(LOC, map, tmps, stats);
    }
  }
  del_mapobjst(map, TYPE_BUILD);
  tmps = btech_attribute_read(context->database, LOC, A_BUILDLINKS,
                              request->traversal->attribute_buffer);
  if (tmps)
    add_links(request, map, tmps);
}

static void update_links(BtechContext *context, DbRef source, DbRef location,
                         MapLinkUpdateStats *stats) {
  MapLinkTraversalState traversal = {
      .visited = red_black_tree_init(compare_dbrefs, nullptr),
      .attribute_buffer = alloc_lbuf("update_links.attribute_buffer"),
  };
  if (traversal.visited == nullptr)
    abort();

  recursively_update_links(&(MapLinkUpdateRequest){
      .context = context,
      .source = source,
      .location = location,
      .stats = stats,
      .traversal = &traversal,
  });
  free_buf(traversal.attribute_buffer);
  red_black_tree_destroy(traversal.visited);
}

void recursively_updatelinks(BtechContext *context, DbRef from, DbRef loc) {
  update_links(context, from, loc, nullptr);
}

void map_updatelinks(DbRef player, void *data, char *buffer [[maybe_unused]]) {
  BattleMap *map = data;
  MapLinkUpdateStats stats = {0};
  DbRef ourloc;

  ourloc = game_object_location(map->xcode.context->database, player);
  update_links(map->xcode.context, NOTHING, ourloc, &stats);
  notify_printf(btech_context_evaluation(map->xcode.context), player,
                "Updated %d BUILD objs, %d LEAVE objs, %d ENTRANCE objs; "
                "skipped %d link descents.",
                stats.builds, stats.leaves, stats.entrances,
                stats.skipped_links);
}

int map_linked(BtechContext *context, DbRef map_object) {
  BattleMap *map = btech_context_get_map(context, map_object);

  if (!map)
    return 0;
  return (first_mapobj(map, TYPE_LINKED)) ? 1 : 0;
}
