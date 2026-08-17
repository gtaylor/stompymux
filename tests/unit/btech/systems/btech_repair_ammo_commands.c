#include "mech_tech_commands_api.h"

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_tech_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

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
static GameObject database_objects[11];
static GameDatabase test_database = {.object_storage = database_objects,
                                     .size = 10};
static char test_buffer[] = "ignored";
static TechPartParseResult parse_result;
static bool parse_position;
static bool parse_extra;
static bool nonfunctional;
static bool disabled;
static bool section_destroyed;
static bool section_flooded;
static bool repairing;
static bool scrapping;
static bool wizard;
static bool in_character;
static bool started;
static bool startup;
static bool require_stall;
static int repair_stall;
static int ammo_data;
static int ammo_mode;
static int ammo_type;
static int full_ammo_amount;
static int valid_mode;
static int technology_time;
static int maximum_technology_time;
static int roll_result;
static int reload_resource_result;
static int reload_failure_result;
static int reload_success_result;
static int notify_count;
static int mech_notify_count;
static const char *last_notification;
static int data_sets;
static int mode_sets;
static int resource_count;
static int resource_ammo_mode;
static int failure_count;
static int success_count;
static int techtime_units;
static int scheduled_count;
static ScheduledEvent scheduled;

static void reset_test_state(void) {
  parse_result = (TechPartParseResult){
      .status = TECH_PART_PARSE_OK, .location = 2, .position = 3, .extra = 'L'};
  nonfunctional = false;
  disabled = false;
  section_destroyed = false;
  section_flooded = false;
  repairing = false;
  scrapping = false;
  wizard = false;
  in_character = false;
  started = false;
  startup = false;
  require_stall = false;
  repair_stall = 1;
  ammo_data = 4;
  ammo_mode = 0x40000000;
  ammo_type = AMMO_BASE_INDEX;
  full_ammo_amount = 10;
  valid_mode = 0x20;
  technology_time = 0;
  maximum_technology_time = 100;
  roll_result = 0;
  reload_resource_result = 0;
  reload_failure_result = -1;
  reload_success_result = 0;
  notify_count = 0;
  mech_notify_count = 0;
  last_notification = nullptr;
  data_sets = 0;
  mode_sets = 0;
  resource_count = 0;
  resource_ammo_mode = 0;
  failure_count = 0;
  success_count = 0;
  techtime_units = 0;
  scheduled_count = 0;
  scheduled = (ScheduledEvent){0};
  parse_position = false;
  parse_extra = false;
  memset(database_objects, 0, sizeof(database_objects));
  database_objects[8].has_wizard_flag = wizard;
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

bool game_object_has_flag(const ObjectFlagRequest *request) {
  return request->flag == OBJECT_FLAG_IN_CHARACTER && in_character;
}

DbRef mech_dbref(const Mech *mech [[maybe_unused]]) { return 9; }
bool mech_is_dropship(const Mech *mech [[maybe_unused]]) { return false; }
bool mech_is_started(const Mech *mech [[maybe_unused]]) { return started; }
DbRef mech_repair_stall_dbref(const Mech *mech [[maybe_unused]]) {
  return repair_stall;
}
int mech_event_count(const Mech *mech [[maybe_unused]], MechEventType type) {
  return type == EVENT_STARTUP && startup;
}

TechPartParseResult tech_part_parse(const TechPartParseRequest *request) {
  parse_position = request->parse_position;
  parse_extra = request->parse_extra;
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

int tech_time_scaled_seconds(BtechContext *context [[maybe_unused]],
                             int units) {
  return units;
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]], const char *message) {
  notify_count++;
  last_notification = message;
}

void mech_notify(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *message) {
  mech_notify_count++;
  last_notification = message;
}

int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int location [[maybe_unused]],
                            int part [[maybe_unused]]) {
  return ammo_type;
}

bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int location [[maybe_unused]],
                                    int part [[maybe_unused]]) {
  return nonfunctional;
}
bool mech_critical_is_disabled(const Mech *mech [[maybe_unused]],
                               int location [[maybe_unused]],
                               int part [[maybe_unused]]) {
  return disabled;
}
int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int location [[maybe_unused]],
                       int part [[maybe_unused]]) {
  return ammo_data;
}
void mech_critical_data_set(Mech *mech [[maybe_unused]],
                            int location [[maybe_unused]],
                            int part [[maybe_unused]], int data) {
  data_sets++;
  ammo_data = data;
}
int mech_critical_ammo_mode(const Mech *mech [[maybe_unused]],
                            int location [[maybe_unused]],
                            int part [[maybe_unused]]) {
  return ammo_mode;
}
void mech_critical_ammo_mode_set(Mech *mech [[maybe_unused]],
                                 int location [[maybe_unused]],
                                 int part [[maybe_unused]], int mode) {
  mode_sets++;
  ammo_mode = mode;
}
int full_ammo(const Mech *mech [[maybe_unused]], int location [[maybe_unused]],
              int part [[maybe_unused]]) {
  return full_ammo_amount;
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int location [[maybe_unused]]) {
  return section_destroyed;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int location [[maybe_unused]]) {
  return section_flooded;
}
bool someone_repairing(Mech *mech [[maybe_unused]],
                       int location [[maybe_unused]],
                       int part [[maybe_unused]]) {
  return repairing;
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]],
                          int location [[maybe_unused]]) {
  return scrapping;
}

