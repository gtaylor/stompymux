#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "equipment_types.h"
#include "mech_combat_misc_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_utils_api.h"
#include "weapon_catalogue_api.h"

static Mech mech;
static bool energy;
static bool hand_to_hand;
static bool rocket;
static bool rotary;
static int fire_mode;
static int rounds_2_3;
static int rounds_3_4;
static int rounds_4_5;
static int rounds_5_6;
static int data_reads;
static int data_writes;
static int mode_adds;
static int last_added_mode;
static int expenditure_checks;
static int ammunition_count;
static CriticalSlotLookupResult first_lookup;
static CriticalSlotLookupResult second_lookup;
static int lookup_count;
static int lookup_index;

bool weapon_catalogue_is_energy(int weapon_index) {
  (void)weapon_index;
  return energy;
}

bool weapon_catalogue_is_hand_to_hand(int weapon_index) {
  (void)weapon_index;
  return hand_to_hand;
}

bool weapon_catalogue_is_only_rocket(int weapon_index) {
  (void)weapon_index;
  return rocket;
}

bool weapon_catalogue_is_rotary_autocannon(int weapon_index) {
  (void)weapon_index;
  return rotary;
}

int get_weapon_crits(Mech *value, int weapon_index) {
  (void)value;
  (void)weapon_index;
  return 3;
}

int mech_weapon_first_critical(const WeaponCriticalSearch *search) {
  (void)search;
  return 2;
}

int mech_critical_part_type(const Mech *value, int section, int critical) {
  (void)value;
  (void)section;
  (void)critical;
  return 17;
}

int mech_critical_fire_mode(const Mech *value, int section, int critical) {
  (void)value;
  (void)section;
  (void)critical;
  return fire_mode;
}

void mech_critical_fire_mode_add(Mech *value, int section, int critical,
                                 int mode) {
  (void)value;
  (void)section;
  (void)critical;
  ++mode_adds;
  last_added_mode = mode;
}

int mech_critical_data(const Mech *value, int section, int critical) {
  (void)value;
  ++data_reads;
  if (section == 2 && critical == 3)
    return rounds_2_3;
  if (section == 3 && critical == 4)
    return rounds_3_4;
  if (section == 4 && critical == 5)
    return rounds_4_5;
  if (section == 5 && critical == 6)
    return rounds_5_6;
  return 0;
}

void mech_critical_data_set(Mech *value, int section, int critical, int data) {
  (void)value;
  ++data_writes;
  if (section == 2 && critical == 3)
    rounds_2_3 = data;
  else if (section == 3 && critical == 4)
    rounds_3_4 = data;
  else if (section == 4 && critical == 5)
    rounds_4_5 = data;
  else if (section == 5 && critical == 6)
    rounds_5_6 = data;
}

void mech_ammunition_expenditure_check(
    const AmmunitionExpenditureCheck *check) {
  (void)check;
  ++expenditure_checks;
}

CriticalSlotLookupResult
ammunition_find(const AmmunitionLookupRequest *request) {
  (void)request;
  if (lookup_index >= lookup_count)
    return (CriticalSlotLookupResult){0};
  ++lookup_index;
  return lookup_index == 1 ? first_lookup : second_lookup;
}

int count_ammo_for_weapon(Mech *value, int weapon_index) {
  (void)value;
  (void)weapon_index;
  return ammunition_count;
}

static void reset_fixture(void) {
  memset(&mech, 0, sizeof(mech));
  energy = false;
  hand_to_hand = false;
  rocket = false;
  rotary = false;
  fire_mode = 0;
  rounds_2_3 = 0;
  rounds_3_4 = 0;
  rounds_4_5 = 0;
  rounds_5_6 = 0;
  data_reads = 0;
  data_writes = 0;
  mode_adds = 0;
  last_added_mode = 0;
  expenditure_checks = 0;
  ammunition_count = 100;
  first_lookup = (CriticalSlotLookupResult){0};
  second_lookup = (CriticalSlotLookupResult){0};
  lookup_count = 0;
  lookup_index = 0;
}

