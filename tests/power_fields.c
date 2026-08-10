/* power_fields.c -- individual object power field unit test */

#include "mux/objects/powers.h"

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {
      .object_storage = objects,
      .top = 2,
      .size = 2,
  };
  if (game_object_has_power(&(ObjectPowerRequest){
          .database = &database, .object = 0, .power = POWER_IDLE}))
    return 1;
  game_object_set_power(&(ObjectPowerChange){
      .target = {.database = &database, .object = 0, .power = POWER_IDLE},
      .value = true});
  if (!game_object_has_power(&(ObjectPowerRequest){
          .database = &database, .object = 0, .power = POWER_IDLE}))
    return 1;

  if (!game_database_object(&database, 0)->has_idle_power)
    return 1;

  game_object_clear_powers(&database, 0);
  if (game_object_has_power(&(ObjectPowerRequest){
          .database = &database, .object = 0, .power = POWER_IDLE}))
    return 1;

  game_object_set_power(&(ObjectPowerChange){
      .target = {.database = &database, .object = 0, .power = POWER_NONE},
      .value = true});
  game_object_set_power(&(ObjectPowerChange){
      .target = {.database = &database, .object = 0, .power = POWER_COUNT},
      .value = true});
  if (game_object_has_power(&(ObjectPowerRequest){
          .database = &database, .object = 0, .power = POWER_NONE}) ||
      game_object_has_power(&(ObjectPowerRequest){
          .database = &database, .object = 0, .power = POWER_COUNT}))
    return 1;

  return 0;
}
