/*
 * power_fields.c - object power field accessors
 */

#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/server/platform.h"

bool game_object_has_power(const ObjectPowerRequest *request) {
  const GameObject *game_object =
      game_database_object(request->database, request->object);

  switch (request->power) {
  case POWER_IDLE:
    return game_object->has_idle_power;
  case POWER_NONE:
  case POWER_COUNT:
    return false;
  }
  return false;
}

void game_object_set_power(const ObjectPowerChange *change) {
  GameObject *game_object =
      game_database_object(change->target.database, change->target.object);

  switch (change->target.power) {
  case POWER_IDLE:
    game_object->has_idle_power = change->value;
    break;
  case POWER_NONE:
  case POWER_COUNT:
    break;
  }
}

void game_object_clear_powers(GameDatabase *database, DbRef object) {
  GameObject *game_object = game_database_object(database, object);

  game_object->has_idle_power = false;
}
