
/* Implements map bitfield storage and operations. */

#include "map.h"
#include "map_bits_api.h"
#include "map_obj_api.h"
#include "mux/support/checked_storage.h"

#include <stdlib.h>

constexpr unsigned char BIT_MINE = 1;
constexpr unsigned char BIT_HANGAR = 2;

static size_t map_bits_byte_count(int hex_count) {
  return (size_t)(hex_count / 4 + (hex_count % 4 ? 1 : 0));
}

static size_t map_bits_byte_index(int x) { return (size_t)x / 4; }

static unsigned char map_bits_mask(int x, unsigned char bits) {
  return (unsigned char)(bits << (2 * (x % 4)));
}

static unsigned char **map_bits_row_slot(unsigned char **bits, int height,
                                         int y) {
  if (height < 0 || y < 0)
    abort();
  return checked_storage_at(bits, (size_t)height, sizeof(*bits), (size_t)y);
}

static unsigned char *map_bits_byte(unsigned char **bits, const BattleMap *map,
                                    int x, int y) {
  if (x < 0)
    abort();
  unsigned char *row = *map_bits_row_slot(bits, map->map_height, y);
  return checked_storage_at(row, map_bits_byte_count(map->map_width),
                            sizeof(*row), map_bits_byte_index(x));
}

static MapObject **map_object_slot(BattleMap *map, int type) {
  if (type < 0)
    abort();
  return checked_storage_at(map->MapObject, NUM_MAPOBJTYPES,
                            sizeof(*map->MapObject), (size_t)type);
}

/* Main idea: By using 2 bits / hex in external array, we can _fast_
   figure out if a certain hex has mines / hangars or not. Downside is
   keeping the table up to date. */

static void create_if_neccessary(unsigned char **foo, BattleMap *map, int y) {
  int xs = map->map_width;

  unsigned char **row_slot = map_bits_row_slot(foo, map->map_height, y);
  if (!*row_slot) {
    *row_slot = calloc(map_bits_byte_count(xs), sizeof(**foo));
    if (*row_slot == nullptr)
      abort();
  }
}

static void map_bits_set(unsigned char **bits, BattleMap *map, int x, int y,
                         unsigned char value) {
  create_if_neccessary(bits, map, y);
  *map_bits_byte(bits, map, x, y) |= map_bits_mask(x, value);
}

static void map_bits_unset(unsigned char **bits, BattleMap *map, int x, int y,
                           unsigned char value) {
  if (*map_bits_row_slot(bits, map->map_height, y))
    *map_bits_byte(bits, map, x, y) &= (unsigned char)~map_bits_mask(x, value);
}

static bool map_bits_is_set(unsigned char **bits, BattleMap *map, int x, int y,
                            unsigned char value) {
  return *map_bits_byte(bits, map, x, y) & map_bits_mask(x, value);
}

/* Okay, now we got code to load / save the bits.. but what will we do with
   them? */

/* Nasty stuff starts here ;) */

static unsigned char **grab_us_an_array(BattleMap *map) {
  unsigned char **foo;
  MapObject foob;
  const size_t ys = (size_t)map->map_height;

  MapObject **bits_object = map_object_slot(map, TYPE_BITS);
  if (!*bits_object) {
    foo = calloc(ys, sizeof(*foo));
    if (foo == nullptr && ys > 0)
      abort();

    foob.datai = (long)((void *)foo);
    add_mapobj(map, bits_object, &foob, 0);
  } else
    foo = (unsigned char **)((void *)(*bits_object)->datai);
  return foo;
}

void set_hex_enterable(BattleMap *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  map_bits_set(foo, map, x, y, BIT_HANGAR);
}

void set_hex_mine(BattleMap *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  map_bits_set(foo, map, x, y, BIT_MINE);
}

void unset_hex_enterable(BattleMap *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  map_bits_unset(foo, map, x, y, BIT_HANGAR);
}

void unset_hex_mine(BattleMap *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  map_bits_unset(foo, map, x, y, BIT_MINE);
}

int is_mine_hex(BattleMap *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!first_mapobj(map, TYPE_BITS))
    return 0;
  foo = grab_us_an_array(map);
  if (!*map_bits_row_slot(foo, map->map_height, y))
    return 0;
  return map_bits_is_set(foo, map, x, y, BIT_MINE);
}

int is_hangar_hex(BattleMap *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!first_mapobj(map, TYPE_BITS))
    return 0;
  foo = grab_us_an_array(map);
  if (!*map_bits_row_slot(foo, map->map_height, y))
    return 0;
  return map_bits_is_set(foo, map, x, y, BIT_HANGAR);
}

void clear_hex_bits(BattleMap *map, int bits) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, j;
  unsigned char **foo;

  if (!first_mapobj(map, TYPE_BITS))
    return;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (*map_bits_row_slot(foo, ys, i))
      for (j = 0; j < xs; j++) {
        switch (bits) {
        case 1:
        case 2:
          if (map_bits_is_set(foo, map, j, i, (unsigned char)bits))
            map_bits_unset(foo, map, j, i, (unsigned char)bits);
          break;
        case 0:
          if (map_bits_is_set(foo, map, j, i, BIT_MINE))
            map_bits_unset(foo, map, j, i, BIT_MINE);
          if (map_bits_is_set(foo, map, j, i, BIT_HANGAR))
            map_bits_unset(foo, map, j, i, BIT_HANGAR);
          break;
        }
      }
}

int bit_size(BattleMap *map) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, s = 0;
  unsigned char **foo;

  if (!first_mapobj(map, TYPE_BITS))
    return 0;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (*map_bits_row_slot(foo, ys, i))
      s += map_bits_byte_count(xs);
  return s;
}
