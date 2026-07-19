
/*
   p.map.obj.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:43 CET 1999 from map.obj.c */

#pragma once

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct GameDatabase GameDatabase;
typedef struct BtechContext BtechContext;

typedef struct StructureName {
  char text[MBUF_SIZE];
} StructureName;

/* map.obj.c */
mapobj *next_mapobj(mapobj *m);
mapobj *first_mapobj(MAP *map, int type);
int find_entrance(MAP *map, char dir, int *x, int *y);
StructureName structure_name(GameDatabase *database, mapobj *mapo);
mapobj *find_entrance_by_target(MAP *map, DbRef target);
mapobj *find_entrance_by_xy(MAP *map, int x, int y);
mapobj *find_mapobj(MAP *map, int x, int y, int type);
char find_decorations(MAP *map, int x, int y);
void del_mapobj(MAP *map, mapobj *mapob, int type, int zap);
void del_mapobjst(MAP *map, int type);
void del_mapobjs(MAP *map);
mapobj *add_mapobj(MAP *map, mapobj **to, mapobj *from, int flag);
int FindXEven(int wind, int x);
int FindYEven(int wind, int y);
int FindXOdd(int wind, int x);
int FindYOdd(int wind, int y);
void CheckForFire(MAP *map, int x[], int y[]);
void CheckForSmoke(MAP *map, int x[], int y[]);
void add_decoration(MAP *map, int x, int y, int type, char data, int flaggo);
void list_mapobjs(DbRef player, MAP *map);
void map_addfire(DbRef player, void *data, char *buffer);
void map_addsmoke(DbRef player, void *data, char *buffer);
void map_add_block(DbRef player, void *data, char *buffer);
int is_blocked_lz(MECH *mech, MAP *map, int x, int y);
void map_setlinked(DbRef player, void *data, char *buffer);
int mapobj_del(MAP *map, int x, int y, int tt);
void map_delobj(DbRef player, void *data, char *buffer);
int parse_coord(MAP *map, int dir, char *data, int *x, int *y);
void recursively_updatelinks(BtechContext *context, DbRef from, DbRef loc);
void map_updatelinks(DbRef player, void *data, char *buffer);
int map_linked(BtechContext *context, DbRef mapobj);
void possibly_start_building_regen(BtechContext *context, DbRef obj);
void hit_building(MECH *mech, int x, int y, int weapindx, int damage);
void fire_hex(MECH *mech, int x, int y, int meant);
void steppable_base_check(MECH *mech, int x, int y);
void show_building_in_hex(MECH *mech, int x, int y);
int obj_size(MAP *map);
int map_underlying_terrain(MAP *map, int x, int y);
int mech_underlying_terrain(MECH *mech);
