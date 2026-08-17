#include "mech_tech_commands_api.h"

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"

typedef struct ScheduledEvent {
  int type;
  MuxEventCallback callback;
  int delay;
  RepairEventPayload payload;
} ScheduledEvent;

static BtechContext *const test_context = (BtechContext *)(uintptr_t)0x1;
static Mech *const test_mech = (Mech *)(uintptr_t)0x2;
static EvaluationContext *const test_evaluation =
    (EvaluationContext *)(uintptr_t)0x3;
static GameObject database_objects[16];
static GameDatabase test_database = {.object_storage = database_objects,
                                     .size = 15};
static char test_buffer[] = "ignored";
static TechPartParseResult parse_result;
static bool parser_allow_rear;
static bool parser_position;
static bool parser_extra;
static bool started;
static bool starting;
static bool require_stall;
static int repair_stall;
static bool destroyed;
static bool flooded;
static bool fixing_armor;
static bool fixing_internal;
static int fixing_armor_location;
static int fixing_internal_location;
static bool scrapping;
static int armor[8];
static int original_armor[8];
static int rear_armor[8];
static int original_rear_armor[8];
static int internal[8];
static int original_internal[8];
static int technology_time;
static int maximum_technology_time;
static int roll_result;
static int resource_result;
static int failure_result;
static int success_result;
static int partial_amount;
static int resource_count;
static int resource_amount;
static int failure_count;
static int success_count;
static int notification_count;
static const char *last_notification;
static int techtime_units;
static int schedule_count;
static ScheduledEvent scheduled;

static int *section_value(int values[8], int location) {
  return checked_storage_at(values, 8, sizeof(*values), (size_t)location);
}

static void reset_test_state(void) {
  parse_result = (TechPartParseResult){.status = TECH_PART_PARSE_OK,
                                       .location = 2};
  parser_allow_rear = false;
  parser_position = true;
  parser_extra = true;
  started = false;
  starting = false;
  require_stall = false;
  repair_stall = 1;
  destroyed = false;
  flooded = false;
  fixing_armor = false;
  fixing_internal = false;
  fixing_armor_location = 2;
  fixing_internal_location = 2;
  scrapping = false;
  for (int location = 0; location < 8; location++) {
    *section_value(armor, location) = 10;
    *section_value(original_armor, location) = 10;
    *section_value(rear_armor, location) = 6;
    *section_value(original_rear_armor, location) = 6;
    *section_value(internal, location) = 5;
    *section_value(original_internal, location) = 5;
  }
  technology_time = 0;
  maximum_technology_time = 100;
  roll_result = 0;
  resource_result = 0;
  failure_result = -1;
  success_result = 0;
  partial_amount = 1;
  resource_count = 0;
  resource_amount = -1;
  failure_count = 0;
  success_count = 0;
  notification_count = 0;
  last_notification = nullptr;
  techtime_units = 0;
  schedule_count = 0;
  scheduled = (ScheduledEvent){0};
  memset(database_objects, 0, sizeof(database_objects));
}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}
EvaluationContext *btech_context_evaluation(BtechContext *context) {
  return context == test_context ? test_evaluation : nullptr;
}
GameDatabase *btech_context_database(BtechContext *context) {
  return context == test_context ? &test_database : nullptr;
}
bool btech_context_limits_repairs_to_stalls(const BtechContext *context
                                            [[maybe_unused]]) {
  return require_stall;
}
int btech_context_maximum_technology_time(const BtechContext *context
                                          [[maybe_unused]]) {
  return maximum_technology_time;
}
bool mech_is_dropship(const Mech *mech [[maybe_unused]]) { return false; }
bool mech_is_started(const Mech *mech [[maybe_unused]]) { return started; }
DbRef mech_repair_stall_dbref(const Mech *mech [[maybe_unused]]) {
  return repair_stall;
}
int mech_event_count(const Mech *mech [[maybe_unused]], MechEventType type) {
  return type == EVENT_STARTUP && starting;
}

