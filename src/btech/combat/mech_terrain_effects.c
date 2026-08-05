/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997-2002 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "artillery_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "failures.h"
#include "failures_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_build_api.h"
#include "mech_combat.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_combat_missile_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/attrs.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "pcombat_api.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_settings.h"

const char *mech_hex_target_description(const Mech *mech) {
  if (mech_targets_hex_for_ignition(mech))
    return "at the hex, trying to ignite it";
  if (mech_targets_hex_for_clearing(mech))
    return "at the hex, trying to clear it";
  if (mech_targets_hex(mech))
    return "at the hex";
  if (mech_targets_building(mech))
    return "at the building at";
  return "at";
}

/*
intentional:
    if ignite attack hits, roll 2D6 and consult table:
        (Success Numbers) Weapon Type:
            Flamer 4+,
            Incendiary LRMs 5+,
            Energy Weapon (minus small lasers) 7+,
            Missile or Ballistic (minus GR or SRM2s) 9+

Modifiers for terrain to those bths are as follows:
    Woods/Light Buildings no bth mod,
    Med Bldg +1,
    Heavy Bldg +2,
    Hardened Building +3

A unit attempting to clear a wooded hex runs the risk of setting fire to the
woods accidently. To represent this risk, the player rolls 2D6 before attempting
to clear. If the result is 5 or less, then the woods catch on fire.

If a weapon attack against a unit occupying a wooded hex misses its target and
the weapon can be used to start fires (weapons listed above),
the attacking player rolls 2D6. On a result of 2 or 3, the hex catches fire.
Buildings can't accidently be set on fire.

*/

static bool weapon_can_ignite(int weapindx) {
  if (strcmp(&MechWeapons[weapindx].name[3], "ERSmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "SmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "SmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "X-SmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "ERSmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "HeavySmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "GaussRifle") &&
      strcmp(&MechWeapons[weapindx].name[3], "LightGaussRifle") &&
      strcmp(&MechWeapons[weapindx].name[3], "HeavyGaussRifle") &&
      strcmp(&MechWeapons[weapindx].name[3], "MagshotGaussRifle") &&
      strcmp(&MechWeapons[weapindx].name[3], "MachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "LightMachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "HeavyMachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "StreakSRM-2") &&
      strcmp(&MechWeapons[weapindx].name[3], "SRM-2") &&
      strcmp(&MechWeapons[weapindx].name[3], "NarcBeacon") &&
      strcmp(&MechWeapons[weapindx].name[3], "iNarcBeacon"))
    return 1;

  return 0;
}

static bool weapon_can_clear(int weapindx) {
  if (strcmp(&MechWeapons[weapindx].name[3], "ERSmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "SmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "SmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "X-SmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "ERSmallPulseLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "HeavySmallLaser") &&
      strcmp(&MechWeapons[weapindx].name[3], "MachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "LightMachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "HeavyMachineGun") &&
      strcmp(&MechWeapons[weapindx].name[3], "AC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "UltraAC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "CaselessAC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "HyperAC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "LightAC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "RotaryAC/2") &&
      strcmp(&MechWeapons[weapindx].name[3], "LB2-XAC") &&
      strcmp(&MechWeapons[weapindx].name[3], "AC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "UltraAC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "CaselessAC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "HyperAC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "LightAC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "RotaryAC/5") &&
      strcmp(&MechWeapons[weapindx].name[3], "LB5-XAC") &&
      strcmp(&MechWeapons[weapindx].name[3], "StreakSRM-2") &&
      strcmp(&MechWeapons[weapindx].name[3], "SRM-2"))
    return 1;

  return 0;
}

void mech_terrain_possibly_ignite(Mech *mech, BattleMap *map, int weapindx,
                                  int ammoMode, int x, int y, int intentional) {
  char terrain = map_terrain_get(map, x, y);
  int roll = btech_random_roll(mech_context(mech));
  int bth = 13;

  if (MechWeapons[weapindx].special & PCOMBAT)
    return;

  if ((terrain != LIGHT_FOREST) && (terrain != HEAVY_FOREST))
    return;

  if (!strcmp(&MechWeapons[weapindx].name[3], "Flamer") ||
      !strcmp(&MechWeapons[weapindx].name[3], "HeavyFlamer"))
    bth = 4;
  else if (IsMissile(weapindx) && (ammoMode & INFERNO_MODE))
    bth = 5;
  else if (IsBallistic(weapindx) && (ammoMode & AC_FLECHETTE_MODE))
    bth = 5;
  else if (IsEnergy(weapindx) && weapon_can_ignite(weapindx))
    bth = 5;
  else if ((IsMissile(weapindx) || IsBallistic(weapindx)) &&
           weapon_can_ignite(weapindx))
    bth = 9;

  if (roll >= bth)
    fire_hex(mech, x, y, intentional);
}

void mech_terrain_possibly_clear(Mech *mech, BattleMap *map, int weapindx,
                                 int ammoMode, int damage, int x, int y,
                                 int intentional) {
  int igniteBTH = 5; /* This is for intentional clearing */
  int igniteRoll = btech_random_roll(mech_context(mech));
  int clearRoll = btech_random_roll(mech_context(mech));

  if (MechWeapons[weapindx].special & PCOMBAT)
    return;

  if (!intentional)
    igniteBTH = 3;

  if (igniteRoll <= igniteBTH) {
    mech_terrain_possibly_ignite(mech, map, weapindx, ammoMode, x, y,
                                 intentional);
    return;
  }

  if (!weapon_can_clear(weapindx))
    return;

  if (clearRoll > damage)
    return;

  clear_hex(mech, x, y, intentional);
  possibly_remove_mines(mech, x, y);
}

void mech_terrain_possibly_ignite_or_clear(Mech *mech, int weapindx,
                                           int ammoMode, int damage, int x,
                                           int y, int intentional) {
  BattleMap *map;

  map = btech_context_find_object(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return;

  if (mech_targets_hex_for_ignition(mech)) {
    mech_terrain_possibly_ignite(mech, map, weapindx, ammoMode, x, y, 1);
    return;
  }

  if (mech_targets_hex_for_clearing(mech)) {
    mech_terrain_possibly_clear(mech, map, weapindx, ammoMode, damage, x, y, 1);
    return;
  }

  mech_terrain_possibly_clear(mech, map, weapindx, ammoMode, damage, x, y,
                              intentional);
}

void mech_terrain_hex_hit(Mech *mech, int x, int y, int weapindx, int ammoMode,
                          int damage, int ishit) {
  if (!mech_targets_hex_or_building(mech))
    return;

  /* Ok.. we either try to clear/ignite the hex, or alternatively we try to hit
   * building in it */
  if (mech_targets_building(mech)) {
    if (ishit > 0)
      hit_building(mech, x, y, weapindx, damage);
  } else {
    mech_terrain_possibly_ignite_or_clear(mech, weapindx, ammoMode, damage, x,
                                          y, 1);

    if (mech_targets_hex(mech)) {
      possibly_blow_ice(mech, weapindx, x, y);
      possibly_blow_bridge(mech, weapindx, x, y);
    }
  }
}

/****************************************
 * End: Hex hitting related functions
 ****************************************/
