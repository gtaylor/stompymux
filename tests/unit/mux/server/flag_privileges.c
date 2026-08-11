/* flag_privileges.c -- role-only object control unit test */

#include "mux/objects/flags.h"

int main(void) {
  GameObject objects[6] = {0};
  GameDatabase database = {
      .object_storage = objects,
      .top = 5,
      .size = 5,
  };

  game_database_object(&database, GOD)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, GOD)->has_wizard_flag = true;
  game_database_object(&database, 2)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, 2)->has_wizard_flag = true;
  game_database_object(&database, 3)->type = OBJECT_TYPE_PLAYER;
  game_database_object(&database, 4)->type = OBJECT_TYPE_THING;

  if (!is_controls(&database, GOD, GOD) || !is_controls(&database, GOD, 2) ||
      !is_controls(&database, GOD, 3) || !is_controls(&database, GOD, 4))
    return 1;

  if (is_controls(&database, 2, GOD) || !is_controls(&database, 2, 2) ||
      !is_controls(&database, 2, 3) || !is_controls(&database, 2, 4))
    return 1;

  if (is_controls(&database, 3, 3) || is_controls(&database, 3, 4) ||
      is_examinable(&database, 3, 3))
    return 1;

  return is_examinable(&database, 2, 4) ? 0 : 1;
}