TechPartParseResult tech_part_parse(const TechPartParseRequest *request) {
  parser_allow_rear = request->allow_rear;
  parser_position = request->parse_position;
  parser_extra = request->parse_extra;
  return parse_result;
}
int player_techtime(BtechContext *context [[maybe_unused]],
                    DbRef player [[maybe_unused]]) {
  return technology_time;
}
int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
             int difficulty [[maybe_unused]]) {
  return roll_result;
}
int tech_addtechtime(const TechTimeAddition *addition) {
  techtime_units = addition->units;
  return addition->units;
}
int tech_time_scaled_seconds(BtechContext *context [[maybe_unused]], int units) {
  return units;
}

int mech_section_armor(const Mech *mech [[maybe_unused]], int location) {
  return *section_value(armor, location);
}
int mech_section_original_armor(const Mech *mech [[maybe_unused]],
                                int location) {
  return *section_value(original_armor, location);
}
int mech_section_rear_armor(const Mech *mech [[maybe_unused]], int location) {
  return *section_value(rear_armor, location);
}
int mech_section_original_rear_armor(const Mech *mech [[maybe_unused]],
                                     int location) {
  return *section_value(original_rear_armor, location);
}
int mech_section_internal(const Mech *mech [[maybe_unused]], int location) {
  return *section_value(internal, location);
}
int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int location) {
  return *section_value(original_internal, location);
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int location [[maybe_unused]]) {
  return destroyed;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int location [[maybe_unused]]) {
  return flooded;
}

int someone_fixing_a(Mech *mech [[maybe_unused]], int location) {
  return fixing_armor && location == fixing_armor_location;
}
int someone_fixing_i(Mech *mech [[maybe_unused]], int location) {
  return fixing_internal && location == fixing_internal_location;
}
bool someone_fixing(Mech *mech, int location) {
  return someone_fixing_a(mech, location) || someone_fixing_i(mech, location);
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]],
                          int location [[maybe_unused]]) {
  return scrapping;
}

