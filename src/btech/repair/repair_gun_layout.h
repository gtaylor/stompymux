/* Shared validation and coordinates for weapon critical footprints. */

#pragma once

#include "mech_api_types.h"
#include "mech_utils_api.h"

typedef struct RepairGunLayout {
  int size;
  int local_count;
  SplitCriticalLookup split;
} RepairGunLayout;

typedef enum RepairGunLayoutRequirements : unsigned int {
  REPAIR_GUN_LAYOUT_NONE = 0,
  REPAIR_GUN_LAYOUT_REQUIRE_WEAPON = 1U << 0,
  REPAIR_GUN_LAYOUT_REQUIRE_GUN_START = 1U << 1,
  REPAIR_GUN_LAYOUT_REQUIRE_CONTIGUOUS = 1U << 2,
  REPAIR_GUN_LAYOUT_REQUIRE_INTACT_SECTIONS = 1U << 3,
} RepairGunLayoutRequirements;

bool repair_gun_layout_find(Mech *mech, int location, int position,
                            unsigned int requirements, RepairGunLayout *layout);
