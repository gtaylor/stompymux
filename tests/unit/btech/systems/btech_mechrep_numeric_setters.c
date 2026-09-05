#include "mechrep_api.h"

#undef NDEBUG
#include <assert.h>
#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "mech_api_types.h"
#include "mech_electronics_api.h"
#include "mech_specification_api.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_helpers.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"

static BtechContext *const CONTEXT = (BtechContext *)1;
static EvaluationContext *const EVALUATION = (EvaluationContext *)2;
static Mech *const MECH = (Mech *)3;
static CommandContext command_context;
static RepairCommandStatus command_status;
static float maximum_speed;
static float jump_speed;
static int heat_sinks;
static int lrs_range;
static int tac_range;
static int scan_range;
static int radio_range;
static int current_tonnage;
static DbRef matched_target;
static bool matched_target_is_mech;

static void reset_state(void) {
  command_status = REPAIR_COMMAND_READY;
  maximum_speed = 12.5F;
  jump_speed = 7.5F;
  heat_sinks = 10;
  lrs_range = 11;
  tac_range = 12;
  scan_range = 13;
  radio_range = 14;
  current_tonnage = 15;
  matched_target = NOTHING;
  matched_target_is_mech = false;
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

CommandContext *btech_context_command(BtechContext *context) {
  return context == CONTEXT ? &command_context : nullptr;
}

bool btech_context_is_mech(BtechContext *context, DbRef key) {
  return context == CONTEXT && key == matched_target && matched_target_is_mech;
}

DbRef match_thing(MatchContext *match [[maybe_unused]],
                  DbRef player [[maybe_unused]], char *name [[maybe_unused]]) {
  return matched_target;
}

int mech_parseattributes(char *buffer, char **args, int maxargs) {
  int count = 0;
  for (char *token = strtok(buffer, " "); token != nullptr && count < maxargs;
       token = strtok(nullptr, " "))
    *(char **)checked_storage_at((void *)args, (size_t)maxargs, sizeof(*args),
                                 (size_t)count++) = token;
  return count;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *msg [[maybe_unused]]) {}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {}

float mech_maximum_speed(const Mech *mech [[maybe_unused]]) {
  return maximum_speed;
}

void mech_maximum_speed_set(Mech *mech [[maybe_unused]], float speed) {
  maximum_speed = speed;
}

float mech_jump_speed(const Mech *mech [[maybe_unused]]) { return jump_speed; }

void mech_jump_speed_set(Mech *mech [[maybe_unused]], float speed) {
  jump_speed = speed;
}

int mech_heat_sink_count(const Mech *mech [[maybe_unused]]) {
  return heat_sinks;
}

void mech_heat_sink_count_set(Mech *mech [[maybe_unused]], int count) {
  heat_sinks = count;
}

int mech_long_range_sensor_range(const Mech *mech [[maybe_unused]]) {
  return lrs_range;
}

void mech_long_range_sensor_range_set(Mech *mech [[maybe_unused]], int range) {
  lrs_range = range;
}

int mech_tactical_range(const Mech *mech [[maybe_unused]]) { return tac_range; }

void mech_tactical_range_set(Mech *mech [[maybe_unused]], int range) {
  tac_range = range;
}

int mech_scanner_range(const Mech *mech [[maybe_unused]]) { return scan_range; }

void mech_scanner_range_set(Mech *mech [[maybe_unused]], int range) {
  scan_range = range;
}

int mech_radio_range(const Mech *mech [[maybe_unused]]) { return radio_range; }

void mech_radio_range_set(Mech *mech [[maybe_unused]], int range) {
  radio_range = range;
}

int mech_tonnage(const Mech *mech [[maybe_unused]]) { return current_tonnage; }

void mech_tonnage_set(Mech *mech [[maybe_unused]], int tonnage) {
  current_tonnage = tonnage;
}

typedef void (*MechrepCommand)(DbRef player, void *data, char *buffer);

static void invoke(MechrepCommand command, const char *input) {
  char buffer[64];

  assert(strlen(input) < sizeof(buffer));
  assert(snprintf(buffer, sizeof(buffer), "%s", input) >= 0);
  command(99, MECH, buffer);
}

#pragma clang unsafe_buffer_usage begin
static void test_speed_setters_reject_invalid_values(void) {
  static const char *const INVALID_VALUES[] = {
      "nan", "inf", "junk", "2 trailing", "3.2e37", "3.4028235e38", "-1"};

  reset_state();
  invoke(mechrep_rsetspeed, "3.5");
  assert(maximum_speed == 37.625F);
  for (size_t index = 0;
       index < sizeof(INVALID_VALUES) / sizeof(*INVALID_VALUES); index++) {
    maximum_speed = 12.5F;
    invoke(mechrep_rsetspeed, INVALID_VALUES[index]);
    assert(maximum_speed == 12.5F);
  }

  reset_state();
  invoke(mechrep_rsetjumpspeed, "2.5");
  assert(jump_speed == 26.875F);
  for (size_t index = 0;
       index < sizeof(INVALID_VALUES) / sizeof(*INVALID_VALUES); index++) {
    jump_speed = 7.5F;
    invoke(mechrep_rsetjumpspeed, INVALID_VALUES[index]);
    assert(jump_speed == 7.5F);
  }
}

typedef struct MechrepIntegerSetterTest {
  MechrepCommand command;
  int *storage;
  int minimum;
  int maximum;
} MechrepIntegerSetterTest;

static void test_integer_setters_reject_unsafe_values(void) {
  const MechrepIntegerSetterTest TESTS[] = {
      {.command = mechrep_rsetheatsinks,
       .storage = &heat_sinks,
       .minimum = 0,
       .maximum = CHAR_MAX},
      {.command = mechrep_rsetlrsrange,
       .storage = &lrs_range,
       .minimum = 0,
       .maximum = CHAR_MAX},
      {.command = mechrep_rsettacrange,
       .storage = &tac_range,
       .minimum = 0,
       .maximum = CHAR_MAX},
      {.command = mechrep_rsetscanrange,
       .storage = &scan_range,
       .minimum = 0,
       .maximum = CHAR_MAX},
      {.command = mechrep_rsetradiorange,
       .storage = &radio_range,
       .minimum = 0,
       .maximum = SHRT_MAX},
      {.command = mechrep_rsettons,
       .storage = &current_tonnage,
       .minimum = 1,
       .maximum = INT_MAX},
  };

  for (size_t index = 0; index < sizeof(TESTS) / sizeof(*TESTS); index++) {
    char valid[32];
    char above_maximum[32];

    reset_state();
    assert(snprintf(valid, sizeof(valid), "%d", TESTS[index].minimum) > 0);
    invoke(TESTS[index].command, valid);
    assert(*TESTS[index].storage == TESTS[index].minimum);

    assert(snprintf(valid, sizeof(valid), "%d", TESTS[index].maximum) > 0);
    invoke(TESTS[index].command, valid);
    assert(*TESTS[index].storage == TESTS[index].maximum);

    *TESTS[index].storage = 77;
    assert(snprintf(above_maximum, sizeof(above_maximum), "%lld",
                    (long long)TESTS[index].maximum + 1LL) > 0);
    invoke(TESTS[index].command, above_maximum);
    assert(*TESTS[index].storage == 77);

    invoke(TESTS[index].command, "-1");
    assert(*TESTS[index].storage == 77);
    invoke(TESTS[index].command, "junk");
    assert(*TESTS[index].storage == 77);
    invoke(TESTS[index].command, "1 trailing");
    assert(*TESTS[index].storage == 77);
    invoke(TESTS[index].command, "2147483648");
    assert(*TESTS[index].storage == 77);
  }
}
#pragma clang unsafe_buffer_usage end

static void test_setters_reject_unavailable_contexts(void) {
  reset_state();
  command_status = REPAIR_COMMAND_UNAUTHORIZED;
  invoke(mechrep_rsetspeed, "4");
  assert(maximum_speed == 12.5F);
  invoke(mechrep_rsetheatsinks, "20");
  assert(heat_sinks == 10);

  command_status = REPAIR_COMMAND_MISSING_MECH;
  invoke(mechrep_rsetspeed, "4");
  assert(maximum_speed == 12.5F);
  invoke(mechrep_rsetheatsinks, "20");
  assert(heat_sinks == 10);
}

int main(void) {
  test_speed_setters_reject_invalid_values();
  test_integer_setters_reject_unsafe_values();
  test_setters_reject_unavailable_contexts();
  return 0;
}
