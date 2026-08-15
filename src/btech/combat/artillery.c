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
#include "map_coordinates.h"
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
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void artillery_hit(ArtilleryShot *s);

static const char *artillery_type(ArtilleryShot *s) {
  if (s->type == CL_ARROW || s->type == IS_ARROW)
    return "a missile";
  return "a round";
}

typedef struct ArtilleryDirection {
  int dir;
  const char *desc;
} ArtilleryDirection;

static const ArtilleryDirection ARTY_DIRS[] = {
    {0, "north"},       {60, "northeast"},  {90, "east"},
    {120, "southeast"}, {180, "south"},     {240, "southwest"},
    {270, "west"},      {300, "northwest"}, {0, nullptr}};

static const ArtilleryDirection *artillery_direction_at(size_t index) {
  return checked_storage_at_const(ARTY_DIRS,
                                  sizeof(ARTY_DIRS) / sizeof(*ARTY_DIRS),
                                  sizeof(*ARTY_DIRS), index);
}

static const char *artillery_direction(ArtilleryShot *s) {
  float fx;
  float fy;
  float tx;
  float ty;
  int b;
  int d;
  int i;
  int best = -1;
  int bestd = 0;

  map_coord_to_real_coord(s->from_x, s->from_y, &fx, &fy);
  map_coord_to_real_coord(s->to_x, s->to_y, &tx, &ty);
  b = map_bearing(&(MapRealSegment){.start = {.x = fx, .y = fy},
                                    .end = {.x = tx, .y = ty}});
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
  const float FLIGHT_TIME = map_real_range(&(MapRealSegment){
                                .start = {.x = fx, .y = fy},
                                .end = {.x = tx, .y = ty},
                            }) /
                            ARTY_SPEED;
  const int DELAY = max(ARTILLERY_MINIMUM_FLIGHT, (int)FLIGHT_TIME);

  /* XXX Different weapons, diff. speed? */
  return DELAY;
}

static void artillery_hit_event(MuxEvent *e) {
  ArtilleryShot *s = (ArtilleryShot *)e->data;

  artillery_hit(s);
}

void artillery_shoot(const ArtilleryShotRequest *request) {
  Mech *mech = request->mech;
  struct ArtilleryShot *s;
  float fx;
  float fy;
  float tx;
  float ty;

  s = checked_storage_allocate(sizeof(*s));
  s->from_x = mech_position_x(mech);
  s->from_y = mech_position_y(mech);
  s->to_x = request->target.x;
  s->to_y = request->target.y;
  s->type = request->weapon_index;
  s->mode = request->weapon_mode;
  s->ishit = request->hit;
  s->shooter = mech_dbref(mech);
  s->map = mech_map_dbref(mech);
  s->context = mech_context(mech);
  mech_los_broadcastf(mech, "shoots %s towards the %s!", artillery_type(s),
                      artillery_direction(s));
  map_coord_to_real_coord(s->from_x, s->from_y, &fx, &fy);
  map_coord_to_real_coord(s->to_x, s->to_y, &tx, &ty);
  btech_context_owned_event_schedule(
      mech_context(mech), s, EVENT_DHIT, artillery_hit_event,
      artillery_round_flight_time(fx, fy, tx, ty), 0);
}

static int blast_arcf(float fx, float fy, Mech *mech) {
  int b;
  int dir;

  b = map_bearing(&(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                              .y = mech_position_real_y(mech)},
                                    .end = {.x = fx, .y = fy}});
  dir = acceptable_degree(b - mech_heading_degrees(mech));
  if (dir > 120 && dir < 240)
    return BACK;
  if (dir > 300 || dir < 60)
    return FRONT;
  if (dir > 180)
    return LEFTSIDE;
  return RIGHTSIDE;
}

static constexpr int TABLE_GEN = 0;
static constexpr int TABLE_PUNCH = 1;
static constexpr int TABLE_KICK = 2;

