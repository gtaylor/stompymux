#include "mech_combat_misc_api.h"
#include "mech_equipment_api.h"
#include "mech_utils_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

void mech_ammunition_decrement(const AmmunitionDecrementRequest *request) {
  Mech *mech = request->mech;
  const int weapon_index = request->weapon_index;
  const int section = request->weapon.section;
  const int critical = request->weapon.critical;

  if (weapon_catalogue_is_energy(weapon_index) ||
      weapon_catalogue_is_hand_to_hand(weapon_index))
    return;

  if (weapon_catalogue_is_only_rocket(weapon_index)) {
    const int weapon_size = GetWeaponCrits(mech, weapon_index);
    const int first_critical =
        mech_weapon_first_critical(&(WeaponCriticalSearch){
            .mech = mech,
            .weapon = {.section = section, .critical = critical},
            .start_critical = 0,
            .part_type = mech_critical_part_type(mech, section, critical),
            .maximum_criticals = weapon_size,
        });

    for (int index = first_critical; index < first_critical + weapon_size;
         ++index)
      mech_critical_fire_mode_add(mech, section, index, ROCKET_FIRED);
    return;
  }

  if (mech_critical_fire_mode(mech, section, critical) & OS_MODE) {
    mech_critical_fire_mode_add(mech, section, critical, OS_USED);
    return;
  }

  const bool rotary = weapon_catalogue_is_rotary_autocannon(weapon_index);
  const bool gatling =
      (mech_critical_fire_mode(mech, section, critical) & GATTLING_MODE) != 0;
  if (!rotary && !gatling && !request->primary_ammunition.found)
    return;

  const bool double_rate = (mech_critical_fire_mode(mech, section, critical) &
                            (ULTRA_MODE | RFAC_MODE)) != 0;
  const int warning_rounds = request->gatling_shots > (int)double_rate
                                 ? request->gatling_shots
                                 : (int)double_rate;
  mech_ammunition_expenditure_check(&(AmmunitionExpenditureCheck){
      .mech = mech,
      .weapon_index = weapon_index,
      .rounds_remaining = warning_rounds,
  });

  if (rotary || gatling) {
    int shots_left = gatling ? request->gatling_shots * 3 : 1;
    const int fire_mode = mech_critical_fire_mode(mech, section, critical);
    if (rotary && (fire_mode & RAC_TWOSHOT_MODE))
      shots_left = 2;
    else if (rotary && (fire_mode & RAC_FOURSHOT_MODE))
      shots_left = 4;
    else if (rotary && (fire_mode & RAC_SIXSHOT_MODE))
      shots_left = 6;

    while (shots_left > 0) {
      const CriticalSlotLookupResult ammunition =
          ammunition_find(&(AmmunitionLookupRequest){
              .mech = mech,
              .weapon = {.section = section, .critical = critical},
              .use_weapon_preference = true,
              .weapon_index = weapon_index,
              .start_section = section,
              .forbidden_modes = AMMO_MODES,
          });
      if (!ammunition.found)
        break;

      const int rounds = mech_critical_data(mech, ammunition.slot.section,
                                            ammunition.slot.critical);
      const int spent = rounds < shots_left ? rounds : shots_left;
      mech_critical_data_set(mech, ammunition.slot.section,
                             ammunition.slot.critical, rounds - spent);
      shots_left -= spent;

      if (CountAmmoForWeapon(mech, weapon_index) <= 0)
        break;
    }
    return;
  }

  const CriticalSlotReference primary = request->primary_ammunition.slot;
  const int primary_rounds =
      mech_critical_data(mech, primary.section, primary.critical);
  if (primary_rounds > 0)
    mech_critical_data_set(mech, primary.section, primary.critical,
                           primary_rounds - 1);

  if (!double_rate || !request->secondary_ammunition.found)
    return;

  const CriticalSlotReference secondary = request->secondary_ammunition.slot;
  const int secondary_rounds =
      mech_critical_data(mech, secondary.section, secondary.critical);
  if (secondary_rounds > 0)
    mech_critical_data_set(mech, secondary.section, secondary.critical,
                           secondary_rounds - 1);
}
