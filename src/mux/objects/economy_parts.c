/* economy_parts.c - Normalized BattleTech parts inventory state. */

#include "mux/objects/economy_parts.h"

#include <stdint.h>
#include <stdlib.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static EconomyPartEntry *economy_part(EconomyPartsState *parts, size_t count,
                                      size_t index) {
  return checked_storage_at(parts->entries, count, sizeof(*parts->entries),
                            index);
}

void economy_parts_clear(GameDatabase *database, DbRef object) {
  if (!database || object < 0 || object >= database->top)
    return;
  free(game_database_object(database, object)->economy_parts.entries);
  game_database_object(database, object)->economy_parts.entries = nullptr;
  game_database_object(database, object)->economy_parts.count = 0;
}

typedef struct EconomyPartReference {
  GameDatabase *database;
  DbRef object;
  int part_id;
  int brand_id;
} EconomyPartReference;

static size_t economy_parts_find(const EconomyPartReference *reference) {
  EconomyPartsState *parts =
      &game_database_object(reference->database, reference->object)
           ->economy_parts;
  for (size_t index = 0; index < parts->count; index++)
    if (economy_part(parts, parts->count, index)->part_id ==
            reference->part_id &&
        economy_part(parts, parts->count, index)->brand_id ==
            reference->brand_id)
      return index;
  return parts->count;
}

size_t economy_parts_entry_count(GameDatabase *database, DbRef object) {
  if (!is_good_obj(database, object))
    return 0;
  return game_database_object(database, object)->economy_parts.count;
}

EconomyPartsEntryResult
economy_parts_entry(const EconomyPartsEntryRequest *request) {
  EconomyPartsState *parts;

  if (!is_good_obj(request->database, request->object))
    return (EconomyPartsEntryResult){0};
  parts =
      &game_database_object(request->database, request->object)->economy_parts;
  if (request->index >= parts->count)
    return (EconomyPartsEntryResult){0};
  const EconomyPartEntry *stored =
      economy_part(parts, parts->count, request->index);
  return (EconomyPartsEntryResult){.found = true,
                                   .entry = {.part_id = stored->part_id,
                                             .brand_id = stored->brand_id,
                                             .quantity = stored->quantity}};
}

int economy_parts_quantity(GameDatabase *database, DbRef object, int part_id,
                           int brand_id) {
  size_t index;
  EconomyPartsState *parts;

  if (!is_good_obj(database, object))
    return 0;
  parts = &game_database_object(database, object)->economy_parts;
  index = economy_parts_find(&(EconomyPartReference){.database = database,
                                                     .object = object,
                                                     .part_id = part_id,
                                                     .brand_id = brand_id});
  return index < parts->count
             ? economy_part(parts, parts->count, index)->quantity
             : 0;
}

bool economy_parts_set_quantity(GameDatabase *database, DbRef object,
                                int part_id, int brand_id, int quantity) {
  EconomyPartsState *parts;
  size_t index;

  if (!is_good_obj(database, object))
    return false;
  parts = &game_database_object(database, object)->economy_parts;
  index = economy_parts_find(&(EconomyPartReference){.database = database,
                                                     .object = object,
                                                     .part_id = part_id,
                                                     .brand_id = brand_id});
  if (quantity <= 0) {
    if (index == parts->count)
      return true;
    *economy_part(parts, parts->count, index) =
        *economy_part(parts, parts->count, parts->count - 1);
    parts->count--;
    if (parts->count == 0)
      economy_parts_clear(database, object);
    return true;
  }
  if (index < parts->count) {
    economy_part(parts, parts->count, index)->quantity = quantity;
    return true;
  }
  if (parts->count == SIZE_MAX / sizeof(*parts->entries))
    return false;
  EconomyPartEntry *grown =
      realloc(parts->entries, (parts->count + 1) * sizeof(*grown));
  if (!grown)
    return false;
  parts->entries = grown;
  *economy_part(parts, parts->count + 1, parts->count) = (EconomyPartEntry){
      .part_id = part_id,
      .brand_id = brand_id,
      .quantity = quantity,
  };
  parts->count++;
  return true;
}
