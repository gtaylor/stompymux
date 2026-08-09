#include "equipment_types.h"
#include "mech_consistency_api.h"

#include "btech_channel.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "section_types.h"
#include <stddef.h>
#include <stdlib.h>

static const char mech_location_table[][2] = {
    {CTORSO, 1}, {LTORSO, 2}, {RTORSO, 2}, {LARM, 3},
    {RARM, 3},   {LLEG, 4},   {RLEG, 4},   {-1, 0}};
static const char quad_location_table[][2] = {
    {CTORSO, 1}, {LTORSO, 2}, {RTORSO, 2}, {LARM, 4},
    {RARM, 4},   {LLEG, 4},   {RLEG, 4},   {-1, 0}};
static const char internal_structure[][5] = {
    {10, 4, 3, 1, 2},      {15, 5, 4, 2, 3},     {20, 6, 5, 3, 4},
    {25, 8, 6, 4, 6},      {30, 10, 7, 5, 7},    {35, 11, 8, 6, 8},
    {40, 12, 10, 6, 10},   {45, 14, 11, 7, 11},  {50, 16, 12, 8, 12},
    {55, 18, 13, 9, 13},   {60, 20, 14, 10, 14}, {65, 21, 15, 10, 15},
    {70, 22, 15, 11, 15},  {75, 23, 16, 12, 16}, {80, 25, 17, 13, 17},
    {85, 27, 18, 14, 18},  {90, 29, 19, 15, 19}, {95, 30, 20, 16, 20},
    {100, 31, 21, 17, 21}, {-1, 0, 0, 0, 0}};

static char table_value(const void *table, size_t rows, size_t columns,
                        size_t row, size_t column) {
  const char *selected =
      checked_storage_at_const(table, rows, columns * sizeof(char), row);
  return *(const char *)checked_storage_at_const(selected, columns,
                                                 sizeof(char), column);
}

static int internal_location_table(const Mech *mech, int row, int column) {
  if (row < 0 || column < 0)
    abort();
  const void *table =
      mech_is_quad(mech) ? quad_location_table : mech_location_table;
  return table_value(table, 8, 2, (size_t)row, (size_t)column);
}

static char internal_structure_value(size_t row, size_t column) {
  return table_value(internal_structure,
                     sizeof(internal_structure) / sizeof(*internal_structure),
                     5, row, column);
}

static int expected_internal_structure(Mech *mech, int location,
                                       int tonnage_index) {
  if (location == HEAD)
    return 3;
  int row = 0;
  while (internal_location_table(mech, row, 0) >= 0 &&
         location != internal_location_table(mech, row, 0))
    row++;
  if (internal_location_table(mech, row, 0) < 0)
    return 0;
  return internal_structure_value(
      (size_t)tonnage_index, (size_t)internal_location_table(mech, row, 1));
}

void vehicle_int_check(Mech *mech, int noisy) {
  const int rounded_tonnage = mech->ud.tons + 5;
  const int expected = (rounded_tonnage > 10 ? rounded_tonnage : 10) / 10;
  for (int location = 0; location < NUM_SECTIONS; location++) {
    if (!mech_section_original_internal(mech, location) ||
        mech_section_original_internal(mech, location) == expected)
      continue;
    if (noisy)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Template %s / mech #%ld: Invalid internals in loc %d "
                  "(should be %d, are %d)",
                  mech->ud.mech_type, mech->mynum, location, expected,
                  mech_section_original_internal(mech, location)));
    mech_section_original_internal_set(mech, location, expected);
    mech_section_internal_set(mech, location, expected);
  }
}

void mech_int_check(Mech *mech, int noisy) {
  if (mech->ud.type != CLASS_MECH) {
    if (mech->ud.type == CLASS_VEH_GROUND || mech->ud.type == CLASS_VTOL ||
        mech->ud.type == CLASS_VEH_NAVAL)
      vehicle_int_check(mech, noisy);
    return;
  }

  int tonnage_index = 0;
  while (internal_structure_value((size_t)tonnage_index, 0) >= 0 &&
         mech->ud.tons != internal_structure_value((size_t)tonnage_index, 0))
    tonnage_index++;
  if (internal_structure_value((size_t)tonnage_index, 0) < 0) {
    if (noisy)
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
                         tprintf("VERY odd tonnage for #%ld: %d.", mech->mynum,
                                 mech->ud.tons));
    return;
  }

  for (int location = 0; location < NUM_SECTIONS; location++) {
    const int expected =
        expected_internal_structure(mech, location, tonnage_index);
    if (mech_section_original_internal(mech, location) == expected)
      continue;
    if (noisy)
      btech_channel_send(
          mech->xcode.context, BTECH_CHANNEL_MECH_ERRORS, "%s",
          tprintf("Template %s / mech #%ld: Invalid internals in loc %d "
                  "(should be %d, are %d)",
                  mech->ud.mech_type, mech->mynum, location, expected,
                  mech_section_original_internal(mech, location)));
    mech_section_original_internal_set(mech, location, expected);
    mech_section_internal_set(mech, location, expected);
  }
}
