/* Implements BattleTech combat mechanics for artillery. */

/*
   Artillery code for
   - standard rounds (damage to target hex, damage/2 to neighbor hexes)
   - smoke rounds (to be implemented)
   - fascam rounds (to be implemented)
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "artillery.h"
#include "artillery_api.h"
#include "btconfig.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/network/mux_event.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void artillery_hit(artillery_shot *s);

static const char *artillery_type(artillery_shot *s) {
  if (s->type == CL_ARROW || s->type == IS_ARROW)
    return "a missile";
  return "a round";
}

typedef struct ArtilleryDirection {
  int dir;
  const char *desc;
} ArtilleryDirection;

static const ArtilleryDirection arty_dirs[] = {
    {0, "north"},       {60, "northeast"},  {90, "east"},
    {120, "southeast"}, {180, "south"},     {240, "southwest"},
    {270, "west"},      {300, "northwest"}, {0, nullptr}};

static const ArtilleryDirection *artillery_direction_at(size_t index) {
  return checked_storage_at_const(arty_dirs,
                                  sizeof(arty_dirs) / sizeof(*arty_dirs),
                                  sizeof(*arty_dirs), index);
}

static const char *artillery_direction(artillery_shot *s) {
  float fx, fy, tx, ty;
  int b, d, i, best = -1, bestd = 0;

  MapCoordToRealCoord(s->from_x, s->from_y, &fx, &fy);
  MapCoordToRealCoord(s->to_x, s->to_y, &tx, &ty);
  b = FindBearing(fx, fy, tx, ty);
  for (i = 0; artillery_direction_at((size_t)i)->desc; i++) {
    d = abs(b - artillery_direction_at((size_t)i)->dir);
    if (best < 0 || d < bestd) {
      best = i;
      bestd = d;
    }
  }
  if (best < 0)
    return "Invalid";
  return artillery_direction_at((size_t)best)->desc;
}

int artillery_round_flight_time(float fx, float fy, float tx, float ty) {
  const float flight_time = FindHexRange(fx, fy, tx, ty) / ARTY_SPEED;
  const int delay = MAX(ARTILLERY_MINIMUM_FLIGHT, (int)flight_time);

  /* XXX Different weapons, diff. speed? */
  return delay;
}

static void artillery_hit_event(MuxEvent *e) {
  artillery_shot *s = (artillery_shot *)e->data;

  artillery_hit(s);
}

void artillery_shoot(Mech *mech, int targx, int targy, int windex, int wmode,
                     int ishit) {
  struct ArtilleryShot *s;
  float fx, fy, tx, ty;

  Create(s, artillery_shot, 1);
  s->from_x = mech_position_x(mech);
  s->from_y = mech_position_y(mech);
  s->to_x = targx;
  s->to_y = targy;
  s->type = windex;
  s->mode = wmode;
  s->ishit = ishit;
  s->shooter = mech_dbref(mech);
  s->map = mech_map_dbref(mech);
  s->context = mech_context(mech);
  mech_los_broadcast(mech, tprintf("shoots %s towards the %s!",
                                   artillery_type(s), artillery_direction(s)));
  MapCoordToRealCoord(s->from_x, s->from_y, &fx, &fy);
  MapCoordToRealCoord(s->to_x, s->to_y, &tx, &ty);
  btech_context_owned_event_schedule(
      mech_context(mech), s, EVENT_DHIT, artillery_hit_event,
      artillery_round_flight_time(fx, fy, tx, ty), 0);
}

static int blast_arcf(float fx, float fy, Mech *mech) {
  int b, dir;

  b = FindBearing(mech_position_real_x(mech), mech_position_real_y(mech), fx,
                  fy);
  dir = AcceptableDegree(b - mech_heading_degrees(mech));
  if (dir > 120 && dir < 240)
    return BACK;
  if (dir > 300 || dir < 60)
    return FRONT;
  if (dir > 180)
    return LEFTSIDE;
  return RIGHTSIDE;
}

