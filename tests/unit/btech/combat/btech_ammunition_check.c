#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "equipment_types.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_utils_api.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static Mech mech;
static int weapon_type;
static long weapon_specials;
static bool rac;
static bool artillery;
static int fire_mode;
static int ammunition_mode;
static int ammunition_count;
static int primary_rounds;
static int secondary_rounds;
static int mode_clear_count;
static int last_cleared_mode;
static int notification_count;
static CriticalSlotLookupResult first_lookup;
static CriticalSlotLookupResult second_lookup;
static int lookup_count;
static int lookup_index;
static AmmunitionLookupRequest last_lookup;

int weapon_catalogue_type(int weapon_index [[maybe_unused]]) {
  return weapon_type;
}

long weapon_catalogue_specials(int weapon_index [[maybe_unused]]) {
  return weapon_specials;
}

bool weapon_catalogue_has_special(int weapon_index [[maybe_unused]],
                                  int special) {
  return special == RAC && rac;
}

bool weapon_catalogue_is_artillery(int weapon_index [[maybe_unused]]) {
  return artillery;
}

int mech_critical_fire_mode(const Mech *value [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return fire_mode;
}

int mech_critical_ammo_mode(const Mech *value [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return ammunition_mode;
}

void mech_critical_fire_mode_clear(Mech *value [[maybe_unused]],
                                   int section [[maybe_unused]],
                                   int critical [[maybe_unused]], int mode) {
  ++mode_clear_count;
  last_cleared_mode = mode;
  fire_mode &= ~mode;
}

int mech_critical_data(const Mech *value [[maybe_unused]], int section,
                       int critical) {
  if (section == 3 && critical == 4)
    return primary_rounds;
  if (section == 5 && critical == 6)
    return secondary_rounds;
  return 0;
}

void mech_critical_data_set(Mech *value [[maybe_unused]], int section,
                            int critical, int data) {
  if (section == 3 && critical == 4)
    primary_rounds = data;
  else if (section == 5 && critical == 6)
    secondary_rounds = data;
}

int count_ammo_for_weapon(Mech *value [[maybe_unused]],
                          int weapon_index [[maybe_unused]]) {
  return ammunition_count;
}

CriticalSlotLookupResult
ammunition_find(const AmmunitionLookupRequest *request) {
  last_lookup = *request;
  if (lookup_index >= lookup_count)
    return (CriticalSlotLookupResult){0};
  ++lookup_index;
  return lookup_index == 1 ? first_lookup : second_lookup;
}

DbRef mech_gunner_dbref(const Mech *value [[maybe_unused]]) { return 42; }

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return (EvaluationContext *)context;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {
  ++notification_count;
}

static void reset_fixture(void) {
  memset(&mech, 0, sizeof(mech));
  mech.xcode.context = (BtechContext *)&mech;
  weapon_type = TAMMO;
  weapon_specials = 0;
  rac = false;
  artillery = false;
  fire_mode = 0;
  ammunition_mode = 0;
  ammunition_count = 20;
  primary_rounds = 0;
  secondary_rounds = 0;
  mode_clear_count = 0;
  last_cleared_mode = 0;
  notification_count = 0;
  first_lookup = (CriticalSlotLookupResult){0};
  second_lookup = (CriticalSlotLookupResult){0};
  lookup_count = 0;
  lookup_index = 0;
  last_lookup = (AmmunitionLookupRequest){0};
}

static AmmunitionCheckResult check(void) {
  return ammunition_check(&(AmmunitionCheckRequest){
      .mech = &mech,
      .weapon_index = 7,
      .weapon = {.section = 1, .critical = 2},
      .gatling_shots = 4,
  });
}

static void provide_primary(int rounds) {
  first_lookup = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 3, .critical = 4}};
  lookup_count = 1;
  primary_rounds = rounds;
}

