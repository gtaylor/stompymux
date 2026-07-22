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
  case POWER_LONG_FINGERS:
    return game_object->has_long_fingers_power;
  case POWER_COMM_ALL:
    return game_object->has_comm_all_power;
  case POWER_SEE_HIDDEN:
    return game_object->has_see_hidden_power;
  case POWER_NO_DESTROY:
    return game_object->has_no_destroy_power;
  case POWER_PASS_LOCKS:
    return game_object->has_pass_locks_power;
  case POWER_MECH:
    return game_object->has_mech_power;
  case POWER_SECURITY:
    return game_object->has_security_power;
  case POWER_MECHREP:
    return game_object->has_mechrep_power;
  case POWER_MAP:
    return game_object->has_map_power;
  case POWER_TEMPLATE:
    return game_object->has_template_power;
  case POWER_TECH:
    return game_object->has_tech_power;
  case POWER_NONE:
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
  case POWER_LONG_FINGERS:
    game_object->has_long_fingers_power = value;
    break;
  case POWER_COMM_ALL:
    game_object->has_comm_all_power = value;
    break;
  case POWER_SEE_HIDDEN:
    game_object->has_see_hidden_power = value;
    break;
  case POWER_NO_DESTROY:
    game_object->has_no_destroy_power = value;
    break;
  case POWER_PASS_LOCKS:
    game_object->has_pass_locks_power = value;
    break;
  case POWER_MECH:
    game_object->has_mech_power = value;
    break;
  case POWER_SECURITY:
    game_object->has_security_power = value;
    break;
  case POWER_MECHREP:
    game_object->has_mechrep_power = value;
    break;
  case POWER_MAP:
    game_object->has_map_power = value;
    break;
  case POWER_TEMPLATE:
    game_object->has_template_power = value;
    break;
  case POWER_TECH:
    game_object->has_tech_power = value;
    break;
  case POWER_NONE:
    break;
  }
}

void game_object_clear_powers(GameDatabase *database, DbRef object) {
  GameObject *game_object = game_database_object(database, object);

  game_object->has_idle_power = false;
  game_object->has_long_fingers_power = false;
  game_object->has_comm_all_power = false;
  game_object->has_see_hidden_power = false;
  game_object->has_no_destroy_power = false;
  game_object->has_pass_locks_power = false;
  game_object->has_mech_power = false;
  game_object->has_security_power = false;
  game_object->has_mechrep_power = false;
  game_object->has_map_power = false;
  game_object->has_template_power = false;
  game_object->has_tech_power = false;
}
