#pragma once
/* Declares internal physical-attack support. */

#include <ctype.h>
#include <math.h>
#include <stdio.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_terrain.h"
#include "mech_bth_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_hitloc_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical.h"
#include "mech_physical_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

int have_punch(Mech *mech, int location);
bool physical_arm_check(DbRef player, Mech *mech, const char *verb);
bool physical_quad_check(DbRef player, Mech *mech, const char *verb);
bool phys_common_checks(Mech *mech);
// Implementations retain this callback's int contract and must be exempted by
// name in check-boolean-contracts when added.
typedef int (*PhysicalEquipmentCheck)(Mech *mech, int location);

typedef struct ArmSelectionRequest {
  int using;
  int argument_count;
  char **arguments;
  Mech *mech;
  PhysicalEquipmentCheck has_weapon;
  const char *weapon;
} ArmSelectionRequest;

typedef struct ArmSelectionResult {
  bool failed;
  int using;
  int argument_count;
  char **arguments;
} ArmSelectionResult;

ArmSelectionResult physical_arm_select(const ArmSelectionRequest *request);

void physical_damage_apply(Mech *target, Mech *attacker, int cause_pilot,
                           DbRef pilot, int hit_location, int rear,
                           int critical, int damage, int glancing);
void physical_damage_apply_without_experience(Mech *target, Mech *attacker,
                                              int cause_pilot, DbRef pilot,
                                              int hit_location, int rear,
                                              int critical, int damage,
                                              int glancing);

enum { CHARGE_SECTIONS = 6 };

int physical_charge_section(int index);

/**
 * Checks to see if all limbs have recycled from any previous physical attacks.
 */
