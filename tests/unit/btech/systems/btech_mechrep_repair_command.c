#include "mechrep_api.h"

#undef NDEBUG
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_utils_api.h"
#include "mux/network/network_output.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

static BtechContext *const CONTEXT = (BtechContext *)1;
static EvaluationContext *const EVALUATION = (EvaluationContext *)2;
static Mech *const MECH = (Mech *)3;
static RepairCommandStatus command_status;
static int armor;
static int internal;
static int rear_armor;
static int critical_count;
static int repaired_parts;
static int repaired_section;
static int repaired_critical;
static int reattached_sections;
static int reattached_section;

static void reset_state(void) {
  command_status = REPAIR_COMMAND_READY;
  armor = 10;
  internal = 20;
  rear_armor = 30;
  critical_count = 2;
  repaired_parts = 0;
  repaired_section = -1;
  repaired_critical = -1;
  reattached_sections = 0;
  reattached_section = -1;
}

RepairCommandStatus
mech_admin_command_context_initialize(DbRef player, void *data,
                                      MechAdminCommandContext *command) {
  assert(data == MECH);
  *command = (MechAdminCommandContext){.player = player,
                                       .context = CONTEXT,
                                       .evaluation = EVALUATION,
                                       .mech = MECH};
  return command_status;
}

const char *repair_command_status_message(RepairCommandStatus status
                                          [[maybe_unused]]) {
  return "Unavailable.";
}

EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return context == CONTEXT ? EVALUATION : nullptr;
}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return CONTEXT;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }

MechMovementType mech_movement_type(const Mech *mech [[maybe_unused]]) {
  return MOVE_BIPED;
}

int mech_parseattributes(char *buffer, char **args, int maxargs) {
  int count = 0;

  for (char *token = strtok(buffer, " "); token != nullptr && count < maxargs;
       token = strtok(nullptr, " ")) {
    *(char **)checked_storage_at((void *)args, (size_t)maxargs, sizeof(*args),
                                 (size_t)count++) = token;
  }
  return count;
}

int armor_section_from_string(UnitClass type [[maybe_unused]],
                              MechMovementType movement_type [[maybe_unused]],
                              const char *string) {
  if (strcmp(string, "HEAD") == 0)
    return HEAD;
  if (strcmp(string, "CTORSO") == 0)
    return CTORSO;
  return -1;
}

int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return critical_count;
}

// NOLINTBEGIN(bugprone-easily-swappable-parameters)
void mech_section_armor_set(Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]], int armor_value) {
  armor = armor_value;
}

void mech_section_internal_set(Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]],
                               int internal_value) {
  internal = internal_value;
}

void mech_section_rear_armor_set(Mech *mech [[maybe_unused]],
                                 int section [[maybe_unused]],
                                 int armor_value) {
  rear_armor = armor_value;
}

void mech_repair_part(Mech *mech [[maybe_unused]], int loc, int pos) {
  repaired_parts++;
  repaired_section = loc;
  repaired_critical = pos;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

void mech_re_attach(Mech *mech [[maybe_unused]], int loc) {
  reattached_sections++;
  reattached_section = loc;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *msg [[maybe_unused]]) {}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {}

static void invoke(const char *input) {
  char buffer[96];

  assert(snprintf(buffer, sizeof(buffer), "%s", input) >= 0);
  mechrep_rrepair(99, MECH, buffer);
}

static void assert_unchanged(void) {
  assert(armor == 10);
  assert(internal == 20);
  assert(rear_armor == 30);
  assert(repaired_parts == 0);
  assert(reattached_sections == 0);
}

#pragma clang unsafe_buffer_usage begin
static void test_value_repairs_are_strict_and_bounded(void) {
  reset_state();
  invoke("HEAD A 255");
  assert(armor == 255);
  assert(internal == 20);
  assert(rear_armor == 30);

  reset_state();
  invoke("HEAD I 0");
  assert(armor == 10);
  assert(internal == 0);
  assert(rear_armor == 30);

  reset_state();
  invoke("CTORSO R 255");
  assert(armor == 10);
  assert(internal == 20);
  assert(rear_armor == 255);

  static const char *const INVALID_COMMANDS[] = {
      "HEAD A",
      "HEAD A malformed",
      "HEAD A -1",
      "HEAD A 256",
      "HEAD A 1 extra",
      "HEAD I",
      "HEAD I malformed",
      "HEAD I -1",
      "HEAD I 256",
      "HEAD I 1 extra",
      "CTORSO R",
      "CTORSO R malformed",
      "CTORSO R -1",
      "CTORSO R 256",
      "CTORSO R 1 extra",
      "HEAD R 1",
      "HEAD R 1 extra extra",
  };
  for (size_t index = 0;
       index < sizeof(INVALID_COMMANDS) / sizeof(*INVALID_COMMANDS); index++) {
    reset_state();
    invoke(INVALID_COMMANDS[index]);
    assert_unchanged();
  }
}

static void test_critical_positions_use_the_section_limit(void) {
  reset_state();
  critical_count = 2;
  invoke("HEAD C 2");
  assert(repaired_parts == 1);
  assert(repaired_section == HEAD);
  assert(repaired_critical == 1);

  static const char *const INVALID_COMMANDS[] = {
      "HEAD C",   "HEAD C malformed", "HEAD C 0", "HEAD C -1",
      "HEAD C 3", "HEAD C 2 extra",   "NOPE C 1",
  };
  for (size_t index = 0;
       index < sizeof(INVALID_COMMANDS) / sizeof(*INVALID_COMMANDS); index++) {
    reset_state();
    critical_count = 2;
    invoke(INVALID_COMMANDS[index]);
    assert_unchanged();
  }
}

static void test_section_reattach_has_exact_arguments(void) {
  reset_state();
  invoke("HEAD S");
  assert(reattached_sections == 1);
  assert(reattached_section == HEAD);

  reset_state();
  invoke("HEAD S value");
  assert_unchanged();

  reset_state();
  invoke("HEAD S value extra");
  assert_unchanged();
}

static void test_rejected_contexts_never_mutate(void) {
  const RepairCommandStatus STATUSES[] = {REPAIR_COMMAND_MISSING_MECH,
                                          REPAIR_COMMAND_UNAUTHORIZED};

  for (size_t index = 0; index < sizeof(STATUSES) / sizeof(*STATUSES);
       index++) {
    reset_state();
    command_status = STATUSES[index];
    invoke("CTORSO R 12");
    assert_unchanged();
  }
}
#pragma clang unsafe_buffer_usage end

int main(void) {
  test_value_repairs_are_strict_and_bounded();
  test_critical_positions_use_the_section_limit();
  test_section_reattach_has_exact_arguments();
  test_rejected_contexts_never_mutate();
  return 0;
}