void blast_hit_real_hex(const BlastRealHexRequest *request) {
  BattleMap *map = request->map;
  Mech *temp_mech;
  int loop;
  int isrear = 0;
  int iscritical = 0;
  int hitloc;
  int damleft;
  int arc;
  int ndam;
  int ground_zero;
  short tx;
  short ty;

  /* Not on a map so just return */
  if (!map)
    return;

  real_coord_to_map_coord(&tx, &ty, request->impact.x, request->impact.y);
  if (tx < 0 || ty < 0 || tx >= map->map_width || ty >= map->map_height)
    return;
  if (!request->messages.target || !request->messages.observers)
    return;
  if (request->safety.underwater)
    ground_zero = battle_map_hex_elevation(map, tx, ty);
  else
    ground_zero = max(0, battle_map_hex_elevation(map, tx, ty));

  for (loop = 0; loop < battle_map_unit_count(map); loop++) {
    const DbRef UNIT = battle_map_unit_dbref(map, loop);
    if (UNIT >= 0) {
      temp_mech = btech_context_get_mech(battle_map_context(map), UNIT);
      if (!temp_mech)
        continue;
      if (mech_position_x(temp_mech) != tx || mech_position_y(temp_mech) != ty)
        continue;
      /* Far too high.. */
      if (mech_position_z(temp_mech) >= (request->safety.above + ground_zero))
        continue;
      /* Far too below (underwater, mostly) */
      if (/* MechTerrain(tempMech) == WATER &&  */
          mech_position_z(temp_mech) <= (ground_zero - request->safety.below))
        continue;
      mech_los_broadcast(temp_mech, request->messages.observers);
      mech_notify(temp_mech, MECHALL, request->messages.target);
      arc = blast_arcf(request->source.x, request->source.y, temp_mech);

      if (arc == BACK)
        isrear = 1;
      damleft = request->damage.total;

      while (damleft > 0) {
        if (request->damage.hit_size <= damleft)
          ndam = request->damage.hit_size;
        else
          ndam = damleft;

        damleft -= ndam;

        switch (request->hit_table) {
        case TABLE_PUNCH:
          if (mech_class(temp_mech) != CLASS_MECH) {
            hitloc = mech_hit_location(temp_mech, arc, &iscritical, &isrear);
          } else {
            hitloc = mech_punch_hit_location(temp_mech, arc);
          }
          break;
        case TABLE_KICK:
          if (mech_class(temp_mech) != CLASS_MECH) {
            hitloc = mech_hit_location(temp_mech, arc, &iscritical, &isrear);
          } else {
            hitloc = mech_kick_hit_location(temp_mech, arc);
          }
          break;
        default:
          hitloc = mech_hit_location(temp_mech, arc, &iscritical, &isrear);
        }

        mech_damage_apply(&(MechDamageRequest){.target = temp_mech,
                                               .attacker = temp_mech,
                                               .line_of_sight = false,
                                               .attack_pilot = -1,
                                               .hit_location = hitloc,
                                               .rear = isrear != 0,
                                               .critical = iscritical != 0,
                                               .armor_damage = ndam,
                                               .internal_damage = 0,
                                               .transfer = MECH_DAMAGE_NORMAL,
                                               .cause = -1,
                                               .base_to_hit = 0,
                                               .weapon_index = -1,
                                               .ammunition_mode = 0,
                                               .ignore_swarmers = false});
      }
      mech_heat_effect_apply(nullptr, temp_mech, request->damage.heat, false);
    }
  }
}

void blast_hit_hex(const BlastHexRequest *request) {
  float ftx;
  float fty;
  float ffx;
  float ffy;

  map_coord_to_real_coord(request->impact.x, request->impact.y, &ftx, &fty);
  map_coord_to_real_coord(request->source.x, request->source.y, &ffx, &ffy);
  BlastRealHexRequest real_request = {
      .map = request->map,
      .damage = request->damage,
      .impact = {.x = ftx, .y = fty},
      .source = {.x = ffx, .y = ffy},
      .messages = request->messages,
      .hit_table = request->hit_table,
      .safety = request->safety,
  };
  blast_hit_real_hex(&real_request);
}

