/* flag_privileges.c -- role-only object control unit test */

#include "mux/database/flags.h"

int main(void) {
  GameObject objects[5] = {0};
  GameDatabase database = {
      .objects = objects,
      .top = 5,
      .size = 5,
  };

  objects[GOD].flags = TYPE_PLAYER | WIZARD;
  objects[2].flags = TYPE_PLAYER | WIZARD;
  objects[3].flags = TYPE_PLAYER;
  objects[4].flags = TYPE_THING;

  if (!is_controls(&database, GOD, GOD) ||
      !is_controls(&database, GOD, 2) ||
      !is_controls(&database, GOD, 3) ||
      !is_controls(&database, GOD, 4))
    return 1;

  if (is_controls(&database, 2, GOD) || is_controls(&database, 2, 2) ||
      !is_controls(&database, 2, 3) || !is_controls(&database, 2, 4))
    return 1;

  if (is_controls(&database, 3, 3) || is_controls(&database, 3, 4) ||
      is_examinable(&database, 3, 3))
    return 1;

  return is_examinable(&database, 2, 4) ? 0 : 1;
}
