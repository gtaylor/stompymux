#include "equipment_types.h"
#include "floatsim.h"
#include "mech_partnames.h"

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