#define TABLE_GEN 0
#define TABLE_PUNCH 1
#define TABLE_KICK 2

void blast_hit_hexf(BattleMap *map, int dam, int singlehitsize, int heatdam,
                    float fx, float fy, float tfx, float tfy, const char *tomsg,
                    const char *otmsg, int table, int safeup, int safedown,
                    int isunderwater) {
  Mech *tempMech;
  int loop;
  int isrear = 0, iscritical = 0, hitloc;
  int damleft, arc, ndam;
  int ground_zero;
  short tx, ty;

  /* Not on a map so just return */
  if (!map)
    return;

  RealCoordToMapCoord(&tx, &ty, fx, fy);
  if (tx < 0 || ty < 0 || tx >= map->map_width || ty >= map->map_height)
    return;
  if (!tomsg || !otmsg)
    return;
  if (isunderwater)
    ground_zero = battle_map_hex_elevation(map, tx, ty);
  else
    ground_zero = MAX(0, battle_map_hex_elevation(map, tx, ty));

  for (loop = 0; loop < battle_map_unit_count(map); loop++) {
    const DbRef unit = battle_map_unit_dbref(map, loop);
    if (unit >= 0) {
      tempMech = btech_context_get_mech(battle_map_context(map), unit);
      if (!tempMech)
        continue;
      if (mech_position_x(tempMech) != tx || mech_position_y(tempMech) != ty)
        continue;
      /* Far too high.. */
      if (mech_position_z(tempMech) >= (safeup + ground_zero))
        continue;
      /* Far too below (underwater, mostly) */
      if (/* MechTerrain(tempMech) == WATER &&  */
          mech_position_z(tempMech) <= (ground_zero - safedown))
        continue;
      mech_los_broadcast(tempMech, otmsg);
      mech_notify(tempMech, MECHALL, tomsg);
      arc = blast_arcf(tfx, tfy, tempMech);

      if (arc == BACK)
        isrear = 1;
      damleft = dam;

      while (damleft > 0) {
        if (singlehitsize <= damleft)
          ndam = singlehitsize;
        else
          ndam = damleft;

        damleft -= ndam;

        switch (table) {
        case TABLE_PUNCH:
          if (mech_class(tempMech) != CLASS_MECH) {
            hitloc = mech_hit_location(tempMech, arc, &iscritical, &isrear);
          } else {
            hitloc = mech_punch_hit_location(tempMech, arc);
          }
          break;
        case TABLE_KICK:
          if (mech_class(tempMech) != CLASS_MECH) {
            hitloc = mech_hit_location(tempMech, arc, &iscritical, &isrear);
          } else {
            hitloc = mech_kick_hit_location(tempMech, arc);
          }
          break;
        default:
          hitloc = mech_hit_location(tempMech, arc, &iscritical, &isrear);
        }

        DamageMech(tempMech, tempMech, 0, -1, hitloc, isrear, iscritical, ndam,
                   0, -1, 0, -1, 0, 0);
      }
      mech_heat_effect_apply(nullptr, tempMech, heatdam, false);
    }
  }
}

void blast_hit_hex(BattleMap *map, int dam, int singlehitsize, int heatdam,
                   int fx, int fy, int tx, int ty, const char *tomsg,
                   const char *otmsg, int table, int safeup, int safedown,
                   int isunderwater) {
  float ftx, fty;
  float ffx, ffy;

  MapCoordToRealCoord(tx, ty, &ftx, &fty);
  MapCoordToRealCoord(fx, fy, &ffx, &ffy);
  blast_hit_hexf(map, dam, singlehitsize, heatdam, ffx, ffy, ftx, fty, tomsg,
                 otmsg, table, safeup, safedown, isunderwater);
}