int valid_ammo_mode(Mech *mech [[maybe_unused]], int location [[maybe_unused]],
                    int part [[maybe_unused]], int letter [[maybe_unused]]) {
  return valid_mode;
}
int reload_econ(const RepairOperationCall *call [[maybe_unused]]) {
  resource_count++;
  resource_ammo_mode = ammo_mode;
  return reload_resource_result;
}
int reload_fail(const RepairOperationCall *call [[maybe_unused]]) {
  failure_count++;
  return reload_failure_result;
}
int reload_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_count++;
  return reload_success_result;
}

void btech_context_event_schedule(BtechContext *context [[maybe_unused]],
                                  void *object [[maybe_unused]], int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data) {
  scheduled_count++;
  scheduled = (ScheduledEvent){.type = type,
                               .callback = callback,
                               .delay = delay,
                               .payload = repair_event_payload_unpack(data)};
}

void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reload(MuxEvent *event [[maybe_unused]]) {}

static bool notification_is(const char *message) {
  return last_notification && strcmp(last_notification, message) == 0;
}

static bool test_toggletype_paths(void) {
  reset_test_state();
  in_character = true;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is("This command only works in simpods!") || mode_sets)
    return false;

  reset_test_state();
  in_character = true;
  wizard = true;
  database_objects[8].has_wizard_flag = true;
  tech_toggletype(7, test_mech, test_buffer);
  if (mode_sets != 1 || data_sets != 1 || mech_notify_count != 1)
    return false;

  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is("Invalid section!") || mode_sets)
    return false;

  reset_test_state();
  ammo_type = WEAPON_BASE_INDEX;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is("That's no ammo!"))
    return false;

  reset_test_state();
  disabled = true;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is("The ammo compartment is nonfunctional!"))
    return false;

  reset_test_state();
  parse_result.extra = 0;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is(
          "You need to give a type to toggle to (use - for normal)"))
    return false;

  reset_test_state();
  valid_mode = -1;
  tech_toggletype(7, test_mech, test_buffer);
  if (!notification_is("That is invalid ammo type for this weapon!"))
    return false;

  reset_test_state();
  tech_toggletype(7, test_mech, test_buffer);
  return ammo_data == full_ammo_amount &&
         ammo_mode == ((0x40000000 & ~AMMO_MODES) | valid_mode) &&
         data_sets == 1 && mode_sets == 1 && mech_notify_count == 1 &&
         parse_position && parse_extra && notification_is("Ammo toggled.");
}

static bool test_reload_rejections_preserve_requested_change(void) {
  reset_test_state();
  started = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("The mech's started up ; please shut it down first."))
    return false;

  reset_test_state();
  startup = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is(
          "The mech's starting up! Please stop the sequence first."))
    return false;

  reset_test_state();
  require_stall = true;
  repair_stall = 0;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("The 'mech isn't in a repair stall!"))
    return false;

  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("Invalid section!"))
    return false;

  reset_test_state();
  ammo_type = WEAPON_BASE_INDEX;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("That's no ammo!"))
    return false;

  reset_test_state();
  nonfunctional = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is(
          "The ammo compartment is destroyed ; repair/replacepart it first."))
    return false;

  reset_test_state();
  disabled = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is(
          "The ammo compartment is disabled ; repair/replacepart it first."))
    return false;

  reset_test_state();
  ammo_data = full_ammo_amount;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is(
          "That particular ammo compartment doesn't need reloading."))
    return false;

  reset_test_state();
  repairing = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("Someone's playing with that part already!"))
    return false;

  reset_test_state();
  section_destroyed = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("That part's blown off! Use reattach first!"))
    return false;

  reset_test_state();
  section_flooded = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("That location has been flooded! Use reseal first!"))
    return false;

  reset_test_state();
  scrapping = true;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is(
          "Someone's scrapping that section - no repairs are possible!"))
    return false;

  reset_test_state();
  valid_mode = -1;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("That is invalid ammo type for this weapon!"))
    return false;

  reset_test_state();
  technology_time = maximum_technology_time;
  tech_reload(7, test_mech, test_buffer);
  if (!notification_is("You're too tired to do that!") || ammo_data != 4 ||
      ammo_mode != 0x40000000 || data_sets || mode_sets)
    return false;

  reset_test_state();
  reload_resource_result = -1;
  tech_reload(7, test_mech, test_buffer);
  return !scheduled_count && resource_count == 1 && ammo_data == 4 &&
         ammo_mode == 0x40000000 && data_sets == 2 && mode_sets == 2;
}

