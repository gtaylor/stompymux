#include <assert.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_c3_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_network_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "template_api.h"
#include "weapon_catalogue_api.h"

static int discovered_total_masters;
static int discovered_working_masters;
static bool c3_master_present;

int mech_c3_total_master_count(Mech *mech [[maybe_unused]]) {
  return discovered_total_masters;
}

int mech_c3_working_master_count(Mech *mech [[maybe_unused]]) {
  return discovered_working_masters;
}

int mech_c3_total_masters(const Mech *mech) {
  return mech->sd.w_total_c3_masters;
}

void mech_c3_total_masters_set(Mech *mech, int count) {
  mech->sd.w_total_c3_masters = count;
}

int mech_c3_working_masters(const Mech *mech) {
  return mech->sd.w_working_c3_masters;
}

void mech_c3_working_masters_set(Mech *mech, int count) {
  mech->sd.w_working_c3_masters = count;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters)
int crits_in_loc(Mech *mech [[maybe_unused]], int index) {
  return c3_master_present && index == HEAD ? 1 : 0;
}

int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return special_equipment_index(C3_MASTER);
}

bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int section [[maybe_unused]],
                                    int critical [[maybe_unused]]) {
  return false;
}

void mech_section_configuration_remove(Mech *mech [[maybe_unused]],
                                       int section [[maybe_unused]],
                                       int configuration [[maybe_unused]]) {}

void mech_section_configuration_add(Mech *mech [[maybe_unused]],
                                    int section [[maybe_unused]],
                                    int configuration [[maybe_unused]]) {}

bool weapon_catalogue_is_anti_missile(int weapon_index [[maybe_unused]]) {
  return false;
}

bool weapon_catalogue_has_special(int weapon_index [[maybe_unused]],
                                  int special [[maybe_unused]]) {
  return false;
}

bool equipment_can_use_targeting_computer(int equipment_index
                                          [[maybe_unused]]) {
  return false;
}

void mech_critical_fire_mode_add(Mech *mech [[maybe_unused]],
                                 int section [[maybe_unused]],
                                 int critical [[maybe_unused]],
                                 int modes [[maybe_unused]]) {}

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format [[maybe_unused]], ...) {}

int max(int v1, int v2) { return v1 > v2 ? v1 : v2; }

int bsuit_member_count(const Mech *mech [[maybe_unused]]) { return 0; }
// NOLINTEND(bugprone-easily-swappable-parameters)

static void test_c3_master_present_and_functional(void) {
  Mech mech;

  memset(&mech, 0, sizeof(mech));
  c3_master_present = true;
  discovered_total_masters = 2;
  discovered_working_masters = 1;
  mech_crit_status_set(&mech.rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED);
  update_specials(&mech);

  assert(mech_c3_total_masters(&mech) == 2);
  assert(mech_c3_working_masters(&mech) == 1);
  assert((mech.rd.specials & C3_MASTER_TECH) != 0);
  assert(
      !mech_crit_status_has(mech.rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED));
}

static void test_c3_master_present_and_broken(void) {
  Mech mech;

  memset(&mech, 0, sizeof(mech));
  c3_master_present = true;
  discovered_total_masters = 2;
  discovered_working_masters = 0;
  update_specials(&mech);

  assert(mech_c3_total_masters(&mech) == 2);
  assert(mech_c3_working_masters(&mech) == 0);
  assert((mech.rd.specials & C3_MASTER_TECH) != 0);
  assert(
      mech_crit_status_has(mech.rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED));
}

static void test_c3_master_removed_clears_cached_state(void) {
  Mech mech;

  memset(&mech, 0, sizeof(mech));
  c3_master_present = false;
  mech_c3_total_masters_set(&mech, 2);
  mech_c3_working_masters_set(&mech, 0);
  mech.rd.specials |= C3_MASTER_TECH;
  mech_crit_status_set(&mech.rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED);
  update_specials(&mech);

  assert(mech_c3_total_masters(&mech) == 0);
  assert(mech_c3_working_masters(&mech) == 0);
  assert((mech.rd.specials & C3_MASTER_TECH) == 0);
  assert(
      !mech_crit_status_has(mech.rd.critstatus, MECH_CRIT_STATUS_C3_DESTROYED));
}

int main(void) {
  test_c3_master_present_and_functional();
  test_c3_master_present_and_broken();
  test_c3_master_removed_clears_cached_state();
  return 0;
}
