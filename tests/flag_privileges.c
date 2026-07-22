/* flag_privileges.c -- role-only object control unit test */

#include "mux/objects/flags.h"

int main(void) {
  GameObject objects[5] = {0};
  GameDatabase database = {
      .objects = objects,
      .top = 5,
      .size = 5,
  };

  objects[GOD].type = OBJECT_TYPE_PLAYER;
  objects[GOD].has_wizard_flag = true;
  objects[2].type = OBJECT_TYPE_PLAYER;
  objects[2].has_wizard_flag = true;
  objects[3].type = OBJECT_TYPE_PLAYER;
  objects[4].type = OBJECT_TYPE_THING;

  if (!is_controls(&database, GOD, GOD) || !is_controls(&database, GOD, 2) ||
      !is_controls(&database, GOD, 3) || !is_controls(&database, GOD, 4))
    return 1;

  if (is_controls(&database, 2, GOD) || is_controls(&database, 2, 2) ||
      !is_controls(&database, 2, 3) || !is_controls(&database, 2, 4))
    return 1;

  if (is_controls(&database, 3, 3) || is_controls(&database, 3, 4) ||
      is_examinable(&database, 3, 3))
    return 1;

  return is_examinable(&database, 2, 4) ? 0 : 1;
}
