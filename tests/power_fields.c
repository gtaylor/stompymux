/* power_fields.c -- individual object power field unit test */

#include "mux/objects/powers.h"

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {
      .object_storage = objects,
      .top = 2,
      .size = 2,
  };
  if (game_object_has_power(&database, 0, POWER_IDLE))
    return 1;
  game_object_set_power(&database, 0, POWER_IDLE, true);
  if (!game_object_has_power(&database, 0, POWER_IDLE))
    return 1;

  if (!game_database_object(&database, 0)->has_idle_power)
    return 1;

  game_object_clear_powers(&database, 0);
  if (game_object_has_power(&database, 0, POWER_IDLE))
    return 1;

  game_object_set_power(&database, 0, POWER_NONE, true);
  game_object_set_power(&database, 0, POWER_COUNT, true);
  if (game_object_has_power(&database, 0, POWER_NONE) ||
      game_object_has_power(&database, 0, POWER_COUNT))
    return 1;

  return 0;
}
