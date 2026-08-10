/* Resolves weapon failures independently from command parsing. */

#include "btech/context.h"
#include "failures.h"
#include "failures_api.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "weapon_catalogue_api.h"

WeaponFailureResolution
weapon_failure_resolve(const WeaponFailureResolutionRequest *request) {
  MechWeaponFailureRequest failure_request = {
      .mech = request->mech,
      .weapon_number = request->weapon_number,
      .weapon_type = request->weapon_index,
      .section = request->weapon.section,
      .critical = request->weapon.critical,
  };
  PartFailureResult failure = mech_weapon_failure_check(&failure_request);
  WeaponFailureResolution result = {
      .range_ok = true,
      .modifier = failure.modifier,
      .type = failure.type,
  };
  if (result.type == POWER_SPIKE) {
    result.handled = true;
    return result;
  }
  if (result.type == WEAPON_JAMMED || result.type == WEAPON_DUD) {
    mech_ammunition_decrement(&(AmmunitionDecrementRequest){
        .mech = request->mech,
        .weapon_index = request->weapon_index,
        .weapon = request->weapon,
        .primary_ammunition = request->primary_ammunition,
        .secondary_ammunition = request->secondary_ammunition,
        .gatling_shots = request->gatling_shots,
    });
    result.handled = true;
    return result;
  }
  if (result.type == RANGE) {
    const BtechContext *context = mech_context(request->mech);
    const bool extended = btech_context_uses_extended_weapon_ranges(context);
    const int effective_range =
        mech_section_is_underwater(request->mech, request->weapon.section)
            ? weapon_catalogue_effective_water_range(request->weapon_index,
                                                     extended)
            : weapon_catalogue_effective_range(request->weapon_index, extended);
    if ((float)(effective_range - result.modifier) < request->range) {
      mech_notify(
          request->mech, MECHALL,
          "Due to weapons failure your shot falls short of its target!");
      result.range_ok = false;
    }
  }
  return result;
}
