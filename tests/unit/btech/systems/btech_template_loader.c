#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "context_internal.h"
#include "mech_crew_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_targeting_api.h"
#include "mech_template_api.h"
#include "template_api.h"

#undef NDEBUG
#include <assert.h>
#include <string.h>

static GameDatabase *const test_database = (GameDatabase *)1;
static BtechContext test_context_data = {.database = (GameDatabase *)1};
static BtechContext *const test_context = &test_context_data;
static int communications_clear_count;
static int template_parse_count;
static int attribute_write_count;
static char stored_template[64];

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}

const char *btech_context_mech_template_path(const BtechContext *context
                                             [[maybe_unused]]) {
  assert(context == test_context);
  return "test-templates";
}

GameDatabase *btech_context_database(BtechContext *context [[maybe_unused]]) {
  assert(context == test_context);
  return context->database;
}

void silly_atr_set_in(GameDatabase *database, DbRef object, int attribute,
                      const char *value) {
  assert(database == test_database);
  assert(object == 77);
  assert(attribute == A_MECHTYPE);
  attribute_write_count++;
  strcpy(stored_template, value);
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

int load_template(DbRef player [[maybe_unused]], Mech *mech, char *filename) {
  template_parse_count++;
  if (strcmp(filename, "test-templates/legacy") == 0) {
    mech->ud.tons = 999;
    mech->rd.speed = 99.0;
    mech->pd.x = 77;
    mech->tic[0][0] = 123;
  }
  if (strcmp(filename, "test-templates/modern") == 0)
    strcpy(mech->ud.mech_type, "modern");
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
  attribute_write_count = 0;
  strcpy(stored_template, "previous-template");
}

int main(void) {
  Mech mech = {};
  Mech expected;

  mech.xcode.context = test_context;
  mech.mynum = 77;

  strcpy(mech.ud.mech_type, "modern");
  reset_observations();
  assert(mech_template_load(1, &mech, "modern"));
  assert(template_parse_count == 1);
  assert(communications_clear_count == 0);
  assert(attribute_write_count == 1);
  assert(strcmp(stored_template, "modern") == 0);

  strcpy(mech.ud.mech_type, "modern");
  mech.ud.tons = 55;
  mech.rd.speed = 4.0;
  mech.pd.x = 9;
  mech.tic[0][0] = 42;
  expected = mech;
  reset_observations();
  assert(!mech_template_load(1, &mech, "legacy"));
  assert(template_parse_count == 1);
  assert(communications_clear_count == 1);
  assert(memcmp(&mech, &expected, sizeof(mech)) == 0);
  assert(attribute_write_count == 0);
  assert(strcmp(stored_template, "previous-template") == 0);

  strcpy(mech.ud.mech_type, "modern");
  expected = mech;
  reset_observations();
  assert(!mech_template_load(1, &mech, "missing"));
  assert(template_parse_count == 0);
  assert(communications_clear_count == 0);
  assert(memcmp(&mech, &expected, sizeof(mech)) == 0);
  assert(attribute_write_count == 0);
  assert(strcmp(stored_template, "previous-template") == 0);
  return 0;
}
