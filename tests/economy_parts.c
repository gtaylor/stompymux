/* economy_parts.c -- Normalized BattleTech parts inventory tests. */

#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

int main(void) {
  GameObject objects[2] = {0};
  GameDatabase database = {.objects = objects, .top = 2, .size = 2};
  EconomyPartEntryView entry;

  game_object_set_type(&database, 0, OBJECT_TYPE_THING);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);
  if (!economy_parts_set_quantity(&database, 0, 10, 2, 3) ||
      !economy_parts_set_quantity(&database, 0, 4, 1, 7) ||
      economy_parts_quantity(&database, 0, 10, 2) != 3 ||
      economy_parts_entry_count(&database, 0) != 2 ||
      !economy_parts_entry(&database, 0, 1, &entry) || entry.part_id != 4 ||
      entry.brand_id != 1 || entry.quantity != 7 ||
      !economy_parts_set_quantity(&database, 0, 10, 2, 0) ||
      economy_parts_quantity(&database, 0, 10, 2) != 0 ||
      economy_parts_entry_count(&database, 0) != 1)
    return 1;

  if (!economy_parts_set_quantity(&database, 1, 8, 3, 2))
    return 1;
  game_object_set_type(&database, 1, OBJECT_TYPE_GARBAGE);
  economy_parts_clear(&database, 1);
  if (objects[1].economy_parts.entries || objects[1].economy_parts.count != 0)
    return 1;
  economy_parts_clear(&database, 0);
  return objects[0].economy_parts.entries || objects[0].economy_parts.count
             ? 1
             : 0;
}