void blast_hit_real_area(const BlastRealAreaRequest *request) {
  BattleMap *map = request->center.map;
  int x1;
  int y1;
  int x2;
  int y2;
  int dm;
  short tx;
  short ty;
  float hx;
  float hy;
  float t = map_real_range(&(MapRealSegment){
      .start = request->center.impact,
      .end = request->center.source,
  });

  dm = max(1, (int)t + 1);
  BlastRealHexRequest hit = request->center;
  hit.damage.total /= dm;
  hit.damage.heat /= dm;
  blast_hit_real_hex(&hit);
  if (!request->neighbor_radius)
    return;
  real_coord_to_map_coord(&tx, &ty, request->center.impact.x,
                          request->center.impact.y);
  for (x1 = (tx - request->neighbor_radius);
       x1 <= (tx + request->neighbor_radius); x1++) {
    for (y1 = (ty - request->neighbor_radius);
         y1 <= (ty + request->neighbor_radius); y1++) {
      int spot;

      dm = map_hex_distance(&(HexDistanceRequest){
          .start = {.x = tx, .y = ty},
          .end = {.x = x1, .y = y1},
          .correction = 0,
      });
      if (dm > request->neighbor_radius)
        continue;
      if ((tx == x1) && (ty == y1))
        continue;
      x2 = bounded(0, x1, map->map_width - 1);
      y2 = bounded(0, y1, map->map_height - 1);
      if (x1 != x2 || y1 != y2)
        continue;
      spot = (x1 == tx && y1 == ty);
      map_coord_to_real_coord(x1, y1, &hx, &hy);
      dm++;
      if (!(request->center.damage.total / dm))
        continue;
      hit = request->center;
      hit.damage.total /= dm;
      hit.damage.heat /= dm;
      hit.impact = (MapRealPosition){.x = hx, .y = hy};
      if (!spot)
        hit.messages = request->neighbor_messages;
      blast_hit_real_hex(&hit);

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
          add_decoration(&(MapDecorationRequest){
              .map = map,
              .position = {.x = x1, .y = y1},
              .type = TYPE_FIRE,
              .terrain_marker = FIRE,
              .duration =
                  btech_random_range_int(battle_map_context(map), 60, 180),
          });
        }

        break;
      }
    }
  }
}

void blast_hit_area(const BlastAreaRequest *request) {
  float fx;
  float fy;

  map_coord_to_real_coord(request->center.impact.x, request->center.impact.y,
                          &fx, &fy);
  BlastRealAreaRequest real_request = {
      .center =
          {
              .map = request->center.map,
              .damage = request->center.damage,
              .impact = {.x = fx, .y = fy},
              .source = {.x = fx, .y = fy},
              .messages = request->center.messages,
              .hit_table = request->center.hit_table,
              .safety = request->center.safety,
          },
      .neighbor_messages = request->neighbor_messages,
      .neighbor_radius = request->neighbor_radius,
  };
  blast_hit_real_area(&real_request);
}

typedef struct ArtilleryImpact {
  BattleMap *map;
  ArtilleryShot *shot;
  int damage;
  MapHexPosition position;
  bool direct;
} ArtilleryImpact;