static AmmunitionDecrementRequest request(void) {
  return (AmmunitionDecrementRequest){
      .mech = &mech,
      .weapon_index = 9,
      .weapon = {.section = 1, .critical = 2},
      .gatling_shots = 2,
  };
}

static int test_absent_primary_never_accesses_storage(void) {
  reset_fixture();
  AmmunitionDecrementRequest decrement = request();
  mech_ammunition_decrement(&decrement);
  return data_reads != 0 || data_writes != 0 || expenditure_checks != 0;
}

static int test_normal_and_double_rate(void) {
  reset_fixture();
  AmmunitionDecrementRequest decrement = request();
  decrement.primary_ammunition = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 3, .critical = 4}};
  rounds_3_4 = 3;
  mech_ammunition_decrement(&decrement);
  if (rounds_3_4 != 2 || data_writes != 1 || expenditure_checks != 1)
    return 1;

  reset_fixture();
  decrement = request();
  decrement.primary_ammunition = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 3, .critical = 4}};
  decrement.secondary_ammunition = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 5, .critical = 6}};
  rounds_3_4 = 2;
  rounds_5_6 = 4;
  fire_mode = ULTRA_MODE;
  mech_ammunition_decrement(&decrement);
  if (rounds_3_4 != 1 || rounds_5_6 != 3 || data_writes != 2)
    return 1;

  reset_fixture();
  decrement = request();
  decrement.primary_ammunition = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 3, .critical = 4}};
  rounds_3_4 = 2;
  fire_mode = RFAC_MODE;
  mech_ammunition_decrement(&decrement);
  return rounds_3_4 != 1 || data_reads != 1 || data_writes != 1;
}

static int test_non_ammunition_weapons(void) {
  reset_fixture();
  AmmunitionDecrementRequest decrement = request();
  energy = true;
  mech_ammunition_decrement(&decrement);
  if (data_reads != 0 || mode_adds != 0)
    return 1;

  reset_fixture();
  decrement = request();
  rocket = true;
  mech_ammunition_decrement(&decrement);
  if (mode_adds != 3 || last_added_mode != ROCKET_FIRED)
    return 1;

  reset_fixture();
  decrement = request();
  fire_mode = OS_MODE;
  mech_ammunition_decrement(&decrement);
  return mode_adds != 1 || last_added_mode != OS_USED;
}

static int test_rotary_and_gatling_bins(void) {
  reset_fixture();
  AmmunitionDecrementRequest decrement = request();
  rotary = true;
  fire_mode = RAC_FOURSHOT_MODE;
  first_lookup = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 2, .critical = 3}};
  second_lookup = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 4, .critical = 5}};
  lookup_count = 2;
  rounds_2_3 = 2;
  rounds_4_5 = 5;
  mech_ammunition_decrement(&decrement);
  if (rounds_2_3 != 0 || rounds_4_5 != 3 || data_writes != 2)
    return 1;

  reset_fixture();
  decrement = request();
  fire_mode = GATTLING_MODE;
  first_lookup = (CriticalSlotLookupResult){
      .found = true, .slot = {.section = 2, .critical = 3}};
  lookup_count = 1;
  rounds_2_3 = 10;
  mech_ammunition_decrement(&decrement);
  return rounds_2_3 != 4 || data_writes != 1;
}

int main(void) {
  int failures = 0;
  if (test_absent_primary_never_accesses_storage()) {
    fprintf(stderr, "absent primary ammunition case failed\n");
    ++failures;
  }
  if (test_normal_and_double_rate()) {
    fprintf(stderr, "normal and double-rate ammunition cases failed\n");
    ++failures;
  }
  if (test_non_ammunition_weapons()) {
    fprintf(stderr, "non-ammunition weapon cases failed\n");
    ++failures;
  }
  if (test_rotary_and_gatling_bins()) {
    fprintf(stderr, "rotary and Gatling ammunition cases failed\n");
    ++failures;
  }
  return failures != 0;
}
