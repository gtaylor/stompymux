#include "mech_charge_tracking_api.h"

#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech/context.h"
#include "equipment_types.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_targeting_api.h"
#include "registry_api.h"

struct Mech {
  DbRef charge_target;
  int charge_timer;
  float charge_distance;
};

static BtechContext *context = (BtechContext *)(uintptr_t)1;
static bool new_charge_rules;
static bool target_exists;
static float target_range;
static int charge_count;
static char notification[64];

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return context;
}

bool btech_context_uses_new_charge_rules(const BtechContext *value
                                         [[maybe_unused]]) {
  return new_charge_rules;
}

DbRef mech_charge_target_dbref(const Mech *mech) { return mech->charge_target; }

int mech_charge_timer_advance(Mech *mech) { return mech->charge_timer++; }

void mech_charge_distance_add(Mech *mech, float distance) {
  mech->charge_distance += distance;
}

void mech_charge_reset(Mech *mech) {
  mech->charge_target = -1;
  mech->charge_timer = 0;
  mech->charge_distance = 0.0F;
}

void mech_notify(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *message) {
  (void)snprintf(notification, sizeof(notification), "%s", message);
}

Mech *btech_context_get_mech(BtechContext *value [[maybe_unused]],
                             DbRef dbref) {
  return target_exists && dbref == 0 ? (Mech *)(uintptr_t)2 : nullptr;
}

float mech_range_to(const Mech *mech [[maybe_unused]],
                    const Mech *target [[maybe_unused]]) {
  return target_range;
}

void charge_mech(Mech *mech [[maybe_unused]], Mech *target [[maybe_unused]]) {
  charge_count++;
}

static Mech fresh_mech(void) { return (Mech){.charge_target = -1}; }

static void reset_fixture(void) {
  new_charge_rules = true;
  target_exists = true;
  target_range = 1.0F;
  charge_count = 0;
  notification[0] = '\0';
}

static void test_distance_tracking(void) {
  Mech mech = fresh_mech();
  reset_fixture();
  mech.charge_target = 0;

  mech_charge_distance_record(&mech, (float)SCALEMAP, 0.0F);
  assert(fabsf(mech.charge_distance - 1.0F) < 0.0001F);

  mech_charge_distance_record(&mech, 0.0F, 1.0F);
  assert(mech.charge_distance > 1.0F);

  const float BEFORE_INVALID = mech.charge_distance;
  mech_charge_distance_record(&mech, INFINITY, 0.0F);
  assert(fabsf(mech.charge_distance - BEFORE_INVALID) < 0.000001F);
  mech_charge_distance_record(&mech, NAN, 0.0F);
  assert(fabsf(mech.charge_distance - BEFORE_INVALID) < 0.000001F);

  new_charge_rules = false;
  mech_charge_distance_record(&mech, (float)SCALEMAP, 0.0F);
  assert(fabsf(mech.charge_distance - BEFORE_INVALID) < 0.000001F);
}

static void test_timeout_boundary(void) {
  Mech mech = fresh_mech();
  reset_fixture();
  mech.charge_target = 0;
  mech.charge_timer = 59;
  mech_charge_timeout_update(&mech);
  assert(mech.charge_timer == 60);
  assert(mech.charge_target == 0);

  mech_charge_timeout_update(&mech);
  assert(mech.charge_timer == 0);
  assert(mech.charge_target == -1);
  assert(strcmp(notification, "Charge timed out, charge reset.") == 0);
}

static void test_impact_resolution(void) {
  Mech mech = fresh_mech();
  reset_fixture();

  mech.charge_target = -1;
  mech_charge_impact_resolve(&mech);
  assert(charge_count == 0);

  mech.charge_target = 0;
  target_exists = false;
  mech_charge_impact_resolve(&mech);
  assert(charge_count == 0);
  assert(mech.charge_target == -1);
  assert(strcmp(notification, "Invalid CHARGE target!") == 0);

  target_exists = true;
  mech.charge_target = 0;
  target_range = (float)CHARGE_DIST_TRIGGER;
  mech_charge_impact_resolve(&mech);
  assert(charge_count == 0);
  assert(mech.charge_target == 0);

  target_range = 0.5F;
  mech_charge_impact_resolve(&mech);
  assert(charge_count == 1);
  assert(mech.charge_target == -1);
}

int main(void) {
  test_distance_tracking();
  test_timeout_boundary();
  test_impact_resolution();
  return 0;
}
