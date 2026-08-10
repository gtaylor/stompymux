#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_bits_api.h"
#include "map_building_query_api.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_effect_types.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_events.h"
#include "mech_utils_api.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "weapon_catalogue_api.h"

#include <math.h>

#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "registry_api.h"

bool battle_map_building_is_invisible(const BattleMap *map) {
  return battle_map_build_is_invisible(map);
}

bool battle_map_building_is_hidden(const BattleMap *map) {
  return battle_map_build_is_hidden(map);
}

bool battle_map_building_is_safe(const BattleMap *map) {
  return battle_map_build_is_safe(map);
}

bool battle_map_building_is_command_center(const BattleMap *map) {
  return battle_map_build_is_complex_structure(map);
}

int battle_map_building_integrity(const BattleMap *map) { return map->cf; }

int battle_map_building_maximum_integrity(const BattleMap *map) {
  return map->cfmax;
}

static int get_building_cf(BattleMap *map, int *i1, int *i2) {
  *i1 = map->cf;
  *i2 = map->cfmax;
  return map->cf;
}

static void set_building_cf(BattleMap *map, int i1, int i2) {
  map->cf = clamp_int_to_short(i1);
  map->cfmax = clamp_int_to_short(i2);
}

static void building_regen_event(MuxEvent *e) {
#ifdef BUILDINGS_REPAIR_THEMSELVES
  BattleMap *map = e->data;
  int cf, max;

  if (!get_building_cf(map, &cf, &max))
    return;
  cf = MIN(cf + map->regen_factor, max);
  set_building_cf(map, cf, max);
  if (cf != max)
    btech_event_schedule(e->scheduler, map, EVENT_BREGEN, building_regen_event,
                         BUILDING_REPAIR_DELAY, 0);
#endif
}

static void building_rebuild_event(MuxEvent *e) {
#ifdef BUILDINGS_REBUILD_FROM_DESTRUCTION
  BattleMap *map = e->data;
  int cf = 0, max = 0;

  if (get_building_cf(map, &cf, &max))
    return;
  if (max <= 0)
    return;
  set_building_cf(map, 1, max);
#endif
}

void possibly_start_building_regen(BtechContext *context, DbRef obj) {
  BattleMap *map = btech_context_get_map(context, obj);
  int f, t;

  if (map == nullptr || !get_building_cf(map, &f, &t))
    return;
  if (f == t)
    return;
  if (!f)
    btech_event_schedule(context->events, map, EVENT_BREBUILD,
                         building_rebuild_event, BUILDING_DREBUILD_DELAY, 0);
  else
    btech_event_schedule(context->events, map, EVENT_BREGEN,
                         building_regen_event, BUILDING_REPAIR_DELAY, 0);
}

typedef struct BuildingDamageRequest {
  Mech *mech;
  MapObject *object;
  int current_integrity;
  int maximum_integrity;
  int damage;
} BuildingDamageRequest;

static void damage_cf(const BuildingDamageRequest *request) {
  Mech *mech = request->mech;
  MapObject *o = request->object;
  int from = request->current_integrity;
  const int to = request->maximum_integrity;
  int damage = request->damage;
  int destroy = 0;
  int start_regen = 0;

  if (from == to)
    start_regen = 1;
  damage = MIN(from, damage);
  if (from == damage)
    destroy = 1;
  from -= damage;
  BattleMap *building = btech_context_get_map(mech_context(mech), o->obj);

  if (building == nullptr)
    return;
  set_building_cf(building, from, to);
  if (destroy) {
    mech_printf(mech, MECHALL,
                "You hit %s for %d points of damage, destroying it!",
                structure_name(mech_context(mech)->database, o).text, damage);
    mecha_notify_except(&(MechaNotificationExclusion){
        .evaluation = btech_context_evaluation(mech_context(mech)),
        .location = o->obj,
        .actor = NOTHING,
        .exception = o->obj,
        .message = tprintf(
            "%s is hit for %d more points of damage, destroying it!",
            MyToUpper(structure_name(mech_context(mech)->database, o).text),
            damage)});
    mech_los_broadcast(
        mech, tprintf("hits %s, destroying it!",
                      structure_name(mech_context(mech)->database, o).text));
    start_regen = 2;
  } else {
    mech_printf(mech, MECHALL, "You hit %s for %d points of damage.",
                structure_name(mech_context(mech)->database, o).text, damage);
    mecha_notify_except(&(MechaNotificationExclusion){
        .evaluation = btech_context_evaluation(mech_context(mech)),
        .location = o->obj,
        .actor = NOTHING,
        .exception = o->obj,
        .message = tprintf(
            "%s is hit for %d points of damage.",
            MyToUpper(structure_name(mech_context(mech)->database, o).text),
            damage)});
  }
  if (start_regen)
    possibly_start_building_regen(mech_context(mech), o->obj);
}