static bool test_reload_scheduling_and_ordering(void) {
  reset_test_state();
  parse_result.extra = 0;
  tech_reload(77, test_mech, test_buffer);
  if (resource_count != 1 || resource_ammo_mode != 0x40000000 ||
      success_count != 1 || failure_count || data_sets || mode_sets ||
      ammo_data != 4 || ammo_mode != 0x40000000 || scheduled_count != 1 ||
      scheduled.type != EVENT_REPAIR_RELO ||
      scheduled.callback != mux_event_tickmech_reload ||
      scheduled.delay != 10 || techtime_units != 10 ||
      scheduled.payload.location != 2 || scheduled.payload.position != 3 ||
      scheduled.payload.extra != 0 || scheduled.payload.player != 77 ||
      !parse_position || !parse_extra)
    return false;

  reset_test_state();
  tech_reload(77, test_mech, test_buffer);
  if (resource_count != 1 || success_count != 1 || failure_count ||
      notify_count != 1 ||
      !notification_is("You start reloading the ammo compartment..") ||
      scheduled_count != 1 || scheduled.type != EVENT_REPAIR_RELO ||
      scheduled.callback != mux_event_tickmech_reload ||
      scheduled.delay != RELOAD_TIME || techtime_units != RELOAD_TIME ||
      scheduled.payload.location != 2 || scheduled.payload.position != 3 ||
      scheduled.payload.extra != 0 || scheduled.payload.player != 77 ||
      ammo_data != 0 ||
      resource_ammo_mode != ((0x40000000 & ~AMMO_MODES) | valid_mode) ||
      ammo_mode != ((0x40000000 & ~AMMO_MODES) | valid_mode) ||
      !parse_position || !parse_extra)
    return false;

  reset_test_state();
  roll_result = -1;
  tech_reload(77, test_mech, test_buffer);
  return resource_count == 1 && failure_count == 1 && success_count == 0 &&
         ammo_data == 0 &&
         ammo_mode == ((0x40000000 & ~AMMO_MODES) | valid_mode) &&
         scheduled_count == 1 && scheduled.type == EVENT_REPAIR_RELO &&
         scheduled.callback == mech_event_failure_marker &&
         scheduled.delay == RELOAD_TIME * 3 / 2 &&
         techtime_units == RELOAD_TIME * 3 / 2 &&
         scheduled.payload.location == 2 && scheduled.payload.position == 3 &&
         scheduled.payload.extra == 0 && scheduled.payload.player == 77;
}

static bool test_unload_paths_and_payloads(void) {
  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID_POSITION;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("Invalid part!"))
    return false;

  reset_test_state();
  ammo_type = WEAPON_BASE_INDEX;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("That's no ammo!"))
    return false;

  reset_test_state();
  nonfunctional = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is(
          "The ammo compartment is destroyed ; repair/replacepart it first."))
    return false;

  reset_test_state();
  disabled = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is(
          "The ammo compartment is disabled ; repair/replacepart it first."))
    return false;

  reset_test_state();
  ammo_data = 0;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("That particular ammo compartment is empty already."))
    return false;

  reset_test_state();
  repairing = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("Someone's playing with that part already!"))
    return false;

  reset_test_state();
  section_destroyed = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("That part's blown off! Use reattach first!"))
    return false;

  reset_test_state();
  section_flooded = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("That location has been flooded! Use reseal first!"))
    return false;

  reset_test_state();
  scrapping = true;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is(
          "Someone's scrapping that section - no repairs are possible!"))
    return false;

  reset_test_state();
  technology_time = maximum_technology_time;
  tech_unload(7, test_mech, test_buffer);
  if (!notification_is("You're too tired to do that!") || scheduled_count)
    return false;

  reset_test_state();
  ammo_data = full_ammo_amount;
  tech_unload(17, test_mech, test_buffer);
  if (!notification_is("You start unloading the ammo compartment..") ||
      scheduled_count != 1 || scheduled.type != EVENT_REPAIR_RELO ||
      scheduled.callback != mux_event_tickmech_reload ||
      scheduled.delay != RELOAD_TIME || techtime_units != RELOAD_TIME ||
      scheduled.payload.location != 2 || scheduled.payload.position != 3 ||
      scheduled.payload.extra != 2 || scheduled.payload.player != 17 ||
      !parse_position || parse_extra)
    return false;

  reset_test_state();
  ammo_data = full_ammo_amount - 1;
  roll_result = -1;
  tech_unload(17, test_mech, test_buffer);
  return scheduled_count == 1 &&
         scheduled.callback == mux_event_tickmech_reload &&
         scheduled.delay == RELOAD_TIME * 3 / 2 &&
         techtime_units == RELOAD_TIME * 3 / 2 && scheduled.payload.extra == 1;
}

int main(void) {
  if (!test_toggletype_paths())
    return 1;
  if (!test_reload_rejections_preserve_requested_change())
    return 2;
  if (!test_reload_scheduling_and_ordering())
    return 3;
  return test_unload_paths_and_payloads() ? 0 : 4;
}
