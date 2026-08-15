#include "mech_combat_api.h"
#include "mech_events.h"
#include "mech_fire_preparation_internal.h"

#include "btech_event.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

static void swap_ints(int *left, int *right) {
  const int TEMPORARY = *left;
  *left = *right;
  *right = TEMPORARY;
}

WeaponFirePreparation weapon_fire_prepare(const WeaponFireRequest *request,
                                          float range) {
  WeaponFirePreparation result = {.target = request->target};
  const int FIRE_MODE = mech_critical_fire_mode(
      request->mech, request->weapon.section, request->weapon.critical);
  const int AMMUNITION_MODE = mech_critical_ammo_mode(
      request->mech, request->weapon.section, request->weapon.critical);
  const WeaponRangeProfile RANGES =
      weapon_catalogue_ranges(request->weapon_index);

  if ((AMMUNITION_MODE & STINGER_MODE) && request->target_kind) {
    mech_notify(request->mech, MECHALL, "Stinger missiles cannot shoot hexes!");
    return result;
  }
  if ((AMMUNITION_MODE & STINGER_MODE) && result.target != nullptr &&
      !(mech_is_jumping(result.target) ||
        mech_cocoon_integrity(result.target) ||
        (mech_is_flying_type(result.target) &&
         !mech_is_landed(result.target)))) {
    mech_notify(request->mech, MECHALL,
                "Stinger missiles can only engage airborne targets!");
    return result;
  }
  if (weapon_catalogue_is_coolant(request->weapon_index) &&
      (FIRE_MODE & HEAT_MODE))
    result.target = request->mech;
  if (mech_section_is_underwater(request->mech, request->weapon.section) &&
      RANGES.water_short_range <= 0) {
    mech_notify(request->mech, MECHALL,
                "This weapon may not be fired underwater.");
    return result;
  }
  if (mech_event_count(request->mech, EVENT_UNSTUN_CREW)) {
    mech_notify(request->mech, MECHALL,
                "You are too stunned to fire a weapon!");
    return result;
  }
  if (mech_event_count(request->mech, EVENT_UNJAM_TURRET)) {
    mech_notify(request->mech, MECHALL,
                "You are too busy unjamming your turret!");
    return result;
  }
  if (mech_event_count(request->mech, EVENT_UNJAM_AMMO)) {
    mech_notify(request->mech, MECHALL, "You are too busy unjamming a weapon!");
    return result;
  }
  if (mech_event_count(request->mech, EVENT_REMOVE_PODS)) {
    mech_notify(request->mech, MECHALL,
                "You are too busy removing iNARC pods!");
    return result;
  }

  const DbRef SWARM_TARGET = mech_swarm_target(request->mech);
  if (SWARM_TARGET > 0 &&
      (result.target == nullptr || SWARM_TARGET != mech_dbref(result.target))) {
    mech_notify(request->mech, MECHALL,
                "You're too busy holding on for dear life!");
    return result;
  }
  result.swarm_attack = ((SWARM_TARGET > 0 && result.target != nullptr &&
                          SWARM_TARGET == mech_dbref(result.target)) != 0);

  int gatling_shots = 0;
  if (FIRE_MODE & GATTLING_MODE)
    gatling_shots = btech_random_range_int(mech_context(request->mech), 1, 6);
  result.ammunition.gatling_shots = gatling_shots;
  if (!request->sight) {
    result.ammunition = ammunition_check(&(AmmunitionCheckRequest){
        .mech = request->mech,
        .weapon_index = request->weapon_index,
        .weapon = {.section = request->weapon.section,
                   .critical = request->weapon.critical},
        .gatling_shots = gatling_shots,
    });
    if (!result.ammunition.available)
      return result;
  }

  if (!weapon_catalogue_is_artillery(request->weapon_index)) {
    const MechNormalToHitResult TO_HIT = mech_normal_to_hit_calculate(
        &(MechNormalToHitRequest){.attacker = request->mech,
                                  .map = request->map,
                                  .section = request->weapon.section,
                                  .critical = request->weapon.critical,
                                  .weapon_index = request->weapon_index,
                                  .range = range,
                                  .target = result.target,
                                  .indirect_fire = request->indirect_fire});
    result.base_to_hit = TO_HIT.value;
    result.c3_reference = TO_HIT.c3_reference;
    if (result.c3_reference) {
      result.c3_mech = btech_context_get_mech(mech_context(request->mech),
                                              result.c3_reference);
      if (result.c3_mech != nullptr &&
          (mech_team(result.c3_mech) != mech_team(request->mech) ||
           result.c3_reference == mech_dbref(request->mech)))
        result.c3_mech = nullptr;
    }
  } else {
    result.base_to_hit = mech_artillery_to_hit_calculate(
        &(MechArtilleryToHitRequest){.attacker = request->mech,
                                     .section = request->weapon.section,
                                     .weapon_index = request->weapon_index,
                                     .indirect = (!request->line_of_sight) != 0,
                                     .range = range});
  }
  if (result.swarm_attack)
    result.base_to_hit = 0;
  result.ready = true;
  return result;
}

int weapon_fire_roll(const WeaponFireRequest *request, float range) {
  if (!weapon_catalogue_is_dead_fire_missile(request->weapon_index) &&
      (!weapon_catalogue_is_extended_lrm(request->weapon_index) ||
       range >= (float)weapon_catalogue_ranges(request->weapon_index).minimum))
    return btech_random_roll(mech_context(request->mech));

  int first = btech_random_range_int(mech_context(request->mech), 1, 6);
  int second = btech_random_range_int(mech_context(request->mech), 1, 6);
  int third = btech_random_range_int(mech_context(request->mech), 1, 6);
  if (first > second)
    swap_ints(&first, &second);
  if (second > third)
    swap_ints(&second, &third);
  return first + second;
}
