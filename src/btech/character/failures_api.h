/* Declares the BattleTech failures API. */

#pragma once

#include "mux/server/platform.h"

#include "failures.h"
#include "mech_api_types.h"

typedef enum FailureSystem {
  FAILURE_SYSTEM_COMPUTER,
  FAILURE_SYSTEM_RADIO,
} FailureSystem;

typedef struct MechWeaponFailureRequest {
  Mech *mech;
  int weapon_number;
  int weapon_type;
  int section;
  int critical;
} MechWeaponFailureRequest;

typedef struct PartBrandRequest {
  int equipment_type;
  int quality_level;
} PartBrandRequest;

const char *mech_part_brand_name(const PartBrandRequest *request);
PartFailureResult mech_generic_failure_check(Mech *mech, FailureSystem system);
PartFailureResult
mech_weapon_failure_check(const MechWeaponFailureRequest *request);
