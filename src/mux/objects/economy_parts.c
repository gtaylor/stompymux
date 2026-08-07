/* economy_parts.c - Normalized BattleTech parts inventory state. */

#include "mux/objects/economy_parts.h"

#include <stdint.h>
#include <stdlib.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"

void economy_parts_clear(GameDatabase *database, DbRef object) {
  if (!database || object < 0 || object >= database->top)
    return;
  free(game_database_object(database, object)->economy_parts.entries);
  game_database_object(database, object)->economy_parts.entries = nullptr;
  game_database_object(database, object)->economy_parts.count = 0;
}

static size_t economy_parts_find(GameDatabase *database, DbRef object,
                                 int part_id, int brand_id) {
  EconomyPartsState *parts =
      &game_database_object(database, object)->economy_parts;
  for (size_t index = 0; index < parts->count; index++)
    if (parts->entries[index].part_id == part_id &&
        parts->entries[index].brand_id == brand_id)
      return index;
  return parts->count;
}

size_t economy_parts_entry_count(GameDatabase *database, DbRef object) {
  if (!is_good_obj(database, object))
    return 0;
  return game_database_object(database, object)->economy_parts.count;
}

bool economy_parts_entry(GameDatabase *database, DbRef object, size_t index,
                         EconomyPartEntryView *entry) {
  EconomyPartsState *parts;

  if (!is_good_obj(database, object) || !entry)
    return false;
  parts = &game_database_object(database, object)->economy_parts;
  if (index >= parts->count)
    return false;
  *entry = (EconomyPartEntryView){
      .part_id = parts->entries[index].part_id,
      .brand_id = parts->entries[index].brand_id,
      .quantity = parts->entries[index].quantity,
  };
  return true;
}

int economy_parts_quantity(GameDatabase *database, DbRef object, int part_id,
                           int brand_id) {
  size_t index;
  EconomyPartsState *parts;

  if (!is_good_obj(database, object))
    return 0;
  parts = &game_database_object(database, object)->economy_parts;
  index = economy_parts_find(database, object, part_id, brand_id);
  return index < parts->count ? parts->entries[index].quantity : 0;
}

bool economy_parts_set_quantity(GameDatabase *database, DbRef object,
                                int part_id, int brand_id, int quantity) {
  EconomyPartsState *parts;
  size_t index;

  if (!is_good_obj(database, object))
    return false;
  parts = &game_database_object(database, object)->economy_parts;
  index = economy_parts_find(database, object, part_id, brand_id);
  if (quantity <= 0) {
    if (index == parts->count)
      return true;
    parts->entries[index] = parts->entries[parts->count - 1];
    parts->count--;
    if (parts->count == 0)
      economy_parts_clear(database, object);
    return true;
  }
  if (index < parts->count) {
    parts->entries[index].quantity = quantity;
    return true;
  }
  if (parts->count == SIZE_MAX / sizeof(*parts->entries))
    return false;
  EconomyPartEntry *grown =
      realloc(parts->entries, (parts->count + 1) * sizeof(*grown));
  if (!grown)
    return false;
  parts->entries = grown;
  parts->entries[parts->count++] = (EconomyPartEntry){
      .part_id = part_id,
      .brand_id = brand_id,
      .quantity = quantity,
  };
  return true;
}
