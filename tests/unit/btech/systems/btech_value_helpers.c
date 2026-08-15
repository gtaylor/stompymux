#include "equipment_types.h"
#include "floatsim.h"
#include "mech_partnames.h"
#include "mux/support/checked_storage.h"
#include "template_internal.h"

#include <string.h>

static int next_equipment(int *evaluations, int equipment) {
  ++*evaluations;
  return equipment;
}

int main(void) {
  int evaluations = 0;
  int id_evaluations = 0;
  int brand_evaluations = 0;

  if (!equipment_is_weapon(
          next_equipment(&evaluations, weapon_equipment_index(3))) ||
      evaluations != 1) {
    return 1;
  }

  evaluations = 0;
  if (equipment_is_ammunition(
          next_equipment(&evaluations, weapon_equipment_index(3))) ||
      evaluations != 1) {
    return 1;
  }

  if (weapon_from_equipment_index(weapon_equipment_index(17)) != 17 ||
      ammunition_to_weapon_index(ammunition_equipment_index(17)) != 17 ||
      special_from_equipment_index(special_equipment_index(9)) != 9 ||
      cargo_from_equipment_index(cargo_equipment_index(11)) != 11 ||
      bomb_from_equipment_index(bomb_equipment_index(4)) != 4) {
    return 1;
  }

  const char *const EXPECTED_CARGO_TAIL[] = {
      "Ammo_ATM3_ER",  "Ammo_ATM3_HE",  "Ammo_ATM6_ER",
      "Ammo_ATM6_HE",  "Ammo_ATM9_ER",  "Ammo_ATM9_HE",
      "Ammo_ATM12_ER", "Ammo_ATM12_HE", "SearchLight",
  };
  constexpr size_t CARGO_TAIL_COUNT =
      sizeof(EXPECTED_CARGO_TAIL) / sizeof(EXPECTED_CARGO_TAIL[0]);
  constexpr int AMMUNITION_WEIGHT = 1024;
  constexpr int SEARCHLIGHT_WEIGHT = 204;
  if (TEMPLATE_CARGO_COUNT != 158) {
    return 1;
  }
  for (size_t index = 0; index < CARGO_TAIL_COUNT; ++index) {
    const int cargo_index = AMMO_ATM3_ER + (int)index;
    const char *const *expected_name =
        checked_storage_at_const(EXPECTED_CARGO_TAIL, CARGO_TAIL_COUNT,
                                 sizeof(*EXPECTED_CARGO_TAIL), index);
    const int *weight =
        checked_storage_at_const(CARGOWEIGHT, (size_t)TEMPLATE_CARGO_COUNT,
                                 sizeof(*CARGOWEIGHT), (size_t)cargo_index);
    if (strcmp(template_cargo_name(cargo_index), *expected_name) ||
        *weight != (index + 1 == CARGO_TAIL_COUNT ? SEARCHLIGHT_WEIGHT
                                                  : AMMUNITION_WEIGHT)) {
      return 1;
    }
  }

  int packed = packed_part(next_equipment(&id_evaluations, 7),
                           next_equipment(&brand_evaluations, 3));
  if (id_evaluations != 1 || brand_evaluations != 1 ||
      packed_part_id(packed) != 7 || packed_part_brand(packed) != 3) {
    return 1;
  }

  return float_simulation_to_int(int_to_float_simulation(27)) == 27 &&
                 float_simulation_to_short(short_to_float_simulation(13)) == 13
             ? 0
             : 1;
}
