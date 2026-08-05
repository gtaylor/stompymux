#include "map_building_query_api.h"
#include "map_obj_internal.h"

#include "mech_identity_api.h"
#include "mech_position_api.h"

bool battle_map_building_is_invisible(const BattleMap *map) {
  return BuildIsInvis(map);
}

bool battle_map_building_is_hidden(const BattleMap *map) {
  return BuildIsHidden(map);
}

bool battle_map_building_is_safe(const BattleMap *map) {
  return BuildIsSafe(map);
}

bool battle_map_building_is_command_center(const BattleMap *map) {
  return BuildIsCS(map);
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
  map->cf = i1;
  map->cfmax = i2;
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

static void damage_cf(Mech *mech, MapObject *o, int from, int to, int damage) {
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
    notify_except(
        btech_context_evaluation(mech_context(mech)), o->obj, NOTHING, o->obj,
        tprintf("%s is hit for %d more points of damage, destroying it!",
                MyToUpper(structure_name(mech_context(mech)->database, o).text),
                damage));
    mech_los_broadcast(
        mech, tprintf("hits %s, destroying it!",
                      structure_name(mech_context(mech)->database, o).text));
    start_regen = 2;
  } else {
    mech_printf(mech, MECHALL, "You hit %s for %d points of damage.",
                structure_name(mech_context(mech)->database, o).text, damage);
    notify_except(
        btech_context_evaluation(mech_context(mech)), o->obj, NOTHING, o->obj,
        tprintf("%s is hit for %d points of damage.",
                MyToUpper(structure_name(mech_context(mech)->database, o).text),
                damage));
  }
  if (start_regen)
    possibly_start_building_regen(mech_context(mech), o->obj);
}

void hit_building(Mech *mech, int x, int y, int weapindx, int damage) {
  MapObject *o;
  BattleMap *map;
  BattleMap *nmap;
  int num_missiles_hit, hit_roll;
  int i1, i2;

  if (!(map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech))))
    return;
  if (!(o = find_entrance_by_xy(map, x, y)))
    return;
  if (!(nmap = btech_context_get_map(mech_context(mech), o->obj)))
    return;
  if (!damage) {
    if (!IsMissile(weapindx))
      damage = MechWeapons[weapindx].damage;
    else {
      const MissileHitEntry *entry = missile_hit_registry_find_weapon(
          &mech_context(mech)->missile_hits, weapindx);

      /* Missile weapon.  Multiple Hit locations... */
      if (entry == nullptr)
        return;
      if ((MechWeapons[weapindx].type == STREAK) && (!AngelECMDisturbed(mech)))
        num_missiles_hit = entry->num_missiles[10];
      else {
        hit_roll = btech_random_roll(map->xcode.context) - 2;
        num_missiles_hit = entry->num_missiles[hit_roll];
      }
      damage = num_missiles_hit * MechWeapons[weapindx].damage;
    }
  }
  if (!damage)
    return;
  if (MapIsCS(map) || BuildIsCS(nmap)) {
    mech_notify(mech, MECHALL, "Your shot only scratches the paint!");
    return;
  }
  if (!get_building_cf(nmap, &i1, &i2))
    return;
  damage_cf(mech, o, i1, i2, damage);
}

void fire_hex(Mech *mech, int x, int y, int meant) {
  BattleMap *map;

  if (!(map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech))))
    return;
  switch (map_terrain_get(map, x, y)) {
  case HEAVY_FOREST:
    break;
  case LIGHT_FOREST:
    break;
  default:
    return;
  }
  if (meant) {
    mech_los_broadcast(mech, tprintf("'s shot ignites %d,%d!", x, y));
    mech_printf(mech, MECHALL, "You ignite %d,%d.", x, y);
  } else {
    mech_los_broadcast(mech, tprintf("'s stray shot ignites %d,%d!", x, y));
    mech_printf(mech, MECHALL, "You accidentally ignite %d,%d!", x, y);
  }
  add_decoration(map, x, y, TYPE_FIRE, FIRE,
                 btech_random_range(map->xcode.context, 60, 180));
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
  if (!(o = find_entrance_by_xy(map, x, y)))
    return;
  if (!(nmap = btech_context_get_map(mech_context(mech), o->obj)))
    return;
  if (BuildIsDSS(nmap))
    return;
  if (BuildIsHidden(nmap) && !MadePerceptionRoll(mech, 0))
    return;
  mech_printf(mech, MECHALL, "%s has CF of %d.",
              MyToUpper(structure_name(mech_context(mech)->database, o).text),
              nmap->cf);
}

void show_building_in_hex(Mech *mech, int x, int y) {
  MapObject *o;
  BattleMap *map;
  BattleMap *nmap;

  if (!(map =
            btech_context_get_map(mech_context(mech), mech_map_dbref(mech)))) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  if (!(o = find_entrance_by_xy(map, x, y))) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  if (!(nmap = btech_context_get_map(mech_context(mech), o->obj))) {
    mech_notify(mech, MECHALL, "The sensors detect no building in the hex!");
    return;
  }
  if (BuildIsInvis(nmap) ||
      (BuildIsHidden(nmap) &&
       !MadePerceptionRoll(
           mech, (int)(FindRange(mech_position_x(mech), mech_position_y(mech),
                                 mech_position_z(mech), x, y, 0) +
                       0.95)))) {
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
    if (map->MapObject[i])
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
