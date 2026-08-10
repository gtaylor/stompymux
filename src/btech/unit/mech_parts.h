/* Parts inventory operations used by BTech repairs. */

#pragma once

#include <stddef.h>

#include "mux/server/platform.h"

// IWYU pragma: no_include "mech.h"

typedef struct Mech Mech;

constexpr int MECH_PART_LOCATION_UNUSED = -1;

typedef struct MechPartRequirement {
  int part;
  int brand;
  int count;
} MechPartRequirement;

typedef struct MechPartLocation {
  Mech *mech;
  int section;
  int part;
} MechPartLocation;

int mech_parts_alias(const MechPartLocation *location);
DbRef mech_parts_store_dbref(const Mech *mech);
bool mech_parts_available(Mech *mech, int part, int brand, int count);
void mech_parts_take(Mech *mech, int part, int brand, int count);
void mech_parts_add(Mech *mech, int location, int part, int brand, int count);
bool mech_parts_consume(Mech *mech, DbRef player,
                        const MechPartRequirement requirements[], size_t count);
bool mech_section_armor_repairing(Mech *mech, int section);
bool mech_section_rear_armor_repairing(Mech *mech, int section);
bool mech_section_internals_repairing(Mech *mech, int section);
