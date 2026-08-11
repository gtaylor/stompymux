/* economy_parts.c -- Normalized BattleTech parts inventory tests. */

#include "mux/objects/economy_parts.h"
#include "mux/objects/db.h"

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {.object_storage = objects, .top = 2, .size = 2};

  game_object_set_type(&database, 0, OBJECT_TYPE_THING);
  game_object_set_type(&database, 1, OBJECT_TYPE_THING);
  if (!economy_parts_set_quantity(&database, 0, 10, 2, 3) ||
      !economy_parts_set_quantity(&database, 0, 4, 1, 7))
    return 1;
  EconomyPartsEntryResult entry_result =
      economy_parts_entry(&(EconomyPartsEntryRequest){
          .database = &database, .object = 0, .index = 1});
  EconomyPartEntryView entry = entry_result.entry;
  if (economy_parts_quantity(&database, 0, 10, 2) != 3 ||
      economy_parts_entry_count(&database, 0) != 2 || !entry_result.found ||
      entry.part_id != 4 || entry.brand_id != 1 || entry.quantity != 7 ||
      !economy_parts_set_quantity(&database, 0, 10, 2, 0) ||
      economy_parts_quantity(&database, 0, 10, 2) != 0 ||
      economy_parts_entry_count(&database, 0) != 1)
    return 1;

  if (!economy_parts_set_quantity(&database, 1, 8, 3, 2))
    return 1;
  game_object_set_type(&database, 1, OBJECT_TYPE_GARBAGE);
  economy_parts_clear(&database, 1);
  if (game_database_object(&database, 1)->economy_parts.entries ||
      game_database_object(&database, 1)->economy_parts.count != 0)
    return 1;
  economy_parts_clear(&database, 0);
  return game_database_object(&database, 0)->economy_parts.entries ||
                 game_database_object(&database, 0)->economy_parts.count
             ? 1
             : 0;
}
