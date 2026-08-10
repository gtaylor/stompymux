/* Structure definitions and what not for the maps for the mechs. */
#pragma once

#include "mux/server/platform.h"
#include "special_object.h"

constexpr int MAX_MECHS_PER_MAP = 250;

/* Map links */
constexpr int MAP_UP = 0;
constexpr int MAP_DOWN = 1;
constexpr int MAP_RIGHT = 2;
constexpr int MAP_LEFT = 3;

/* Map size */
constexpr int MAPX = 1000;
constexpr int MAPY = 1000;
constexpr int MAP_NAME_SIZE = 30;
constexpr int NUM_MAP_LINKS = 4;
constexpr int DEFAULT_MAP_WIDTH = 21;
constexpr int DEFAULT_MAP_HEIGHT = 11;
constexpr int MAP_DISPLAY_WIDTH = 21;
constexpr int MAP_DISPLAY_HEIGHT = 14;
constexpr int MAX_ELEV = 9;

/* Terrain constants */
constexpr char GRASSLAND = ' ';
constexpr char HEAVY_FOREST = '"';
constexpr char LIGHT_FOREST = '`';
constexpr char WATER = '~';
constexpr char HIGHWATER = '?';
constexpr char ROUGH = '%';
constexpr char MOUNTAINS = '^';
constexpr char ROAD = '#';
constexpr char BUILDING = '@';
constexpr char FIRE = '&';
constexpr char TFIRE = '>';
constexpr char SMOKE = ':';
constexpr char WALL = '=';
constexpr char DESERT = '}';
constexpr char BRIDGE = '/';
constexpr char SNOW = '+';
constexpr char ICE = '-';
constexpr char UNKNOWN_TERRAIN = '$';

/*
 * Various Map flags, for use for setting different affects on the map
 */
/* (a) We got mapobjs */
constexpr int MAPFLAG_MAPO = 1;
/* (b) We're using special rules - gravity/temp */
constexpr int MAPFLAG_SPEC = 2;
/* (c) We're in vacuum */
constexpr int MAPFLAG_VACUUM = 4;
/* (d) We have eternal fires */
constexpr int MAPFLAG_FIRES = 8;
/* (e) We're underground. No ejecting, jumping, VTOL taking off */
constexpr int MAPFLAG_UNDERGROUND = 16;
/* (f) We can't see map beyond sensor range */
constexpr int MAPFLAG_DARK = 32;
/* (g) We can't destroy bridges on this map */
constexpr int MAPFLAG_BRIDGESCS = 64;
/* (h) We shouldn't convert roads into bridges */
constexpr int MAPFLAG_NOBRIDGIFY = 128;
/* (i) We can't shoot friendlies AT ALL on this map */
constexpr int MAPFLAG_NOFRIENDLYFIRE = 256;
/* (j) No Physicals allowed on this map (Mainly for Clan) */
constexpr int MAPFLAG_NOPHYSICALS = 512;

/* Fire - datas = counter until next spread, datac = stuff to burn */
constexpr int TYPE_FIRE = 0;
/* Smoke - datas = time until it gets lost */
constexpr int TYPE_SMOKE = 1;
/* Decoration, like those 2 previous ones. obj = obj# of DS it is related to,
   datac = char it replaced */
constexpr int TYPE_DEC = 2;
constexpr int TYPE_LAST_DEC = 2;
/* datac = type, datas = damage it causes, payload.scalar = extra */
constexpr int TYPE_MINE = 3;
/* Building obj=# of the internal map */
constexpr int TYPE_BUILD = 4;
/* Reference to what happens when U leave ; obj=# of new map */
constexpr int TYPE_LEAVE = 5;
/* datac = dir of entry (0=dontcare), x/y */
constexpr int TYPE_ENTRANCE = 6;
/* If this exists, we got a maplink propably */
constexpr int TYPE_LINKED = 7;
/* hangar / mine bit array, if any (in payload.bits) */
constexpr int TYPE_BITS = 8;
/* Land-block */
constexpr int TYPE_B_LZ = 9;
/* Used to create a linked list on each BattleMap per MapObject type*/
constexpr int NUM_MAPOBJTYPES = 10;

/* Externally CS */
constexpr int BUILDFLAG_CS = 1;
/* Internally CS */
constexpr int BUILDFLAG_CSI = 2;
/* DontShowStep when someone steps on the base */
constexpr int BUILDFLAG_DSS = 4;
/* No way to break in */
constexpr int BUILDFLAG_NOB = 8;
/* Really hidden */
constexpr int BUILDFLAG_HID = 16;

/*
 * A map-local feature, such as a terrain decoration, mine, building link, or
 * entrance. BattleMap stores these in separate linked lists by TYPE_* value.
 */
typedef struct MapObject {
  /* Related database object, such as a linked building or destination map. */
  DbRef obj;
  /* Next feature in this map object's TYPE_* linked list. */
  struct MapObject *next;
  /* Hex column containing this feature. */
  short x;
  /* Hex row containing this feature. */
  short y;
  /* Legacy type discriminator; the containing TYPE_* list is authoritative. */
  char type;
  /* Type-specific integer data, such as original terrain or entrance
     direction. */
  int datac;
  /* Type-specific short data, such as a duration or mine damage. */
  short datas;
  /* TYPE_BITS owns bits; every persisted map-object type uses scalar. */
  union {
    long scalar;
    unsigned char **bits;
  } payload;
} MapObject;

/* mech has moved since last LOS update */
constexpr int MECHMAPFLAG_MOVED = 1;

typedef struct BattleMap {
  /* XCODE base class field */
  BtechSpecialObject xcode;
  /* My dbref */
  DbRef mynum;
  /* The map */
  unsigned char **map;
  char mapname[MAP_NAME_SIZE + 1];

  /* Width of map <MAPX */
  short map_width;
  /* Height of map */
  short map_height;

  /* Temperature, in celsius degrees */
  char temp;
  /* Gravity, if any ; in 1/100 G's */
  unsigned char grav;
  short cloudbase;
  char unused_char;
  /* Visibility on the map, used as base for most sensor types */
  char mapvis;
  /* maximum visibility (usually mapvis * n) */
  short maxvis;
  char maplight;
  short winddir, windspeed;

  /* Now, da wicked stuff */
  int flags;

  MapObject *MapObject[NUM_MAPOBJTYPES];
  short cf, cfmax;
  DbRef onmap;
  char buildflag;

  /* First free on da map */
  int first_free;
  /* Allocated occupancy/LOS matrix dimension. */
  int dynamic_size;
  /* Mechs on the map */
  DbRef *mechsOnMap;
  /* Line of sight info */
  unsigned short **LOSinfo;

  /* 1 = mech has moved recently
     2 = mech has possible-LOS event ongoing */
  char *mechflags;
  /* Cheat to prevent idle CPU hoggage */
  short moves;
  short movemod;
  int sensorflags;
  /* Amount of CF to possibly regen per cycle */
  int regen_factor;
} BattleMap;

/* Used by navigate_sketch_map */
constexpr int NAVIGATE_LINES = 13;

extern void newfreemap(DbRef key, void **data,
                       BtechSpecialLifecycleOperation operation);
extern void map_update(DbRef obj, void *data);