void hit_building(const BuildingHitRequest *request) {
  Mech *mech = request->mech;
  const int x = request->position.x;
  const int y = request->position.y;
  const int weapindx = request->weapon_index;
  int damage = request->damage;
  MapObject *o;
  BattleMap *map;
  BattleMap *nmap;
  int num_missiles_hit, hit_roll;
  int i1, i2;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map)
    return;
  o = find_entrance_by_xy(map, x, y);
  if (!o)
    return;
  nmap = btech_context_get_map(mech_context(mech), o->obj);
  if (!nmap)
    return;
  if (!damage) {
    if (!weapon_catalogue_is_missile(weapindx))
      damage = weapon_catalogue_damage(weapindx);
    else {
      /* Missile weapon.  Multiple Hit locations... */
      if (!btech_context_has_missile_hit_table(mech_context(mech), weapindx))
        return;
      if ((weapon_catalogue_type(weapindx) == STREAK) &&
          !mech_condition_summary(mech).angel_ecm_disturbed)
        num_missiles_hit = btech_context_missile_hit_count(&(MissileHitLookup){
            .context = mech_context(mech),
            .weapon = weapindx,
            .roll = 10,
        });
      else {
        hit_roll = btech_random_roll(map->xcode.context) - 2;
        num_missiles_hit = btech_context_missile_hit_count(&(MissileHitLookup){
            .context = mech_context(mech),
            .weapon = weapindx,
            .roll = hit_roll,
        });
      }
      damage = num_missiles_hit * weapon_catalogue_damage(weapindx);
    }
  }
  if (!damage)
    return;
  if (battle_map_build_is_complex(map) ||
      battle_map_build_is_complex_structure(nmap)) {
    mech_notify(mech, MECHALL, "Your shot only scratches the paint!");
    return;
  }
  if (!get_building_cf(nmap, &i1, &i2))
    return;
  damage_cf(&(BuildingDamageRequest){.mech = mech,
                                     .object = o,
                                     .current_integrity = i1,
                                     .maximum_integrity = i2,
                                     .damage = damage});
}

void fire_hex(const TerrainHexEffectRequest *request) {
  Mech *mech = request->mech;
  const int x = request->position.x;
  const int y = request->position.y;
  BattleMap *map;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map)
    return;
  switch (map_terrain_get(map, x, y)) {
  case HEAVY_FOREST:
    break;
  case LIGHT_FOREST:
    break;
  default:
    return;
  }
  if (request->intentional) {
    mech_los_broadcast(mech, tprintf("'s shot ignites %d,%d!", x, y));
    mech_printf(mech, MECHALL, "You ignite %d,%d.", x, y);
  } else {
    mech_los_broadcast(mech, tprintf("'s stray shot ignites %d,%d!", x, y));
    mech_printf(mech, MECHALL, "You accidentally ignite %d,%d!", x, y);
  }
  add_decoration(&(MapDecorationRequest){
      .map = map,
      .position = {.x = x, .y = y},
      .type = TYPE_FIRE,
      .terrain_marker = FIRE,
      .duration = btech_random_range_int(map->xcode.context, 60, 180),
  });
}

void steppable_base_check(Mech *mech, int x, int y) {
  MapObject *o;
  BattleMap *map;
  BattleMap *nmap;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map)
    return;
  if (mech_position_z(mech) != map_elevation_get(map, x, y))
    return;
  if (!(is_hangar_hex(map, x, y)))
    return;
  o = find_entrance_by_xy(map, x, y);
  if (!o)
    return;
  nmap = btech_context_get_map(mech_context(mech), o->obj);
  if (!nmap)
    return;
  if (battle_map_build_is_dropship_structure(nmap))
    return;
  if (battle_map_build_is_hidden(nmap) && !MadePerceptionRoll(mech, 0))
    return;
  mech_printf(mech, MECHALL, "%s has CF of %d.",
              MyToUpper(structure_name(mech_context(mech)->database, o).text),
              nmap->cf);
}

void show_building_in_hex(Mech *mech, int x, int y) {
  MapObject *o;
  BattleMap *map;
  BattleMap *nmap;

  map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  if (!map) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  o = find_entrance_by_xy(map, x, y);
  if (!o) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  nmap = btech_context_get_map(mech_context(mech), o->obj);
  if (!nmap) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  const int mech_x = mech_position_x(mech);
  const int mech_y = mech_position_y(mech);
  const int mech_z = mech_position_z(mech);
  const float building_range = map_spatial_range(&(MapSpatialSegment){
      .start = {.x = (float)mech_x, .y = (float)mech_y, .z = (float)mech_z},
      .end = {.x = (float)x, .y = (float)y, .z = 0.0F},
  });
  const float rounded_range = floorf(building_range + 0.95F);
  const int perception_difficulty = (int)rounded_range;
  if (battle_map_build_is_invisible(nmap) ||
      (battle_map_build_is_hidden(nmap) &&
       !MadePerceptionRoll(mech, perception_difficulty))) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  mech_printf(mech, MECHALL, "%s's CF is %d.",
              MyToUpper(structure_name(mech_context(mech)->database, o).text),
              nmap->cf);
}

int obj_size(BattleMap *map) {
  int s = 0;
  MapObject *m;
  int i;

  for (i = 0; i < NUM_MAPOBJTYPES; i++)
    if (first_mapobj(map, i))
      for (m = first_mapobj(map, i); m; m = next_mapobj(m))
        s += sizeof(MapObject);
  return s;
}

int map_underlying_terrain(BattleMap *map, int x, int y) {
  char c;

  if (!map)
    return 0;
  c = find_decorations(map, x, y);
  if (c)
    return c;
  return map_terrain_get(map, x, y);
}

int mech_underlying_terrain(Mech *mech) {
  char c;
  BattleMap *map =
      btech_context_find_object(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return mech_position_terrain(mech);
  c = find_decorations(map, mech_position_x(mech), mech_position_y(mech));
  if (c)
    return c;
  return mech_position_terrain(mech);
}
