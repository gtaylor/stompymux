#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "failures.h"
#include "failures_api.h"
#include "mech_combat_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "weapon_catalogue_api.h"

static PartFailureResult next_failure;
static int decrement_count;
static AmmunitionDecrementRequest last_decrement;
static bool underwater;
static bool extended_ranges;
static int normal_range = 12;
static int water_range = 6;
static int notification_count;
static BtechContext context;

PartFailureResult
mech_weapon_failure_check(const MechWeaponFailureRequest *request) {
  (void)request;
  return next_failure;
}

void mech_ammunition_decrement(const AmmunitionDecrementRequest *request) {
  ++decrement_count;
  last_decrement = *request;
}

bool mech_section_is_underwater(const Mech *mech, int section) {
  (void)mech;
  (void)section;
  return underwater;
}

BtechContext *mech_context(const Mech *mech) {
  (void)mech;
  return &context;
}

bool btech_context_uses_extended_weapon_ranges(const BtechContext *value) {
  (void)value;
  return extended_ranges;
}

int weapon_catalogue_effective_range(int weapon_index, bool extended) {
  (void)weapon_index;
  return normal_range + (extended ? 3 : 0);
}

int weapon_catalogue_effective_water_range(int weapon_index, bool extended) {
  (void)weapon_index;
  return water_range + (extended ? 2 : 0);
}

void mech_notify(Mech *mech, MechNotifyAudience audience, const char *buffer) {
  (void)mech;
  (void)audience;
  if (strstr(buffer, "falls short") != nullptr)
    ++notification_count;
}

static void reset_fixture(void) {
  next_failure = (PartFailureResult){0};
  decrement_count = 0;
  last_decrement = (AmmunitionDecrementRequest){0};
  underwater = false;
  extended_ranges = false;
  normal_range = 12;
  water_range = 6;
  notification_count = 0;
}

static WeaponFailureResolution invoke(float range) {
  return weapon_failure_resolve(&(WeaponFailureResolutionRequest){
      .mech = (Mech *)&context,
      .weapon_number = 2,
      .weapon_index = 7,
      .weapon = {.section = 3, .critical = 4},
      .primary_ammunition = {.found = true,
                             .slot = {.section = 5, .critical = 6}},
      .secondary_ammunition = {.found = true,
                               .slot = {.section = 7, .critical = 8}},
      .range = range,
      .gatling_shots = 3,
  });
}

static int test_power_spike(void) {
  reset_fixture();
  next_failure = (PartFailureResult){.type = POWER_SPIKE, .modifier = 2};
  WeaponFailureResolution result = invoke(4.0F);
  return !result.handled || !result.range_ok || result.modifier != 2 ||
         decrement_count != 0;
}

static int test_ammunition_failures(void) {
  for (int type = WEAPON_JAMMED; type <= WEAPON_DUD; ++type) {
    reset_fixture();
    next_failure = (PartFailureResult){.type = type};
    WeaponFailureResolution result = invoke(4.0F);
    if (!result.handled || decrement_count != 1 ||
        !last_decrement.primary_ammunition.found ||
        last_decrement.primary_ammunition.slot.section != 5 ||
        last_decrement.secondary_ammunition.slot.critical != 8)
      return 1;
  }
  return 0;
}

static int test_range_failure(void) {
  reset_fixture();
  next_failure = (PartFailureResult){.type = RANGE, .modifier = 3};
  WeaponFailureResolution result = invoke(10.0F);
  if (result.handled || result.range_ok || notification_count != 1)
    return 1;

  reset_fixture();
  next_failure = (PartFailureResult){.type = RANGE, .modifier = 3};
  result = invoke(9.0F);
  if (!result.range_ok || notification_count != 0)
    return 1;

  reset_fixture();
  underwater = true;
  extended_ranges = true;
  next_failure = (PartFailureResult){.type = RANGE, .modifier = 1};
  result = invoke(7.0F);
  return !result.range_ok || notification_count != 0;
}

int main(void) {
  int failures = 0;
  if (test_power_spike()) {
    fprintf(stderr, "power-spike failure case failed\n");
    ++failures;
  }
  if (test_ammunition_failures()) {
    fprintf(stderr, "ammunition failure cases failed\n");
    ++failures;
  }
  if (test_range_failure()) {
    fprintf(stderr, "range failure cases failed\n");
    ++failures;
  }
  return failures != 0;
}