static void artillery_hit_hex(const ArtilleryImpact *impact) {
  BattleMap *map = impact->map;
  ArtilleryShot *s = impact->shot;
  int mode = s->mode;
  int dam = impact->damage;
  int tx = impact->position.x;
  int ty = impact->position.y;
  char buf1[LBUF_SIZE];
  char buf2[LBUF_SIZE];

  /* Safety check -- shouldn't happen */
  if (tx < 0 || tx >= map->map_width || ty < 0 || ty >= map->map_height)
    return;

  if ((mode & SMOKE_MODE)) {
    /* Add smoke */
    add_decoration(&(MapDecorationRequest){
        .map = map,
        .position = {.x = tx, .y = ty},
        .type = TYPE_SMOKE,
        .terrain_marker = SMOKE,
        .duration = btech_random_range_int(battle_map_context(map), 90, 150),
    });
    return;
  }
  if (mode & MINE_MODE) {
    mine_field_add(map, tx, ty, dam);
    return;
  }
  if (!(mode & CLUSTER_MODE)) {
    if (impact->direct)
      (void)snprintf(buf1, LBUF_SIZE, "receives a direct hit!");
    else
      (void)snprintf(buf1, LBUF_SIZE, "is hit by fragments!");
    if (impact->direct)
      (void)snprintf(buf2, LBUF_SIZE, "You receive a direct hit!");
    else
      (void)snprintf(buf2, LBUF_SIZE, "You are hit by fragments!");
  } else {
    if (dam > 2) {
      (void)string_copy_bounded(buf1, sizeof(buf1), "is hit by bomblets!");
      (void)string_copy_bounded(buf2, sizeof(buf2), "You are hit by bomblets!");
    } else {
      (void)string_copy_bounded(buf1, sizeof(buf1), "is hit by a bomblet!");
      (void)string_copy_bounded(buf2, sizeof(buf2),
                                "You are hit by a bomblet!");
    }
  }
  BlastHexRequest request = {
      .map = map,
      .damage = {.total = dam, .hit_size = (mode & CLUSTER_MODE) ? 2 : 5},
      .impact = {.x = tx, .y = ty},
      .source = {.x = tx, .y = ty},
      .messages = {.target = buf2, .observers = buf1},
      .hit_table = (mode & CLUSTER_MODE) ? TABLE_PUNCH : TABLE_GEN,
      .safety = {.above = 10, .below = 4},
  };
  blast_hit_hex(&request);
}

typedef struct ArtilleryNeighborHit ArtilleryNeighborHit;
struct ArtilleryNeighborHit {
  BattleMap *map;
  ArtilleryShot *shot;
  int damage;
};

static void artillery_hit_neighbors_callback(BattleMap *map, int x, int y,
                                             void *context) {
  const ArtilleryNeighborHit *hit = context;

  artillery_hit_hex(&(ArtilleryImpact){
      .map = map,
      .shot = hit->shot,
      .damage = hit->damage,
      .position = {.x = x, .y = y},
  });
}

static void artillery_hit_neighbors(const ArtilleryImpact *impact) {
  ArtilleryNeighborHit hit = {
      .map = impact->map,
      .shot = impact->shot,
      .damage = impact->damage,
  };

  visit_neighbor_hexes(impact->map, impact->position.x, impact->position.y,
                       artillery_hit_neighbors_callback, &hit);
}

static void artillery_cluster_hit(const ArtilleryImpact *impact) {
  BattleMap *map = impact->map;
  int dam = impact->damage;
  int tx = impact->position.x;
  int ty = impact->position.y;
  /* Main idea: Pick <dam/2> bombs of 2pts each, and scatter 'em
     over 5x5 area with weighted numbers */
  int xd;
  int yd;
  int x;
  int y;
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
    const int TARGET_INDEX = ((xd + 2) * 5) + yd + 2;
    int *target = checked_storage_at(targets.cells, 25, sizeof(*targets.cells),
                                     (size_t)TARGET_INDEX);
    (*target)++;
  }
  for (xd = 0; xd < 5; xd++) {
    for (yd = 0; yd < 5; yd++) {
      d = *(const int *)checked_storage_at_const(
          targets.cells, 25, sizeof(*targets.cells),
          ((size_t)xd * 5U) + (size_t)yd);
      if (d) {
        artillery_hit_hex(&(ArtilleryImpact){
            .map = map,
            .shot = impact->shot,
            .damage = d * 2,
            .position = {.x = xd + tx - 2, .y = yd + ty - 2},
            .direct = true,
        });
      }
    }
  }
}

void artillery_friendly_adjustment(DbRef mechnum, BattleMap *map, int x,
                                   int y) {
  Mech *mech;
  Mech *spotter;
  Mech *temp_mech = nullptr;

  mech = btech_context_get_mech(battle_map_context(map), mechnum);
  if (!mech)
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
    if (mech_sees_hex(spotter, map, x, y))
      temp_mech = spotter;
  } else {
    temp_mech = find_mech_in_hex(mech, map, x, y, 2);
  }
  if (!temp_mech)
    return;
  if (!mech_is_started(temp_mech) || !mech_is_started(mech))
    return;
  if (spotter) {
    mech_printf(mech, MECHSTARTED,
                "%s sent you some trajectory-correction data.",
                mech_to_mech_display_id(mech, temp_mech).text);
    mech_printf(temp_mech, MECHSTARTED,
                "You provide %s with information about the miss.",
                mech_to_mech_display_id(temp_mech, mech).text);
  }
  mech_fire_adjustment_increment(mech);
}