int fixarmor_econ(const RepairOperationCall *call) {
  resource_count++;
  resource_amount = *call->amount;
  return resource_result;
}
int fixinternal_econ(const RepairOperationCall *call) {
  resource_count++;
  resource_amount = *call->amount;
  return resource_result;
}
int fixarmor_fail(const RepairOperationCall *call) {
  failure_count++;
  if (failure_result == 0)
    *call->amount = partial_amount;
  return failure_result;
}
int fixinternal_fail(const RepairOperationCall *call) {
  failure_count++;
  if (failure_result == 0)
    *call->amount = partial_amount;
  return failure_result;
}
int fixarmor_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_count++;
  return success_result;
}
int fixinternal_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_count++;
  return success_result;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]], const char *message) {
  notification_count++;
  last_notification = message;
}
void btech_context_event_schedule(BtechContext *context [[maybe_unused]],
                                  void *object [[maybe_unused]], int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data) {
  schedule_count++;
  scheduled = (ScheduledEvent){.type = type,
                               .callback = callback,
                               .delay = delay,
                               .payload = repair_event_payload_unpack(data)};
}
void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairarmor(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairinternal(MuxEvent *event [[maybe_unused]]) {}

static bool notification_is(const char *message) {
  return last_notification && strcmp(last_notification, message) == 0;
}

static bool test_context_and_stall_policies(void) {
  reset_test_state();
  started = true;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The mech's started up ; please shut it down first."))
    return false;

  reset_test_state();
  starting = true;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The mech's starting up! Please stop the sequence first."))
    return false;

  reset_test_state();
  repair_stall = 0;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The location doesn't need armor repair!") || resource_count)
    return false;

  reset_test_state();
  require_stall = true;
  repair_stall = 0;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The 'mech isn't in a repair stall!"))
    return false;

  reset_test_state();
  repair_stall = 0;
  tech_fixinternal(7, test_mech, test_buffer);
  return notification_is("The 'mech isn't in a repair stall!");
}

static bool test_parsing_and_rejection_guards(void) {
  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("Invalid section!") || !parser_allow_rear)
    return false;

  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID_POSITION;
  tech_fixinternal(7, test_mech, test_buffer);
  if (!notification_is("Invalid part!") || parser_allow_rear || parser_position ||
      parser_extra)
    return false;

  const char *const BLOCKED[] = {
      "That part's blown off! Use reattach first!",
      "That location has been flooded! Use reseal first!",
      "Someone's repairing that section already!",
      "Someone's scrapping that section - no repairs are possible!",
  };
  for (int blocker = 0; blocker < 4; blocker++) {
    reset_test_state();
    armor[2] = 8;
    if (blocker == 0)
      destroyed = true;
    else if (blocker == 1)
      flooded = true;
    else if (blocker == 2)
      fixing_armor = true;
    else
      scrapping = true;
    tech_fixarmor(7, test_mech, test_buffer);
    const char *const *message_slot = checked_storage_at_const(
        BLOCKED, sizeof(BLOCKED) / sizeof(*BLOCKED), sizeof(*BLOCKED),
        (size_t)blocker);
    if (!notification_is(*message_slot) || resource_count || schedule_count)
      return false;
  }

  reset_test_state();
  armor[2] = 8;
  fixing_internal = true;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("Someone's repairing that section already!"))
    return false;

  reset_test_state();
  parse_result.location = 2 + NUM_SECTIONS;
  rear_armor[2] = 4;
  fixing_internal = true;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("Someone's repairing that section already!"))
    return false;

  reset_test_state();
  parse_result.location = 2 + NUM_SECTIONS;
  rear_armor[2] = 4;
  fixing_armor = true;
  fixing_armor_location = 2 + NUM_SECTIONS;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("Someone's repairing that section already!"))
    return false;

  reset_test_state();
  armor[2] = 8;
  internal[2] = 4;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The internals need to be fixed first!"))
    return false;

  reset_test_state();
  internal[2] = 4;
  fixing_armor = true;
  fixing_armor_location = 2 + NUM_SECTIONS;
  tech_fixinternal(7, test_mech, test_buffer);
  return notification_is("Someone's repairing that section already!");
}

static bool test_noop_fatigue_and_resource_rejection(void) {
  reset_test_state();
  armor[2] = 12;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("The location doesn't need armor repair!") ||
      resource_count || schedule_count)
    return false;

  reset_test_state();
  internal[2] = 7;
  tech_fixinternal(7, test_mech, test_buffer);
  if (!notification_is("The location doesn't need internals' repair!") ||
      resource_count || schedule_count)
    return false;

  reset_test_state();
  armor[2] = 8;
  technology_time = maximum_technology_time;
  tech_fixarmor(7, test_mech, test_buffer);
  if (!notification_is("You're too tired to do that!") || resource_count)
    return false;

  reset_test_state();
  internal[2] = 3;
  resource_result = -1;
  tech_fixinternal(7, test_mech, test_buffer);
  return resource_count == 1 && resource_amount == 2 && !notification_count &&
         !schedule_count;
}

static bool test_extended_amounts_and_out_of_range_rejection(void) {
  reset_test_state();
  armor[2] = 24;
  original_armor[2] = 40;
  tech_fixarmor(77, test_mech, test_buffer);
  if (resource_amount != 16 || schedule_count != 1 ||
      scheduled.payload.position != 0 || scheduled.payload.extra != 1 ||
      repair_fix_event_amount(scheduled.payload) != 16 ||
      scheduled.payload.player != 77)
    return false;

  reset_test_state();
  internal[2] = 3;
  original_internal[2] = 20;
  tech_fixinternal(91, test_mech, test_buffer);
  if (resource_amount != 17 || schedule_count != 1 ||
      scheduled.payload.position != 1 || scheduled.payload.extra != 1 ||
      repair_fix_event_amount(scheduled.payload) != 17 ||
      scheduled.payload.player != 91)
    return false;

  reset_test_state();
  internal[2] = 0;
  original_internal[2] = REPAIR_FIX_AMOUNT_MAX + 1;
  tech_fixinternal(91, test_mech, test_buffer);
  return !resource_count && !notification_count && !schedule_count;
}

