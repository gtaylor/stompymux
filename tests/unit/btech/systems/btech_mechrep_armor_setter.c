#include "mechrep_api.h"

#undef NDEBUG
#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
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

typedef struct ArmorValues {
  int armor;
  int original_armor;
  int internal;
  int original_internal;
  int rear;
  int original_rear;
} ArmorValues;

static ArmorValues values[NUM_SECTIONS];

static ArmorValues *armor_values_at(size_t section) {
  return checked_storage_at(values, NUM_SECTIONS, sizeof(*values), section);
}

static void reset_state(void) {
  command_status = REPAIR_COMMAND_READY;
  for (size_t section = 0; section < NUM_SECTIONS; section++) {
    *armor_values_at(section) = (ArmorValues){.armor = 10,
                                              .original_armor = 11,
                                              .internal = 12,
                                              .original_internal = 13,
                                              .rear = 14,
                                              .original_rear = 15};
  }
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
  if (strcmp(string, "LTORSO") == 0)
    return LTORSO;
  if (strcmp(string, "RTORSO") == 0)
    return RTORSO;
  if (strcmp(string, "LARM") == 0)
    return LARM;
  return -1;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *msg [[maybe_unused]]) {}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {}

void mech_section_armor_set(Mech *mech [[maybe_unused]], int section,
                            int armor) {
  armor_values_at((size_t)section)->armor = armor;
}

void mech_section_original_armor_set(Mech *mech [[maybe_unused]], int section,
                                     int armor) {
  armor_values_at((size_t)section)->original_armor = armor;
}

void mech_section_internal_set(Mech *mech [[maybe_unused]], int section,
                               int internal) {
  armor_values_at((size_t)section)->internal = internal;
}

void mech_section_original_internal_set(Mech *mech [[maybe_unused]],
                                        int section, int internal) {
  armor_values_at((size_t)section)->original_internal = internal;
}

void mech_section_rear_armor_set(Mech *mech [[maybe_unused]], int section,
                                 int armor) {
  armor_values_at((size_t)section)->rear = armor;
}

void mech_section_original_rear_armor_set(Mech *mech [[maybe_unused]],
                                          int section, int armor) {
  armor_values_at((size_t)section)->original_rear = armor;
}

static void invoke(const char *input) {
  char buffer[128];

  assert(snprintf(buffer, sizeof(buffer), "%s", input) >= 0);
  mechrep_rsetarmor(99, MECH, buffer);
}

static void assert_section_unchanged(int section) {
  const ArmorValues *value = armor_values_at((size_t)section);

  assert(value->armor == 10);
  assert(value->original_armor == 11);
  assert(value->internal == 12);
  assert(value->original_internal == 13);
  assert(value->rear == 14);
  assert(value->original_rear == 15);
}

static const char *invalid_input_at(size_t index) {
  switch (index) {
  case 0:
    return "CTORSO 20 malformed 22";
  case 1:
    return "CTORSO 256";
  case 2:
    return "CTORSO 2147483648";
  case 3:
    return "CTORSO 20 -1";
  case 4:
    return "CTORSO 20 21 malformed";
  case 5:
    return "CTORSO 20 21 -1";
  case 6:
    return "CTORSO 20 21 22 extra";
  default:
    assert(false);
    return nullptr;
  }
}

static void test_rejections_are_atomic(void) {
  for (size_t index = 0; index < 7; index++) {
    reset_state();
    invoke(invalid_input_at(index));
    assert_section_unchanged(CTORSO);
  }

  reset_state();
  invoke("HEAD 20 21 22");
  assert_section_unchanged(HEAD);

  reset_state();
  invoke("LARM 20 21 22");
  assert_section_unchanged(LARM);
}

static void test_boundaries_and_valid_torso_update(void) {
  reset_state();
  invoke("CTORSO 0 0 0");
  assert(values[CTORSO].armor == 0);
  assert(values[CTORSO].original_armor == 0);
  assert(values[CTORSO].internal == 0);
  assert(values[CTORSO].original_internal == 0);
  assert(values[CTORSO].rear == 0);
  assert(values[CTORSO].original_rear == 0);

  reset_state();
  invoke("CTORSO 255 255 255");
  assert(values[CTORSO].armor == 255);
  assert(values[CTORSO].original_armor == 255);
  assert(values[CTORSO].internal == 255);
  assert(values[CTORSO].original_internal == 255);
  assert(values[CTORSO].rear == 255);
  assert(values[CTORSO].original_rear == 255);
}

static void test_no_target_or_authorization_never_mutates(void) {
  reset_state();
  command_status = REPAIR_COMMAND_MISSING_MECH;
  invoke("CTORSO 20 21 22");
  assert_section_unchanged(CTORSO);

  reset_state();
  command_status = REPAIR_COMMAND_UNAUTHORIZED;
  invoke("CTORSO 20 21 22");
  assert_section_unchanged(CTORSO);
}

int main(void) {
  test_rejections_are_atomic();
  test_boundaries_and_valid_torso_update();
  test_no_target_or_authorization_never_mutates();
  return 0;
}
