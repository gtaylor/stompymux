/* Declares the BattleTech map obj API. */

#pragma once

enum {
  MAP_DECORATION_TYPE_FIRE,
};

constexpr char MAP_DECORATION_FIRE_MARKER = '&';

#include "map.h"
#include "map_coordinates.h"
#include "map_effect_types.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct MuxEventScheduler MuxEventScheduler;
typedef struct GameDatabase GameDatabase;
typedef struct BtechContext BtechContext;

typedef struct StructureName {
  char text[MBUF_SIZE];
} StructureName;

typedef struct MapEntranceResult {
  bool found;
  MapHexPosition position;
} MapEntranceResult;

typedef struct MapObjectLookupRequest {
  BattleMap *map;
  MapHexPosition position;
  int type;
} MapObjectLookupRequest;

typedef struct MapObjectDeleteRequest {
  BattleMap *map;
  MapObject *object;
  int type;
  bool preserve_terrain;
  bool cancel_event;
} MapObjectDeleteRequest;

typedef struct MapDecorationRequest {
  BattleMap *map;
  MapHexPosition position;
  int type;
  char terrain_marker;
  int duration;
} MapDecorationRequest;

typedef struct BuildingHitRequest {
  Mech *mech;
  MapHexPosition position;
  int weapon_index;
  int damage;
} BuildingHitRequest;

/* map.obj.c */
MapObject *next_mapobj(MapObject *object);
MapObject *first_mapobj(BattleMap *map, int type);
MapEntranceResult find_entrance(BattleMap *map, char direction);
StructureName structure_name(GameDatabase *database, MapObject *mapo);
MapObject *find_entrance_by_target(BattleMap *map, DbRef target);
MapObject *find_entrance_by_xy(BattleMap *map, int x, int y);
MapObject *find_mapobj(const MapObjectLookupRequest *request);
char find_decorations(BattleMap *map, int x, int y);
void del_mapobj(const MapObjectDeleteRequest *request);
void del_mapobjst(BattleMap *map, int type);
void del_mapobjs(BattleMap *map);
MapObject *add_mapobj(BattleMap *map, MapObject **to, MapObject *from,
                      int flag);
MapObject *add_mapobj_to_type(BattleMap *map, int type, MapObject *from,
                              int flag);
void add_decoration(const MapDecorationRequest *request);
void list_mapobjs(DbRef player, BattleMap *map);
void map_addfire(DbRef player, void *data, char *buffer);
void map_addsmoke(DbRef player, void *data, char *buffer);
void map_add_block(DbRef player, void *data, char *buffer);
bool is_blocked_lz(Mech *mech, BattleMap *map, int x, int y);
void map_setlinked(DbRef player, void *data, char *buffer);
int map_objects_delete(const MapObjectLookupRequest *request);
void map_delobj(DbRef player, void *data, char *buffer);
bool parse_coord(BattleMap *map, int dir, char *data, int *x, int *y);
void recursively_updatelinks(BtechContext *context, DbRef from, DbRef loc);
void map_updatelinks(DbRef player, void *data, char *buffer);
int map_linked(BtechContext *context, DbRef map_object);
void possibly_start_building_regen(BtechContext *context, DbRef obj);
void hit_building(const BuildingHitRequest *request);
void fire_hex(const TerrainHexEffectRequest *request);
void steppable_base_check(Mech *mech, int x, int y);
void show_building_in_hex(Mech *mech, int x, int y);
int obj_size(BattleMap *map);
int map_underlying_terrain(BattleMap *map, int x, int y);
int mech_underlying_terrain(Mech *mech);