void blast_hit_hexesf(BattleMap *map, int dam, int singlehitsize, int heatdam,
                      float fx, float fy, float ftx, float fty,
                      const char *tomsg, const char *otmsg, const char *tomsg1,
                      const char *otmsg1, int table, int safeup, int safedown,
                      int isunderwater, int doneighbors) {
  int x1, y1, x2, y2;
  int dm;
  short tx, ty;
  float hx, hy;
  float t = FindXYRange(fx, fy, ftx, fty);

  dm = MAX(1, (int)t + 1);
  blast_hit_hexf(map, dam / dm, singlehitsize, heatdam / dm, fx, fy, ftx, fty,
                 tomsg, otmsg, table, safeup, safedown, isunderwater);
  if (!doneighbors)
    return;
  RealCoordToMapCoord(&tx, &ty, fx, fy);
  for (x1 = (tx - doneighbors); x1 <= (tx + doneighbors); x1++)
    for (y1 = (ty - doneighbors); y1 <= (ty + doneighbors); y1++) {
      int spot;

      if ((dm = MyHexDist(tx, ty, x1, y1, 0)) > doneighbors)
        continue;
      if ((tx == x1) && (ty == y1))
        continue;
      x2 = BOUNDED(0, x1, map->map_width - 1);
      y2 = BOUNDED(0, y1, map->map_height - 1);
      if (x1 != x2 || y1 != y2)
        continue;
      spot = (x1 == tx && y1 == ty);
      MapCoordToRealCoord(x1, y1, &hx, &hy);
      dm++;
      if (!(dam / dm))
        continue;
      blast_hit_hexf(map, dam / dm, singlehitsize, heatdam / dm, hx, hy, ftx,
                     fty, spot ? tomsg : tomsg1, spot ? otmsg : otmsg1, table,
                     safeup, safedown, isunderwater);

      /*
       * Added in burning woods when a mech's engine goes nova
       *
       * -Kipsta
       * 8/4/99
       */

      switch (map_real_terrain_get(map, x1, y1)) {
      case LIGHT_FOREST:
      case HEAVY_FOREST:
        if (!find_decorations(map, x1, y1)) {
          add_decoration(
              map, x1, y1, TYPE_FIRE, FIRE,
              btech_random_range_int(battle_map_context(map), 60, 180));
        }

        break;
      }
    }
}

void blast_hit_hexes(BattleMap *map, int dam, int singlehitsize, int heatdam,
                     int tx, int ty, const char *tomsg, const char *otmsg,
                     const char *tomsg1, const char *otmsg1, int table,
                     int safeup, int safedown, int isunderwater,
                     int doneighbors) {
  float fx, fy;

  MapCoordToRealCoord(tx, ty, &fx, &fy);
  blast_hit_hexesf(map, dam, singlehitsize, heatdam, fx, fy, fx, fy, tomsg,
                   otmsg, tomsg1, otmsg1, table, safeup, safedown, isunderwater,
                   doneighbors);
}

static void artillery_hit_hex(BattleMap *map, artillery_shot *s, int type,
                              int mode, int dam, int tx, int ty, int isdirect) {
  char buf1[LBUF_SIZE];
  char buf2[LBUF_SIZE];

  /* Safety check -- shouldn't happen */
  if (tx < 0 || tx >= map->map_width || ty < 0 || ty >= map->map_height)
    return;

  if ((mode & SMOKE_MODE)) {
    /* Add smoke */
    add_decoration(map, tx, ty, TYPE_SMOKE, SMOKE,
                   btech_random_range_int(battle_map_context(map), 90, 150));
    return;
  }
  if (mode & MINE_MODE) {
    mine_field_add(map, tx, ty, dam);
    return;
  }
  if (!(mode & CLUSTER_MODE)) {
    if (isdirect)
      snprintf(buf1, LBUF_SIZE, "receives a direct hit!");
    else
      snprintf(buf1, LBUF_SIZE, "is hit by fragments!");
    if (isdirect)
      snprintf(buf2, LBUF_SIZE, "You receive a direct hit!");
    else
      snprintf(buf2, LBUF_SIZE, "You are hit by fragments!");
  } else {
    if (dam > 2) {
      strcpy(buf1, "is hit by bomblets!");
      strcpy(buf2, "You are hit by bomblets!");
    } else {
      strcpy(buf1, "is hit by a bomblet!");
      strcpy(buf2, "You are hit by a bomblet!");
    }
  }
  blast_hit_hex(map, dam, (mode & CLUSTER_MODE) ? 2 : 5, 0, tx, ty, tx, ty,
                buf2, buf1, (mode & CLUSTER_MODE) ? TABLE_PUNCH : TABLE_GEN, 10,
                4, 0);
}

