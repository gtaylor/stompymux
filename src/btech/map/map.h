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
#define GRASSLAND ' '
#define HEAVY_FOREST '"'
#define LIGHT_FOREST '`'
#define WATER '~'
#define HIGHWATER '?'
#define ROUGH '%'
#define MOUNTAINS '^'
#define ROAD '#'
#define BUILDING '@'
#define FIRE '&'
#define TFIRE '>'
#define SMOKE ':'
#define WALL '='
#define DESERT '}'
#define BRIDGE '/'
#define SNOW '+'
#define ICE '-'
#define UNKNOWN_TERRAIN '$'

/*
 * Various Map flags, for use for setting different affects on the map
 */
constexpr int MAPFLAG_MAPO = 1; /* (a) We got mapobjs */
constexpr int MAPFLAG_SPEC =
    2; /* (b) We're using special rules - gravity/temp */
constexpr int MAPFLAG_VACUUM = 4; /* (c) We're in vacuum */
constexpr int MAPFLAG_FIRES = 8;  /* (d) We have eternal fires */
#define MAPFLAG_UNDERGROUND                                                    \
  16 /* (e) We're underground. No ejecting, jumping,                           \
        VTOL taking off */
constexpr int MAPFLAG_DARK = 32; /* (f) We can't see map beyond sensor range */
constexpr int MAPFLAG_BRIDGESCS =
    64; /* (g) We can't destroy bridges on this map */
#define MAPFLAG_NOBRIDGIFY                                                     \
  128 /* (h) We shouldn't convert roads into bridges                           \
       */
#define MAPFLAG_NOFRIENDLYFIRE                                                 \
  256 /* (i) We can't shoot friendlies AT ALL on this map */
#define MAPFLAG_NOPHYSICALS                                                    \
  512 /* (j) No Physicals allowed on this map (Mainly for Clan) */

#define TYPE_FIRE                                                              \
  0 /* Fire - datas = counter until next spread, datac = stuff to burn */
constexpr int TYPE_SMOKE = 1; /* Smoke - datas = time until it gets lost */
#define TYPE_DEC                                                               \
  2 /* Decoration, like those 2 previous ones. obj = obj# of DS it is related  \
       to, datac = char it replaced */
constexpr int TYPE_LAST_DEC = 2;
#define TYPE_MINE                                                              \
  3 /* datac = type, datas = damage it causes, datai = extra                   \
     */
constexpr int TYPE_BUILD = 4; /* Building obj=# of the internal map */
#define TYPE_LEAVE                                                             \
  5 /* Reference to what happens when U leave ; obj=# of new map */
constexpr int TYPE_ENTRANCE = 6; /* datac = dir of entry (0=dontcare), x/y */

constexpr int TYPE_LINKED = 7; /* If this exists, we got a maplink propably */
constexpr int TYPE_BITS = 8;   /* hangar / mine bit array, if any (in datai) */
constexpr int TYPE_B_LZ = 9;   /* Land-block */
constexpr int NUM_MAPOBJTYPES = 10;

constexpr int BUILDFLAG_CS = 1;  /* Externally CS */
constexpr int BUILDFLAG_CSI = 2; /* Internally CS */
constexpr int BUILDFLAG_DSS =
    4; /* DontShowStep when someone steps on the base */
constexpr int BUILDFLAG_NOB = 8;  /* No way to break in */
constexpr int BUILDFLAG_HID = 16; /* Really hidden */

typedef struct MapObject {
  short x, y;
  DbRef obj;
  char type;
  int datac;
  short datas;
  long datai;
  struct MapObject *next;
} MapObject;

constexpr int MECHMAPFLAG_MOVED = 1; /* mech has moved since last LOS update */

typedef struct BattleMap {
  BtechSpecialObject xcode; /* XCODE base class field */

  DbRef mynum;         /* My dbref */
  unsigned char **map; /* The map */
  char mapname[MAP_NAME_SIZE + 1];

  short map_width;  /* Width of map <MAPX  */
  short map_height; /* Height of map */

  char temp;          /* Temperature, in celsius degrees */
  unsigned char grav; /* Gravity, if any ; in 1/100 G's */
  short cloudbase;
  char unused_char;
  char mapvis;  /* Visibility on the map, used as base
                   for most sensor types */
  short maxvis; /* maximum visibility (usually mapvis * n) */
  char maplight;
  short winddir, windspeed;

  /* Now, da wicked stuff */
  int flags;

  MapObject *MapObject[NUM_MAPOBJTYPES];
  short cf, cfmax;
  DbRef onmap;
  char buildflag;

  int first_free;           /* First free on da map */
  int dynamic_size;         /* Allocated occupancy/LOS matrix dimension. */
  DbRef *mechsOnMap;        /* Mechs on the map */
  unsigned short **LOSinfo; /* Line of sight info */

  /* 1 = mech has moved recently
     2 = mech has possible-LOS event ongoing */
  char *mechflags;
  short moves; /* Cheat to prevent idle CPU hoggage */
  short movemod;
  int sensorflags;
  int regen_factor; /* Amount of CF to possibly regen per cycle */
} BattleMap;

/* Used by navigate_sketch_map */
constexpr int NAVIGATE_LINES = 13;

extern void newfreemap(DbRef key, void **data,
                       BtechSpecialLifecycleOperation operation);
extern void map_update(DbRef obj, void *data);