static bool test_armor_schedules_front_rear_partial_and_total_failure(void) {
  reset_test_state();
  armor[2] = 6;
  tech_fixarmor(77, test_mech, test_buffer);
  if (resource_count != 1 || resource_amount != 4 || success_count != 1 ||
      failure_count || !notification_is("You start fixing the armor..") ||
      schedule_count != 1 || scheduled.type != EVENT_REPAIR_FIX ||
      scheduled.callback != mux_event_tickmech_repairarmor || scheduled.delay != 3 ||
      techtime_units != 12 || scheduled.payload.location != 2 ||
      scheduled.payload.position != 4 || scheduled.payload.player != 77)
    return false;

  reset_test_state();
  parse_result.location = 10;
  rear_armor[2] = 4;
  tech_fixarmor(77, test_mech, test_buffer);
  if (!parser_allow_rear || resource_amount != 2 || scheduled.type != EVENT_REPAIR_FIX ||
      scheduled.delay != 3 || scheduled.payload.location != 10 ||
      scheduled.payload.position != 2)
    return false;

  reset_test_state();
  armor[2] = 6;
  roll_result = -1;
  failure_result = 0;
  partial_amount = 2;
  tech_fixarmor(77, test_mech, test_buffer);
  if (failure_count != 1 || success_count || scheduled.type != EVENT_REPAIR_FIX ||
      scheduled.callback != mux_event_tickmech_repairarmor || scheduled.delay != 6 ||
      techtime_units != 9 || scheduled.payload.position != 2)
    return false;

  reset_test_state();
  armor[2] = 6;
  roll_result = -1;
  tech_fixarmor(77, test_mech, test_buffer);
  return failure_count == 1 && scheduled.type == EVENT_REPAIR_FIX &&
         scheduled.callback == mech_event_failure_marker && scheduled.delay == 18 &&
         techtime_units == 18 && scheduled.payload.location == 2 &&
         scheduled.payload.position == 0 && scheduled.payload.player == 77;
}

static bool test_internal_schedules_and_failure_marker_type(void) {
  reset_test_state();
  internal[2] = 2;
  tech_fixinternal(91, test_mech, test_buffer);
  if (resource_amount != 3 || success_count != 1 || scheduled.type != EVENT_REPAIR_FIXI ||
      scheduled.callback != mux_event_tickmech_repairinternal || scheduled.delay != 9 ||
      techtime_units != 27 || scheduled.payload.location != 2 ||
      scheduled.payload.position != 3 || scheduled.payload.player != 91)
    return false;

  reset_test_state();
  internal[2] = 2;
  roll_result = -1;
  failure_result = 0;
  partial_amount = 2;
  tech_fixinternal(91, test_mech, test_buffer);
  if (scheduled.type != EVENT_REPAIR_FIXI ||
      scheduled.callback != mux_event_tickmech_repairinternal || scheduled.delay != 18 ||
      techtime_units != 27 || scheduled.payload.position != 2)
    return false;

  reset_test_state();
  internal[2] = 2;
  roll_result = -1;
  tech_fixinternal(91, test_mech, test_buffer);
  return scheduled.type == EVENT_REPAIR_FIXI &&
         scheduled.callback == mech_event_failure_marker && scheduled.delay == 40 &&
         techtime_units == 40 && scheduled.payload.location == 2 &&
         scheduled.payload.position == 0 && scheduled.payload.player == 91;
}

int main(void) {
  if (!test_context_and_stall_policies())
    return 1;
  if (!test_parsing_and_rejection_guards())
    return 2;
  if (!test_noop_fatigue_and_resource_rejection())
    return 3;
  if (!test_extended_amounts_and_out_of_range_rejection())
    return 4;
  if (!test_armor_schedules_front_rear_partial_and_total_failure())
    return 5;
  return test_internal_schedules_and_failure_marker_type() ? 0 : 6;
}
