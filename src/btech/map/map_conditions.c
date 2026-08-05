
/*
 * $Id: map.conditions.c,v 1.2 2005/01/15 16:57:14 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Wed Apr 23 15:18:01 1997 fingon
 * Last modified: Thu Sep 10 07:35:26 1998 fingon
 *
 */

#include "btech/context.h"
#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_conditions_api.h"
#include "mech_api_types.h"
#include "mech_runtime_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"

void alter_conditions(BattleMap *map) {
  int i;
  Mech *mech;

  for (i = 0; i < map->first_free; i++)
    if ((mech =
             btech_context_get_mech(map->xcode.context, map->mechsOnMap[i]))) {
      map_conditions_apply(mech, map);
    }
}

int battle_map_gravity(const BattleMap *map) { return map->grav; }

int battle_map_light(const BattleMap *map) { return map->maplight; }

int battle_map_visibility(const BattleMap *map) { return map->mapvis; }

int battle_map_maximum_visibility(const BattleMap *map) { return map->maxvis; }

int battle_map_cloud_base(const BattleMap *map) { return map->cloudbase; }

int battle_map_temperature(const BattleMap *map) { return map->temp; }

bool battle_map_sensor_is_disabled(const BattleMap *map, int sensor) {
  return map->sensorflags & (1 << sensor);
}

bool battle_map_bridges_have_capacity(const BattleMap *map) {
  return map->flags & MAPFLAG_BRIDGESCS;
}

bool battle_map_is_dark(const BattleMap *map) {
  return map->flags & MAPFLAG_DARK;
}

bool battle_map_is_underground(const BattleMap *map) {
  return map->flags & MAPFLAG_UNDERGROUND;
}

void map_setconditions(DbRef player, BattleMap *map, char *buffer) {
  char *args[5];
  int vacuum = -1, underground = -1, grav, temp, argc;
  int fl;

  DOCHECK_CONTEXT(map->xcode.context,
                  (argc = mech_parseattributes(buffer, args, 4)) < 2,
                  "(At least) 2 options required (gravity + temperature)");
  DOCHECK_CONTEXT(map->xcode.context, argc > 4,
                  "Too many options! Command accepts only 4 at max (gravity "
                  "+ temperature + vacuum-flag + underground-flag)");
  DOCHECK_CONTEXT(map->xcode.context, Readnum(grav, args[0]),
                  "Invalid gravity (must be integer in range of 0 to 255)");
  DOCHECK_CONTEXT(map->xcode.context, grav < 0 || grav > 255,
                  "Invalid gravity (must be integer in range of 0 to 255)");
  DOCHECK_CONTEXT(
      map->xcode.context, Readnum(temp, args[1]),
      "Invalid temperature (must be integer in range of -128 to 127");
  DOCHECK_CONTEXT(
      map->xcode.context, temp < -128 || temp > 127,
      "Invalid temperature (must be integer in range of -128 to 127");
  if (argc > 2) {
    DOCHECK_CONTEXT(map->xcode.context, Readnum(vacuum, args[2]),
                    "Invalid vacuum flag (must be integer, 0 or 1)");
    DOCHECK_CONTEXT(map->xcode.context, vacuum < 0 || vacuum > 1,
                    "Invalid vacuum flag (must be integer, 0 or 1)");
  }
  if (argc > 3) {
    DOCHECK_CONTEXT(map->xcode.context, Readnum(underground, args[3]),
                    "Invalid underground flag (must be integer, 0 or 1)");
    DOCHECK_CONTEXT(map->xcode.context, underground < 0 || underground > 1,
                    "Invalid underground flag (must be integer, 0 or 1)");
  }
  fl = (map->flags & (~(MAPFLAG_SPEC | MAPFLAG_VACUUM)));
  if (vacuum > 0)
    fl |= MAPFLAG_VACUUM;
  if (underground > 0)
    fl |= MAPFLAG_UNDERGROUND;
  if (fl & MAPFLAG_VACUUM)
    fl |= MAPFLAG_SPEC;
  if (temp < -30 || temp > 50 || grav != 100)
    fl |= MAPFLAG_SPEC;
  map->temp = temp;
  map->grav = grav;
  map->flags = fl;
  notify(btech_context_evaluation(map->xcode.context), player,
         "Conditions set!");
  alter_conditions(map);
}

void map_conditions_apply(Mech *mech, BattleMap *map) {
  if (!mech)
    return;
  if (!map) {
    mech_environment_conditions_set(mech, false, false, false, false);
    return;
  }
  mech_environment_conditions_set(mech, MapUnderSpecialRules(map),
                                  MapTemperature(map) < -30 ||
                                      MapTemperature(map) > 50,
                                  MapGravity(map) != 100, MapIsVacuum(map));
}

bool battle_map_uses_special_rules(const BattleMap *map) {
  return MapUnderSpecialRules(map);
}
