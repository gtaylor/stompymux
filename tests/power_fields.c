/* power_fields.c -- individual object power field unit test */

#include "mux/objects/powers.h"

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {
      .objects = objects,
      .top = 2,
      .size = 2,
  };
  const PowerId powers[] = {
      POWER_IDLE,       POWER_LONG_FINGERS, POWER_COMM_ALL, POWER_SEE_HIDDEN,
      POWER_NO_DESTROY, POWER_MECH,         POWER_SECURITY, POWER_MECHREP,
      POWER_MAP,        POWER_TEMPLATE,     POWER_TECH,
  };

  for (size_t index = 0; index < sizeof(powers) / sizeof(powers[0]); index++) {
    if (game_object_has_power(&database, 0, powers[index]))
      return 1;
    game_object_set_power(&database, 0, powers[index], true);
    if (!game_object_has_power(&database, 0, powers[index]))
      return 1;
  }

  if (!objects[0].has_idle_power || !objects[0].has_long_fingers_power ||
      !objects[0].has_comm_all_power || !objects[0].has_see_hidden_power ||
      !objects[0].has_no_destroy_power || !objects[0].has_mech_power ||
      !objects[0].has_security_power || !objects[0].has_mechrep_power ||
      !objects[0].has_map_power || !objects[0].has_template_power ||
      !objects[0].has_tech_power)
    return 1;

  game_object_clear_powers(&database, 0);
  for (size_t index = 0; index < sizeof(powers) / sizeof(powers[0]); index++) {
    if (game_object_has_power(&database, 0, powers[index]))
      return 1;
  }

  game_object_set_power(&database, 0, POWER_NONE, true);
  if (game_object_has_power(&database, 0, POWER_NONE))
    return 1;

  return 0;
}
