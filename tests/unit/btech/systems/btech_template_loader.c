#include "btech/context.h"
#include "btech_event.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_targeting_api.h"
#include "mech_template_api.h"
#include "template_api.h"

#include <assert.h>
#include <string.h>

static BtechContext *test_context;
static int communications_clear_count;
static int template_parse_count;

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}

const char *btech_context_mech_template_path(const BtechContext *context
                                             [[maybe_unused]]) {
  assert(context == test_context);
  return "test-templates";
}

char *mech_template_resolve_path(BtechContext *context [[maybe_unused]],
                                 const char *mech_path [[maybe_unused]],
                                 const char *id) {
  static char modern_path[] = "test-templates/modern";
  static char legacy_path[] = "test-templates/legacy";

  assert(context == test_context);
  assert(strcmp(mech_path, "test-templates") == 0);
  if (strcmp(id, "modern") == 0)
    return modern_path;
  if (strcmp(id, "legacy") == 0)
    return legacy_path;
  return nullptr;
}

int load_template(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                  char *filename) {
  template_parse_count++;
  return strcmp(filename, "test-templates/modern") == 0 ? 0 : -1;
}

void mech_template_state_reset(Mech *mech) {
  memset(&mech->rd, 0, sizeof(mech->rd));
  memset(&mech->ud, 0, sizeof(mech->ud));
}

void mech_communications_clear(Mech *mech [[maybe_unused]]) {
  communications_clear_count++;
}

void mech_spotter_dbref_set(Mech *mech [[maybe_unused]],
                            DbRef spotter [[maybe_unused]]) {}

void mech_targeting_target_clear(Mech *mech) { (void)mech; }

void mech_charge_reset(Mech *mech) { (void)mech; }

void mech_dfa_target_dbref_set(Mech *mech [[maybe_unused]],
                               DbRef target [[maybe_unused]]) {}

void mech_pilot_dbref_set(Mech *mech [[maybe_unused]],
                          DbRef pilot [[maybe_unused]]) {}

void mech_targeting_aim_reset(Mech *mech) { (void)mech; }

void mech_event_cancel(Mech *mech [[maybe_unused]],
                       MechEventType type [[maybe_unused]]) {
  assert(type == EVENT_VEHICLEBURN);
}

static void reset_observations(void) {
  communications_clear_count = 0;
  template_parse_count = 0;
}

int main(void) {
  Mech mech = {};

  test_context = mech.xcode.context;

  strcpy(mech.ud.mech_type, "modern");
  reset_observations();
  assert(mech_template_load(1, &mech, "modern") == 1);
  assert(template_parse_count == 1);
  assert(communications_clear_count == 0);

  strcpy(mech.ud.mech_type, "modern");
  reset_observations();
  assert(mech_template_load(1, &mech, "legacy") == 0);
  assert(template_parse_count == 1);
  assert(communications_clear_count == 1);

  strcpy(mech.ud.mech_type, "modern");
  reset_observations();
  assert(mech_template_load(1, &mech, "missing") == 0);
  assert(template_parse_count == 0);
  assert(communications_clear_count == 1);
  return 0;
}
