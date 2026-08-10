/* Implements BattleTech combat mechanics for unit terrain effects. */

#include "bsuit_api.h"
#include "btech_event.h"
#include "map.h"
#include "map_api.h"
#include "map_effect_types.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_combat_api.h"
#include "mech_events_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_spot_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

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
  return weapon_catalogue_can_ignite_terrain(weapindx);
}

static bool weapon_can_clear(int weapindx) {
  return weapon_catalogue_can_clear_terrain(weapindx);
}

static void
mech_terrain_possibly_ignite(const TerrainWeaponEffectRequest *request) {
  Mech *mech = request->mech;
  BattleMap *map = request->map;
  const int weapindx = request->weapon_index;
  const int ammoMode = request->ammunition_mode;
  const int x = request->position.x;
  const int y = request->position.y;
  char terrain = map_terrain_get(map, x, y);
  int roll = btech_random_roll(mech_context(mech));
  int bth = 13;

  if (weapon_catalogue_is_personal_combat(weapindx))
    return;

  if ((terrain != LIGHT_FOREST) && (terrain != HEAVY_FOREST))
    return;

  if (weapon_catalogue_is_terrain_flamer(weapindx))
    bth = 4;
  else if (weapon_catalogue_is_missile(weapindx) && (ammoMode & INFERNO_MODE))
    bth = 5;
  else if (weapon_catalogue_is_ballistic(weapindx) &&
           (ammoMode & AC_FLECHETTE_MODE))
    bth = 5;
  else if (weapon_catalogue_is_energy(weapindx) && weapon_can_ignite(weapindx))
    bth = 5;
  else if ((weapon_catalogue_is_missile(weapindx) ||
            weapon_catalogue_is_ballistic(weapindx)) &&
           weapon_can_ignite(weapindx))
    bth = 9;

  if (roll >= bth)
    fire_hex(&(TerrainHexEffectRequest){.mech = mech,
                                        .position = {.x = x, .y = y},
                                        .intentional = request->intentional});
}

static void
mech_terrain_possibly_clear(const TerrainWeaponEffectRequest *request) {
  Mech *mech = request->mech;
  const int weapindx = request->weapon_index;
  const int damage = request->damage;
  const int x = request->position.x;
  const int y = request->position.y;
  int igniteBTH = 5; /* This is for intentional clearing */
  int igniteRoll = btech_random_roll(mech_context(mech));
  int clearRoll = btech_random_roll(mech_context(mech));

  if (weapon_catalogue_is_personal_combat(weapindx))
    return;

  if (!request->intentional)
    igniteBTH = 3;

  if (igniteRoll <= igniteBTH) {
    mech_terrain_possibly_ignite(request);
    return;
  }

  if (!weapon_can_clear(weapindx))
    return;

  if (clearRoll > damage)
    return;

  clear_hex(&(TerrainHexEffectRequest){.mech = mech,
                                       .position = {.x = x, .y = y},
                                       .intentional = request->intentional});
  mine_field_possibly_remove(mech, x, y);
}

void mech_terrain_possibly_ignite_or_clear(
    const TerrainWeaponEffectRequest *request) {
  Mech *mech = request->mech;
  BattleMap *map;

  map = btech_context_find_object(mech_context(mech), mech_map_dbref(mech));

  if (!map)
    return;

  if (mech_targets_hex_for_ignition(mech)) {
    TerrainWeaponEffectRequest effect = *request;
    effect.map = map;
    effect.intentional = true;
    mech_terrain_possibly_ignite(&effect);
    return;
  }

  if (mech_targets_hex_for_clearing(mech)) {
    TerrainWeaponEffectRequest effect = *request;
    effect.map = map;
    effect.intentional = true;
    mech_terrain_possibly_clear(&effect);
    return;
  }

  TerrainWeaponEffectRequest effect = *request;
  effect.map = map;
  mech_terrain_possibly_clear(&effect);
}

void mech_terrain_hex_hit(const TerrainWeaponHitRequest *request) {
  Mech *mech = request->attacker;
  const int weapindx = request->weapon_index;
  const int x = request->position.x;
  const int y = request->position.y;
  if (!mech_targets_hex_or_building(mech))
    return;

  /* Ok.. we either try to clear/ignite the hex, or alternatively we try to hit
   * building in it */
  if (mech_targets_building(mech)) {
    if (request->hit)
      hit_building(&(BuildingHitRequest){.mech = mech,
                                         .position = {.x = x, .y = y},
                                         .weapon_index = weapindx,
                                         .damage = request->damage});
  } else {
    mech_terrain_possibly_ignite_or_clear(&(TerrainWeaponEffectRequest){
        .mech = mech,
        .position = request->position,
        .weapon_index = weapindx,
        .ammunition_mode = request->ammunition_mode,
        .damage = request->damage,
        .intentional = true});

    if (mech_targets_hex(mech)) {
      const TerrainStructureWeaponImpact impact = {
          .attacker = mech,
          .weapon_index = weapindx,
          .position = {.x = x, .y = y},
      };
      ice_weapon_impact_resolve(&impact);
      bridge_weapon_impact_resolve(&impact);
    }
  }
}

/****************************************
 * End: Hex hitting related functions
 ****************************************/
