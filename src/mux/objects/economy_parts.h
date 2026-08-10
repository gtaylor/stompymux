/* economy_parts.h - Normalized BattleTech parts inventory state. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/server/platform.h"

typedef struct EconomyPartEntryView EconomyPartEntryView;
struct EconomyPartEntryView {
  int part_id;
  int brand_id;
  int quantity;
};

typedef struct GameDatabase GameDatabase;

void economy_parts_clear(GameDatabase *database, DbRef object);
size_t economy_parts_entry_count(GameDatabase *database, DbRef object);
typedef struct EconomyPartsEntryRequest {
  GameDatabase *database;
  DbRef object;
  size_t index;
} EconomyPartsEntryRequest;

typedef struct EconomyPartsEntryResult {
  bool found;
  EconomyPartEntryView entry;
} EconomyPartsEntryResult;

EconomyPartsEntryResult
economy_parts_entry(const EconomyPartsEntryRequest *request);
int economy_parts_quantity(GameDatabase *database, DbRef object, int part_id,
                           int brand_id);
bool economy_parts_set_quantity(GameDatabase *database, DbRef object,
                                int part_id, int brand_id, int quantity);
