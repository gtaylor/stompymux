#include "repair_job.h"

#include <stdbool.h>
#include <stdint.h>

#include "btech_event.h"
#include "mech_identity_api.h"
#include "mech_tech_api.h"
#include "registry_api.h"

typedef struct ScheduledEvent {
  BtechContext *context;
  void *object;
  int type;
  MuxEventCallback callback;
  int delay;
  intptr_t data;
} ScheduledEvent;

static ScheduledEvent scheduled;
static int schedule_count;
static int notification_count;
static int normal_roll;
static int weapon_roll;
static int time_added;
static int time_scale_divisor = 1;
static int resource_result;
static int failure_result;
static int success_result;
static int resource_count;
static int failure_count;
static int success_count;
static RepairOperationCall last_call;
static BtechContext *const test_context = (BtechContext *)(uintptr_t)0x1;

static void reset_test_state(void) {
  scheduled = (ScheduledEvent){0};
  schedule_count = 0;
  notification_count = 0;
  normal_roll = 0;
  weapon_roll = 0;
  time_added = 100;
  time_scale_divisor = 1;
  resource_result = 0;
  failure_result = 0;
  success_result = 0;
  resource_count = 0;
  failure_count = 0;
  success_count = 0;
  last_call = (RepairOperationCall){0};
}

int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
              int difficulty [[maybe_unused]]) {
  return normal_roll;
}

int tech_weapon_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                     int difficulty [[maybe_unused]]) {
  return weapon_roll;
}

int tech_addtechtime(const TechTimeAddition *addition) {
  if (addition->context != test_context)
    return -1000;
  return time_added + addition->units;
}

int tech_time_scaled_seconds(BtechContext *input, int units) {
  if (input != test_context)
    return -1000;
  return units / time_scale_divisor;
}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {
  notification_count++;
}

void btech_context_event_schedule(BtechContext *input, void *object, int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data) {
  scheduled = (ScheduledEvent){.context = input,
                               .object = object,
                               .type = type,
                               .callback = callback,
                               .delay = delay,
                               .data = data};
  schedule_count++;
}

void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}

static int resource(const RepairOperationCall *call) {
  resource_count++;
  last_call = *call;
  return resource_result;
}

static int failure(const RepairOperationCall *call) {
  failure_count++;
  last_call = *call;
  return failure_result;
}

static int success(const RepairOperationCall *call) {
  success_count++;
  last_call = *call;
  return success_result;
}

static void event_callback(MuxEvent *event [[maybe_unused]]) {}

static bool scheduled_as(Mech *mech, int type, MuxEventCallback callback,
                         int delay, RepairEventPayload payload) {
  RepairEventPayload actual = repair_event_payload_unpack(scheduled.data);
  return schedule_count == 1 && scheduled.context == test_context &&
         scheduled.object == mech && scheduled.type == type &&
         scheduled.callback == callback && scheduled.delay == delay &&
         actual.location == payload.location &&
         actual.position == payload.position && actual.extra == payload.extra &&
         actual.player == payload.player;
}

static bool test_part_job_paths(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x1234;
  RepairCommandContext command = {.player = 91,
                                  .context = test_context,
                                  .evaluation = nullptr,
                                  .mech = mech};
  const RepairPartJob job = {
      .difficulty = 7,
      .time = 30,
      .event_data = repair_event_payload_pack((RepairEventPayload){
          .location = 1, .position = 2, .extra = 3, .player = 4}),
      .event_type = 55,
      .event_callback = event_callback,
      .message = "repair",
      .resource = resource,
      .failure = failure,
      .success = success};

  reset_test_state();
  resource_result = -1;
  if (repair_part_job_execute(&command, 8, 9, &job) != REPAIR_JOB_REJECTED ||
      resource_count != 1 || notification_count != 0 || schedule_count != 0)
    return false;

  reset_test_state();
  if (repair_part_job_execute(&command, 8, 9, &job) !=
          REPAIR_JOB_SCHEDULED_SUCCESS ||
      resource_count != 1 || success_count != 1 || failure_count != 0 ||
      notification_count != 1 || last_call.player != command.player ||
      last_call.mech != mech || last_call.selection.location != 8 ||
      last_call.selection.part != 9 ||
      !scheduled_as(
          mech, 55, event_callback, 130,
          (RepairEventPayload){
              .location = 1, .position = 2, .extra = 3, .player = 91}))
    return false;

  reset_test_state();
  normal_roll = -1;
  failure_result = -1;
  if (repair_part_job_execute(&command, 8, 9, &job) !=
          REPAIR_JOB_SCHEDULED_FAILURE ||
      failure_count != 1 || success_count != 0 ||
      !scheduled_as(
          mech, 55, mech_event_failure_marker, 145,
          (RepairEventPayload){
              .location = 1, .position = 2, .extra = 3, .player = 91}))
    return false;

  reset_test_state();
  normal_roll = -1;
  if (repair_part_job_execute(&command, 8, 9, &job) !=
          REPAIR_JOB_SCHEDULED_SUCCESS ||
      !scheduled_as(
          mech, 55, event_callback, 145,
          (RepairEventPayload){
              .location = 1, .position = 2, .extra = 3, .player = 91}))
    return false;

  reset_test_state();
  success_result = -1;
  if (repair_part_job_execute(&command, 8, 9, &job) !=
          REPAIR_JOB_CALLBACK_ABORTED ||
      resource_count != 1 || success_count != 1 || notification_count != 1 ||
      schedule_count != 0)
    return false;

  reset_test_state();
  RepairPartJob weapon_job = job;
  weapon_job.weapon_roll = true;
  weapon_roll = -1;
  failure_result = -1;
  if (repair_part_job_execute(&command, 8, 9, &weapon_job) !=
          REPAIR_JOB_SCHEDULED_FAILURE ||
      !scheduled_as(
          mech, 55, mech_event_failure_marker, 145,
          (RepairEventPayload){
              .location = 1, .position = 2, .extra = 3, .player = 91}))
    return false;
  return true;
}