typedef struct ArtilleryNeighborHit ArtilleryNeighborHit;
struct ArtilleryNeighborHit {
  artillery_shot *shot;
  int type;
  int mode;
  int damage;
};

static void artillery_hit_neighbors_callback(BattleMap *map, int x, int y,
                                             void *context) {
  const ArtilleryNeighborHit *hit = context;

  artillery_hit_hex(map, hit->shot, hit->type, hit->mode, hit->damage, x, y, 0);
}

static void artillery_hit_neighbors(BattleMap *map, artillery_shot *s, int type,
                                    int mode, int dam, int tx, int ty) {
  ArtilleryNeighborHit hit = {
      .shot = s,
      .type = type,
      .mode = mode,
      .damage = dam,
  };

  visit_neighbor_hexes(map, tx, ty, artillery_hit_neighbors_callback, &hit);
}

static void artillery_cluster_hit(BattleMap *map, artillery_shot *s, int type,
                                  int mode, int dam, int tx, int ty) {
  /* Main idea: Pick <dam/2> bombs of 2pts each, and scatter 'em
     over 5x5 area with weighted numbers */
  int xd, yd, x, y;
  int i;

  typedef struct ArtilleryTargetGrid {
    int cells[25];
  } ArtilleryTargetGrid;
  ArtilleryTargetGrid targets;
  int d;

  memset(&targets, 0, sizeof(targets));
  for (i = 0; i < dam; i++) {
    do {
      xd = btech_random_range_int(battle_map_context(map), -2, 0) +
           btech_random_range_int(battle_map_context(map), 0, 2);
      yd = btech_random_range_int(battle_map_context(map), -2, 0) +
           btech_random_range_int(battle_map_context(map), 0, 2);
      x = tx + xd;
      y = ty + yd;
    } while (x < 0 || x >= map->map_width || y < 0 || y >= map->map_height);
    /* Whee.. it's time to drop a bomb to the hex */
    int *target = checked_storage_at(targets.cells, 25, sizeof(*targets.cells),
                                     (size_t)((xd + 2) * 5 + yd + 2));
    (*target)++;
  }
  for (xd = 0; xd < 5; xd++)
    for (yd = 0; yd < 5; yd++)
      if ((d = *(const int *)checked_storage_at_const(targets.cells, 25,
                                                      sizeof(*targets.cells),
                                                      (size_t)(xd * 5 + yd))))
        artillery_hit_hex(map, s, type, mode, d * 2, xd + tx - 2, yd + ty - 2,
                          1);
}

void artillery_friendly_adjustment(DbRef mechnum, BattleMap *map, int x,
                                   int y) {
  Mech *mech;
  Mech *spotter;
  Mech *tempMech = nullptr;

  if (!(mech = btech_context_get_mech(battle_map_context(map), mechnum)))
    return;
  /* Ok.. we've a valid guy */
  spotter =
      btech_context_get_mech(battle_map_context(map), mech_spotter_dbref(mech));
  if (!((mech_target_hex_x(mech) == x && mech_target_hex_y(mech) == y) ||
        (spotter &&
         (mech_target_hex_x(spotter) == x && mech_target_hex_y(spotter) == y))))
    return;
  /* Ok.. we've a valid target to adjust fire on */
  /* Now, see if we've any friendlies in LOS.. NOTE: FRIENDLIES ;-) */
  if (spotter) {
    if (MechSeesHex(spotter, map, x, y))
      tempMech = spotter;
  } else
    tempMech = find_mech_in_hex(mech, map, x, y, 2);
  if (!tempMech)
    return;
  if (!mech_is_started(tempMech) || !mech_is_started(mech))
    return;
  if (spotter) {
    mech_printf(mech, MECHSTARTED,
                "%s sent you some trajectory-correction data.",
                mech_to_mech_display_id(mech, tempMech).text);
    mech_printf(tempMech, MECHSTARTED,
                "You provide %s with information about the miss.",
                mech_to_mech_display_id(tempMech, mech).text);
  }
  mech_fire_adjustment_increment(mech);
}