static int test_energy_and_single_use_weapons(void) {
  reset_fixture();
  weapon_type = TBEAM;
  if (!check().available || lookup_count != 0)
    return 1;

  reset_fixture();
  weapon_type = THAND;
  if (!check().available)
    return 1;

  reset_fixture();
  weapon_specials = ROCKET;
  if (!check().available)
    return 1;
  fire_mode = ROCKET_FIRED;
  if (check().available || notification_count != 1)
    return 1;

  reset_fixture();
  fire_mode = OS_MODE;
  if (!check().available)
    return 1;
  fire_mode = OS_MODE | OS_USED;
  return check().available || notification_count != 1;
}

static int test_missing_empty_and_normal_ammunition(void) {
  reset_fixture();
  AmmunitionCheckResult result = check();
  if (result.available || result.primary.found || notification_count != 1)
    return 1;

  reset_fixture();
  provide_primary(0);
  result = check();
  if (result.available || !result.primary.found || notification_count != 1)
    return 1;

  reset_fixture();
  provide_primary(3);
  result = check();
  return !result.available || !result.primary.found ||
         result.primary.slot.section != 3 ||
         last_lookup.forbidden_modes != AMMO_MODES || notification_count != 0;
}

static int test_specialized_ammunition(void) {
  reset_fixture();
  ammunition_mode = AC_PRECISION_MODE;
  provide_primary(2);
  AmmunitionCheckResult result = check();
  if (!result.available || last_lookup.required_modes != AC_PRECISION_MODE ||
      (last_lookup.forbidden_modes & AC_PRECISION_MODE) != 0)
    return 1;

  reset_fixture();
  artillery = true;
  ammunition_mode = CLUSTER_MODE;
  provide_primary(2);
  result = check();
  return !result.available || last_lookup.required_modes != CLUSTER_MODE ||
         (last_lookup.forbidden_modes & CLUSTER_MODE) != 0;
}

static int test_ultra_and_rfac_secondary_rounds(void) {
  reset_fixture();
  fire_mode = ULTRA_MODE;
  provide_primary(2);
  second_lookup = first_lookup;
  lookup_count = 2;
  AmmunitionCheckResult result = check();
  if (!result.available || !result.secondary.found || primary_rounds != 2 ||
      mode_clear_count != 0)
    return 1;

  reset_fixture();
  fire_mode = RFAC_MODE;
  provide_primary(1);
  result = check();
  return !result.available || result.secondary.found || primary_rounds != 1 ||
         mode_clear_count != 1 || last_cleared_mode != RFAC_MODE;
}

static int test_rac_and_gatling_limits(void) {
  reset_fixture();
  rac = true;
  fire_mode = RAC_SIXSHOT_MODE;
  ammunition_count = 4;
  AmmunitionCheckResult result = check();
  if (!result.available || mode_clear_count != 1 ||
      last_cleared_mode != RAC_SIXSHOT_MODE)
    return 1;

  reset_fixture();
  fire_mode = GATTLING_MODE;
  ammunition_count = 5;
  provide_primary(5);
  result = check();
  return !result.available || result.gatling_shots != 1;
}

int main(void) {
  int failures = 0;
  if (test_energy_and_single_use_weapons()) {
    fprintf(stderr, "energy and single-use weapon cases failed\n");
    ++failures;
  }
  if (test_missing_empty_and_normal_ammunition()) {
    fprintf(stderr, "missing, empty, and normal ammunition cases failed\n");
    ++failures;
  }
  if (test_specialized_ammunition()) {
    fprintf(stderr, "specialized ammunition cases failed\n");
    ++failures;
  }
  if (test_ultra_and_rfac_secondary_rounds()) {
    fprintf(stderr, "Ultra and RFAC secondary-round cases failed\n");
    ++failures;
  }
  if (test_rac_and_gatling_limits()) {
    fprintf(stderr, "RAC and Gatling limit cases failed\n");
    ++failures;
  }
  return failures != 0;
}
