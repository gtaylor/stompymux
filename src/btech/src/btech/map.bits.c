
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

#include "mech.h"
#include "mux/network/mux_event_alloc.h"

#define realnum(x) ((x) / 4 + ((x) % 4 ? 1 : 0))
#define boffs(x) (2 * ((x) % 4))
#define boffsbit(x, n) ((1 << boffs(x)) * n)
#define btsetbit(arr, x, y, n)                                                 \
  create_if_neccessary(arr, map, y);                                           \
  arr[y][realnum(x)] |= boffsbit(x, n)
#define btunsetbit(arr, x, y, n)                                               \
  if (arr[y])                                                                  \
  arr[y][realnum(x)] &= ~(boffsbit(x, n))
#define btissetbit(arr, x, y, n) (arr[y][realnum(x)] & boffsbit(x, n))

#define BIT_MINE 1
#define BIT_HANGAR 2

/* Main idea: By using 2 bits / hex in external array, we can _fast_
   figure out if a certain hex has mines / hangars or not. Downside is
   keeping the table up to date. */

static void create_if_neccessary(unsigned char **foo, MAP *map, int y) {
  int xs = map->map_width;

  if (!foo[y])
    Create(foo[y], unsigned char, realnum(xs));
}

/* Okay, now we got code to load / save the bits.. but what will we do with
   them? */

/* Nasty stuff starts here ;) */

static unsigned char **grab_us_an_array(MAP *map) {
  unsigned char **foo;
  mapobj foob;
  int ys = map->map_height;

  if (!map->mapobj[TYPE_BITS]) {
    Create(foo, unsigned char *, ys);

    foob.datai = (long)((void *)foo);
    add_mapobj(map, &map->mapobj[TYPE_BITS], &foob, 0);
  } else
    foo = (unsigned char **)((void *)map->mapobj[TYPE_BITS]->datai);
  return foo;
}

void set_hex_enterable(MAP *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  btsetbit(foo, x, y, BIT_HANGAR);
}

void set_hex_mine(MAP *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  btsetbit(foo, x, y, BIT_MINE);
}

void unset_hex_enterable(MAP *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  btunsetbit(foo, x, y, BIT_HANGAR);
}

void unset_hex_mine(MAP *map, int x, int y) {
  unsigned char **foo;

  foo = grab_us_an_array(map);
  btunsetbit(foo, x, y, BIT_MINE);
}

int is_mine_hex(MAP *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!map->mapobj[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  if (!foo[y])
    return 0;
  return (btissetbit(foo, x, y, BIT_MINE));
}

int is_hangar_hex(MAP *map, int x, int y) {
  unsigned char **foo;

  if (!map)
    return 0;
  if (!map->mapobj[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  if (!foo[y])
    return 0;
  return (btissetbit(foo, x, y, BIT_HANGAR));
}

void clear_hex_bits(MAP *map, int bits) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, j;
  unsigned char **foo;

  if (!map->mapobj[TYPE_BITS])
    return;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (foo[i])
      for (j = 0; j < xs; j++) {
        switch (bits) {
        case 1:
        case 2:
          if (btissetbit(foo, j, i, bits))
            btunsetbit(foo, j, i, bits);
          break;
        case 0:
          if (btissetbit(foo, j, i, 1))
            btunsetbit(foo, j, i, 1);
          if (btissetbit(foo, j, i, 2))
            btunsetbit(foo, j, i, 2);
          break;
        }
      }
}

int bit_size(MAP *map) {
  int xs = map->map_width;
  int ys = map->map_height;
  int i, s = 0;
  unsigned char **foo;

  if (!map->mapobj[TYPE_BITS])
    return 0;
  foo = grab_us_an_array(map);
  for (i = 0; i < ys; i++)
    if (foo[i])
      s += realnum(xs);
  return s;
}
