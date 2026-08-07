
/*
 * $Id: map.bits.c,v 1.1.1.1 2005/01/11 21:18:07 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Tue Oct 22 16:32:09 1996 fingon
 * Last modified: Fri Jun 12 23:10:43 1998 fingon
 *
 */

#include "map.h"
#include "map_bits_api.h"
#include "map_obj_api.h"
#include "mux/network/mux_event_alloc.h"

constexpr unsigned char BIT_MINE = 1;
constexpr unsigned char BIT_HANGAR = 2;

static size_t map_bits_byte_count(int hex_count) {
  return (size_t)(hex_count / 4 + (hex_count % 4 ? 1 : 0));
}

static size_t map_bits_byte_index(int x) { return (size_t)x / 4; }

static unsigned char map_bits_mask(int x, unsigned char bits) {
  return (unsigned char)(bits << (2 * (x % 4)));
}

/* Main idea: By using 2 bits / hex in external array, we can _fast_
   figure out if a certain hex has mines / hangars or not. Downside is
   keeping the table up to date. */

static void create_if_neccessary(unsigned char **foo, BattleMap *map, int y) {
  int xs = map->map_width;

  if (!foo[y])
    Create(foo[y], unsigned char, map_bits_byte_count(xs));
}

static void map_bits_set(unsigned char **bits, BattleMap *map, int x, int y,
                         unsigned char value) {
  create_if_neccessary(bits, map, y);
  bits[y][map_bits_byte_index(x)] |= map_bits_mask(x, value);
}

static void map_bits_unset(unsigned char **bits, int x, int y,
                           unsigned char value) {
  if (bits[y])
    bits[y][map_bits_byte_index(x)] &= (unsigned char)~map_bits_mask(x, value);
}

static bool map_bits_is_set(unsigned char *const *bits, int x, int y,
                            unsigned char value) {
  return bits[y][map_bits_byte_index(x)] & map_bits_mask(x, value);
}

/* Okay, now we got code to load / save the bits.. but what will we do with
   them? */

/* Nasty stuff starts here ;) */

static unsigned char **grab_us_an_array(BattleMap *map) {
  unsigned char **foo;
  MapObject foob;
  int ys = map->map_height;

  if (!map->MapObject[TYPE_BITS]) {
    Create(foo, unsigned char *, ys);

    foob.datai = (long)((void *)foo);
    add_mapobj(map, &map->MapObject[TYPE_BITS], &foob, 0);
  } else
    foo = (unsigned char **)((void *)map->MapObject[TYPE_BITS]->datai);
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
  map_bits_unset(foo, x, y, BIT_HANGAR);
}

void unset_hex_mine(BattleMap *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  map_bits_unset(foo, x, y, BIT_MINE);
}

int is_mine_hex(BattleMap *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!map->MapObject[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  if (!foo[y])
    return 0;
  return map_bits_is_set(foo, x, y, BIT_MINE);
}

int is_hangar_hex(BattleMap *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!map->MapObject[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  if (!foo[y])
    return 0;
  return map_bits_is_set(foo, x, y, BIT_HANGAR);
}

void clear_hex_bits(BattleMap *map, int bits) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, j;
  unsigned char **foo;

  if (!map->MapObject[TYPE_BITS])
    return;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (foo[i])
      for (j = 0; j < xs; j++) {
        switch (bits) {
        case 1:
        case 2:
          if (map_bits_is_set(foo, j, i, (unsigned char)bits))
            map_bits_unset(foo, j, i, (unsigned char)bits);
          break;
        case 0:
          if (map_bits_is_set(foo, j, i, BIT_MINE))
            map_bits_unset(foo, j, i, BIT_MINE);
          if (map_bits_is_set(foo, j, i, BIT_HANGAR))
            map_bits_unset(foo, j, i, BIT_HANGAR);
          break;
        }
      }
}

int bit_size(BattleMap *map) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, s = 0;
  unsigned char **foo;

  if (!map->MapObject[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (foo[i])
      s += map_bits_byte_count(xs);
  return s;
}
