#include "mech_combat_misc_api.h"
#include "mech_equipment_api.h"
#include "mech_utils_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

void mech_ammunition_decrement(const AmmunitionDecrementRequest *request) {
  Mech *mech = request->mech;
  const int WEAPON_INDEX = request->weapon_index;
  const int SECTION = request->weapon.section;
  const int CRITICAL = request->weapon.critical;

  if (weapon_catalogue_is_energy(WEAPON_INDEX) ||
      weapon_catalogue_is_hand_to_hand(WEAPON_INDEX))
    return;

  if (weapon_catalogue_is_only_rocket(WEAPON_INDEX)) {
    const int WEAPON_SIZE = get_weapon_crits(mech, WEAPON_INDEX);
    const int FIRST_CRITICAL =
        mech_weapon_first_critical(&(WeaponCriticalSearch){
            .mech = mech,
            .weapon = {.section = SECTION, .critical = CRITICAL},
            .start_critical = 0,
            .part_type = mech_critical_part_type(mech, SECTION, CRITICAL),
            .maximum_criticals = WEAPON_SIZE,
        });

    for (int index = FIRST_CRITICAL; index < FIRST_CRITICAL + WEAPON_SIZE;
         ++index)
      mech_critical_fire_mode_add(mech, SECTION, index, ROCKET_FIRED);
    return;
  }

  if (mech_critical_fire_mode(mech, SECTION, CRITICAL) & OS_MODE) {
    mech_critical_fire_mode_add(mech, SECTION, CRITICAL, OS_USED);
    return;
  }

  const bool ROTARY = weapon_catalogue_is_rotary_autocannon(WEAPON_INDEX);
  const bool GATLING =
      (mech_critical_fire_mode(mech, SECTION, CRITICAL) & GATTLING_MODE) != 0;
  if (!ROTARY && !GATLING && !request->primary_ammunition.found)
    return;

  const bool DOUBLE_RATE = (mech_critical_fire_mode(mech, SECTION, CRITICAL) &
                            (ULTRA_MODE | RFAC_MODE)) != 0;
  const int WARNING_ROUNDS = request->gatling_shots > (int)DOUBLE_RATE
                                 ? request->gatling_shots
                                 : (int)DOUBLE_RATE;
  mech_ammunition_expenditure_check(&(AmmunitionExpenditureCheck){
      .mech = mech,
      .weapon_index = WEAPON_INDEX,
      .rounds_remaining = WARNING_ROUNDS,
  });

  if (ROTARY || GATLING) {
    int shots_left = GATLING ? request->gatling_shots * 3 : 1;
    const int FIRE_MODE = mech_critical_fire_mode(mech, SECTION, CRITICAL);
    if (ROTARY && (FIRE_MODE & RAC_TWOSHOT_MODE))
      shots_left = 2;
    else if (ROTARY && (FIRE_MODE & RAC_FOURSHOT_MODE))
      shots_left = 4;
    else if (ROTARY && (FIRE_MODE & RAC_SIXSHOT_MODE))
      shots_left = 6;

    while (shots_left > 0) {
      const CriticalSlotLookupResult AMMUNITION =
          ammunition_find(&(AmmunitionLookupRequest){
              .mech = mech,
              .weapon = {.section = SECTION, .critical = CRITICAL},
              .use_weapon_preference = true,
              .weapon_index = WEAPON_INDEX,
              .start_section = SECTION,
              .forbidden_modes = AMMO_MODES,
          });
      if (!AMMUNITION.found)
        break;

      const int ROUNDS = mech_critical_data(mech, AMMUNITION.slot.section,
                                            AMMUNITION.slot.critical);
      const int SPENT = ROUNDS < shots_left ? ROUNDS : shots_left;
      mech_critical_data_set(mech, AMMUNITION.slot.section,
                             AMMUNITION.slot.critical, ROUNDS - SPENT);
      shots_left -= SPENT;

      if (count_ammo_for_weapon(mech, WEAPON_INDEX) <= 0)
        break;
    }
    return;
  }

  const CriticalSlotReference PRIMARY = request->primary_ammunition.slot;
  const int PRIMARY_ROUNDS =
      mech_critical_data(mech, PRIMARY.section, PRIMARY.critical);
  if (PRIMARY_ROUNDS > 0)
    mech_critical_data_set(mech, PRIMARY.section, PRIMARY.critical,
                           PRIMARY_ROUNDS - 1);

  if (!DOUBLE_RATE || !request->secondary_ammunition.found)
    return;

  const CriticalSlotReference SECONDARY = request->secondary_ammunition.slot;
  const int SECONDARY_ROUNDS =
      mech_critical_data(mech, SECONDARY.section, SECONDARY.critical);
  if (SECONDARY_ROUNDS > 0)
    mech_critical_data_set(mech, SECONDARY.section, SECONDARY.critical,
                           SECONDARY_ROUNDS - 1);
}