static void artillery_hit(ArtilleryShot *s) {
  char message_buffer[LBUF_SIZE];
  /* First, we figure where it exactly hits. Our first-hand information
     is only whether it hits or not, not _where_ it hits */
  float dir;
  int di;
  int dist;
  int weight;
  BattleMap *map = btech_context_get_map(s->context, s->map);
  int original_x = 0;
  int original_y = 0;
  int dam = weapon_catalogue_damage(s->type);

  if (!map)
    return;
  if (!s->ishit) {
    /* Shit! We missed target ;-) */
    /* Time to calculate a new target hex */
    di = btech_random_range_int(battle_map_context(map), 0, 359);
    dir = (float)di * (float)M_PI / 180.0F;
    dist = btech_random_range_int(battle_map_context(map), 2, 7);
    weight = 100 * (dist * 6) / (((dist * 6) + map->windspeed));
    dist = ((dist * weight) + ((map->windspeed / 6) * (100 - weight))) / 100;
    original_x = s->to_x;
    original_y = s->to_y;
    s->to_x += (int)((float)dist * cosf(dir));
    s->to_y += (int)((float)dist * sinf(dir));
    s->to_x = bounded(0, s->to_x, map->map_width - 1);
    s->to_y = bounded(0, s->to_y, map->map_height - 1);
    /* Time to calculate if any friendlies have LOS to hex,
       and if so, adjust fire adjustment unless you lack information /
       have changed target */
  }
  /* It's time to run for your lives, lil' ones ;-) */
  if (!(s->mode & ARTILLERY_MODES)) {
    (void)snprintf(message_buffer, sizeof(message_buffer), "%s fire hits $H!",
                   checked_string_suffix(weapon_catalogue_name(s->type), 3));
    hex_los_broadcast(map, s->to_x, s->to_y, message_buffer);
  } else if (s->mode & CLUSTER_MODE) {
    hex_los_broadcast(map, s->to_x, s->to_y,
                      "A rain of small bomblets hits $H's surroundings!");
  } else if (s->mode & MINE_MODE) {
    hex_los_broadcast(map, s->to_x, s->to_y,
                      "A rain of small bomblets hits $H!");
  } else if (s->mode & SMOKE_MODE) {
    (void)snprintf(message_buffer, sizeof(message_buffer),
                   "A %s %s hits $h, and smoke starts to billow!",
                   checked_string_suffix(weapon_catalogue_name(s->type), 3),
                   checked_string_suffix(artillery_type(s), 2));
    hex_los_broadcast(map, s->to_x, s->to_y, message_buffer);
  }

  /* Basic theory:
     - smoke / ordinary rounds are spread with the ordinary functions
     - mines are otherwise ordinary except no hitting of neighbor hexes
     - cluster bombs are special
   */
  if (!(s->mode & CLUSTER_MODE)) {
    /* Enjoy ourselves in all neighbor hexes, too */
    ArtilleryImpact impact = {
        .map = map,
        .shot = s,
        .damage = dam,
        .position = {.x = s->to_x, .y = s->to_y},
        .direct = true,
    };
    artillery_hit_hex(&impact);
    if (!(s->mode & MINE_MODE)) {
      impact.damage /= 2;
      impact.direct = false;
      artillery_hit_neighbors(&impact);
    }
  } else {
    ArtilleryImpact impact = {
        .map = map,
        .shot = s,
        .damage = dam,
        .position = {.x = s->to_x, .y = s->to_y},
        .direct = true,
    };
    artillery_cluster_hit(&impact);
  }
  if (!s->ishit)
    artillery_friendly_adjustment(s->shooter, map, original_x, original_y);
}