static void artillery_hit(artillery_shot *s) {
  /* First, we figure where it exactly hits. Our first-hand information
     is only whether it hits or not, not _where_ it hits */
  float dir;
  int di;
  int dist;
  int weight;
  BattleMap *map = btech_context_get_map(s->context, s->map);
  int original_x = 0, original_y = 0;
  int dam = weapon_catalogue_damage(s->type);

  if (!map)
    return;
  if (!s->ishit) {
    /* Shit! We missed target ;-) */
    /* Time to calculate a new target hex */
    di = btech_random_range_int(battle_map_context(map), 0, 359);
    dir = (float)di * (float)M_PI / 180.0F;
    dist = btech_random_range_int(battle_map_context(map), 2, 7);
    weight = 100 * (dist * 6) / ((dist * 6 + map->windspeed));
    di = (di * weight + map->winddir * (100 - weight)) / 100;
    dist = (dist * weight + (map->windspeed / 6) * (100 - weight)) / 100;
    original_x = s->to_x;
    original_y = s->to_y;
    s->to_x += (int)((float)dist * cosf(dir));
    s->to_y += (int)((float)dist * sinf(dir));
    s->to_x = BOUNDED(0, s->to_x, map->map_width - 1);
    s->to_y = BOUNDED(0, s->to_y, map->map_height - 1);
    /* Time to calculate if any friendlies have LOS to hex,
       and if so, adjust fire adjustment unless you lack information /
       have changed target */
  }
  /* It's time to run for your lives, lil' ones ;-) */
  if (!(s->mode & ARTILLERY_MODES))
    HexLOSBroadcast(
        map, s->to_x, s->to_y,
        tprintf("%s fire hits $H!",
                checked_string_suffix(weapon_catalogue_name(s->type), 3)));
  else if (s->mode & CLUSTER_MODE)
    HexLOSBroadcast(map, s->to_x, s->to_y,
                    "A rain of small bomblets hits $H's surroundings!");
  else if (s->mode & MINE_MODE)
    HexLOSBroadcast(map, s->to_x, s->to_y, "A rain of small bomblets hits $H!");
  else if (s->mode & SMOKE_MODE)
    HexLOSBroadcast(
        map, s->to_x, s->to_y,
        tprintf("A %s %s hits $h, and smoke starts to billow!",
                checked_string_suffix(weapon_catalogue_name(s->type), 3),
                checked_string_suffix(artillery_type(s), 2)));

  /* Basic theory:
     - smoke / ordinary rounds are spread with the ordinary functions
     - mines are otherwise ordinary except no hitting of neighbor hexes
     - cluster bombs are special
   */
  if (!(s->mode & CLUSTER_MODE)) {
    /* Enjoy ourselves in all neighbor hexes, too */
    artillery_hit_hex(map, s, s->type, s->mode, dam, s->to_x, s->to_y, 1);
    if (!(s->mode & MINE_MODE))
      artillery_hit_neighbors(map, s, s->type, s->mode, dam / 2, s->to_x,
                              s->to_y);
  } else
    artillery_cluster_hit(map, s, s->type, s->mode, dam, s->to_x, s->to_y);
  if (!s->ishit)
    artillery_friendly_adjustment(s->shooter, map, original_x, original_y);
}
