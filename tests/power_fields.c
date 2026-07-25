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
      POWER_IDLE,
  };

  for (size_t index = 0; index < sizeof(powers) / sizeof(powers[0]); index++) {
    if (game_object_has_power(&database, 0, powers[index]))
      return 1;
    game_object_set_power(&database, 0, powers[index], true);
    if (!game_object_has_power(&database, 0, powers[index]))
      return 1;
  }

  if (!objects[0].has_idle_power)
    return 1;

  game_object_clear_powers(&database, 0);
  for (size_t index = 0; index < sizeof(powers) / sizeof(powers[0]); index++) {
    if (game_object_has_power(&database, 0, powers[index]))
      return 1;
  }

  game_object_set_power(&database, 0, POWER_NONE, true);
  game_object_set_power(&database, 0, POWER_COUNT, true);
  if (game_object_has_power(&database, 0, POWER_NONE) ||
      game_object_has_power(&database, 0, POWER_COUNT))
    return 1;

  return 0;
}
