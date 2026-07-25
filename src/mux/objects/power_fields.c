/*
 * power_fields.c - object power field accessors
 */

#include "mux/objects/db.h"
#include "mux/objects/powers.h"

bool game_object_has_power(GameDatabase *database, DbRef object, PowerId id) {
  const GameObject *game_object = game_database_object(database, object);

  switch (id) {
  case POWER_IDLE:
    return game_object->has_idle_power;
  case POWER_NONE:
  case POWER_COUNT:
    return false;
  }
  return false;
}

void game_object_set_power(GameDatabase *database, DbRef object, PowerId id,
                           bool value) {
  GameObject *game_object = game_database_object(database, object);

  switch (id) {
  case POWER_IDLE:
    game_object->has_idle_power = value;
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
