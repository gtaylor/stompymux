/* Parts inventory operations used by BTech repairs. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"

// IWYU pragma: no_include "mech.h"

typedef struct mech_data MECH;

constexpr int MECH_PART_LOCATION_UNUSED = -1;

typedef struct MechPartRequirement {
  int part;
  int brand;
  int count;
} MechPartRequirement;

int mech_parts_alias(MECH *mech, int location, int part);
bool mech_parts_available(MECH *mech, int part, int brand, int count);
void mech_parts_take(MECH *mech, int part, int brand, int count);
void mech_parts_add(MECH *mech, int location, int part, int brand, int count);
bool mech_parts_consume(MECH *mech, DbRef player,
                        const MechPartRequirement requirements[], size_t count);
bool mech_section_armor_repairing(MECH *mech, int section);
bool mech_section_rear_armor_repairing(MECH *mech, int section);
bool mech_section_internals_repairing(MECH *mech, int section);
