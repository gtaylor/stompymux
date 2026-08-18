/** @file
 * Normalized BattleTech parts inventory state.
 */
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

/** Clears economy parts. @param[in,out] database Game database. @param[in]
 * object Game object. */

void economy_parts_clear(GameDatabase *database, DbRef object);
/** Counts economy parts entry. @param[in] database Game database. @param[in]
 * object Game object. */

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

/** Executes economy parts entry. @param[in] request Request. */

EconomyPartsEntryResult
economy_parts_entry(const EconomyPartsEntryRequest *request);
/** Executes economy parts quantity. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] part_id Part id. @param[in]
 * brand_id Brand id. */

int economy_parts_quantity(GameDatabase *database, DbRef object, int part_id,
                           int brand_id);
/** Sets quantity on economy parts. @param[in,out] database Game database.
 * @param[in] object Game object. @param[in] part_id Part id. @param[in]
 * brand_id Brand id. @param[in] quantity Quantity. */

bool economy_parts_set_quantity(GameDatabase *database, DbRef object,
                                int part_id, int brand_id, int quantity);
