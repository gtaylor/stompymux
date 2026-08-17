#include "mechrep_api.h"

#undef NDEBUG
#include <assert.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "mech_identity_api.h"
#include "mech_restrict_api.h"
#include "mech_template_api.h"
#include "mechrep.h"
#include "registry_api.h"
#include "repair_job.h"

static BtechContext *const context = (BtechContext *)1;
static EvaluationContext *const evaluation = (EvaluationContext *)2;
static GameDatabase *const database = (GameDatabase *)3;
static RepairFacility facility = {.xcode = {.context = (BtechContext *)1}};
static Mech *const mech = (Mech *)4;
static bool template_load_result;
static int events_cancelled;
static int los_cleared;
static char loaded_template[32];

static void reset_state(void) {
  template_load_result = true;
  events_cancelled = 0;
  los_cleared = 0;
  memset(loaded_template, 0, sizeof(loaded_template));
}

RepairCommandStatus repair_facility_command_context_initialize(
    DbRef player, void *data, bool require_target [[maybe_unused]],
    RepairFacilityCommandContext *command) {
  assert(data == &facility);
  *command = (RepairFacilityCommandContext){.player = player,
                                            .facility = &facility,
                                            .context = context,
                                            .evaluation = evaluation,
                                            .mech = mech};
  return REPAIR_COMMAND_READY;
}

const char *repair_command_status_message(RepairCommandStatus status
                                          [[maybe_unused]]) {
  return "Unavailable.";
}

EvaluationContext *btech_context_evaluation(BtechContext *value) {
  return value == context ? evaluation : nullptr;
}

GameDatabase *btech_context_database(BtechContext *value) {
  return value == context ? database : nullptr;
}

BtechContext *mech_context(const Mech *value [[maybe_unused]]) {
  return context;
}

DbRef mech_dbref(const Mech *value [[maybe_unused]]) { return 44; }

int mech_parseattributes(char *buffer, char **arguments, int maxargs) {
  assert(maxargs == 1);
  if (buffer == nullptr || !*buffer)
    return 0;
  arguments[0] = buffer;
  return 1;
}

bool mech_template_load(DbRef player [[maybe_unused]], Mech *value,
                        const char *id) {
  assert(value == mech);
  strcpy(loaded_template, id);
  return template_load_result;
}

char *btech_attribute_read(GameDatabase *value, DbRef object, int attribute,
                           char *buffer) {
  assert(value == database);
  assert(object == 44);
  assert(attribute == A_MECHTYPE);
  strcpy(buffer, "restore-template");
  return buffer;
}

void mech_events_cancel_all(Mech *value) {
  assert(value == mech);
  events_cancelled++;
}

void clear_mech_from_los(Mech *value) {
  assert(value == mech);
  los_cleared++;
}

void mecha_notify(EvaluationContext *value [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {}

static void test_loadnew_and_restore_cleanup_match(void) {
  char loadnew[] = "load-template";

  reset_state();
  mechrep_rloadnew(99, &facility, loadnew);
  assert(strcmp(loaded_template, "load-template") == 0);
  assert(events_cancelled == 1);
  assert(los_cleared == 1);

  mechrep_rrestore(99, &facility, nullptr);
  assert(strcmp(loaded_template, "restore-template") == 0);
  assert(events_cancelled == 2);
  assert(los_cleared == 2);
}

static void test_failed_load_does_not_cleanup(void) {
  char loadnew[] = "broken-template";

  reset_state();
  template_load_result = false;
  mechrep_rloadnew(99, &facility, loadnew);
  assert(events_cancelled == 0);
  assert(los_cleared == 0);

  mechrep_rrestore(99, &facility, nullptr);
  assert(events_cancelled == 0);
  assert(los_cleared == 0);
}

int main(void) {
  test_loadnew_and_restore_cleanup_match();
  test_failed_load_does_not_cleanup();
  return 0;
}