static bool test_amount_job_paths(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x4321;
  RepairCommandContext command = {.player = 11,
                                  .context = test_context,
                                  .evaluation = nullptr,
                                  .mech = mech};
  const RepairPartAmountJob part_job = {.difficulty = 2,
                                        .time = 20,
                                        .event_type = 71,
                                        .event_callback = event_callback,
                                        .message = "reload",
                                        .resource = resource,
                                        .failure = failure,
                                        .success = success};
  int amount = 4;

  reset_test_state();
  amount = -1;
  if (repair_part_amount_job_execute(&command, 2, 3, &amount, &part_job) !=
          REPAIR_JOB_REJECTED ||
      resource_count != 0 || notification_count != 0 || schedule_count != 0)
    return false;

  reset_test_state();
  amount = 4;
  time_scale_divisor = 1;
  if (repair_part_amount_job_execute(&command, 2, 3, &amount, &part_job) !=
          REPAIR_JOB_SCHEDULED_SUCCESS ||
      last_call.amount != &amount ||
      !scheduled_as(
          mech, 71, event_callback, 105,
          (RepairEventPayload){
              .location = 2, .position = 3, .extra = 4, .player = 11}))
    return false;

  reset_test_state();
  normal_roll = -1;
  failure_result = -1;
  amount = 6;
  if (repair_part_amount_job_execute(&command, 2, 3, &amount, &part_job) !=
          REPAIR_JOB_SCHEDULED_FAILURE ||
      !scheduled_as(
          mech, 71, mech_event_failure_marker, 114,
          (RepairEventPayload){
              .location = 2, .position = 3, .extra = 6, .player = 11}))
    return false;

  const RepairSectionAmountJob section_job = {
      .difficulty = 2,
      .failure_time = 40,
      .unit_time = 10,
      .failure_event_type = 72,
      .event_type = 73,
      .event_callback = event_callback,
      .message = "section",
      .resource = resource,
      .failure = failure,
      .success = success,
  };
  reset_test_state();
  amount = 0;
  if (repair_section_amount_job_execute(&command, 6, &amount, &section_job) !=
          REPAIR_JOB_REJECTED ||
      resource_count != 0 || notification_count != 0 || schedule_count != 0)
    return false;

  reset_test_state();
  amount = REPAIR_FIX_AMOUNT_MAX + 1;
  if (repair_section_amount_job_execute(&command, 6, &amount, &section_job) !=
          REPAIR_JOB_REJECTED ||
      resource_count != 0 || notification_count != 0 || schedule_count != 0)
    return false;

  reset_test_state();
  amount = 3;
  if (repair_section_amount_job_execute(&command, 6, &amount, &section_job) !=
          REPAIR_JOB_SCHEDULED_SUCCESS ||
      !scheduled_as(
          mech, 73, event_callback, 110,
          (RepairEventPayload){
              .location = 6, .position = 3, .extra = 0, .player = 11}))
    return false;

  reset_test_state();
  normal_roll = -1;
  failure_result = -1;
  amount = 3;
  if (repair_section_amount_job_execute(&command, 6, &amount, &section_job) !=
          REPAIR_JOB_SCHEDULED_FAILURE ||
      !scheduled_as(mech, 72, mech_event_failure_marker, 160,
                    (RepairEventPayload){.location = 6, .player = 11}))
    return false;
  return true;
}

int main(void) {
  return test_part_job_paths() && test_amount_job_paths() ? 0 : 1;
}
