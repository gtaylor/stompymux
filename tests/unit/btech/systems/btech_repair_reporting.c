#include "mech_tech_repairs_api.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_event.h"
#include "coolmenu.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mux/objects/db.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"

enum {
  MAX_EVENTS = LAST_TECH_EVENT - FIRST_TECH_EVENT + 1,
  MAX_MENU_ENTRIES = MAX_EVENTS + 8,
};

static BtechContext *const TEST_CONTEXT = (BtechContext *)(uintptr_t)0x1;
static EvaluationContext *const TEST_EVALUATION =
    (EvaluationContext *)(uintptr_t)0x2;
static Mech *const TEST_MECH = (Mech *)(uintptr_t)0x3;
static GameObject database_objects[2];
static GameDatabase database;
static MuxEventScheduler scheduler;
static MuxEvent events[MAX_EVENTS];
static int event_count;
static int startup_event_count;
static int latest_tech_event;
static int repair_stall;
static int critical_count;
static bool started;
static bool dropship;
static bool limit_to_stalls;
static int abbreviation_calls;
static int part_name_calls;
static bool invalid_part_lookup;
static int notification_count;
static char notification[LBUF_SIZE];
static int shown_menu_count;
static int captured_entry_count;
static char captured_entries[MAX_MENU_ENTRIES][LBUF_SIZE];

static MuxEvent *event_at(int index) {
  return checked_storage_at(events, MAX_EVENTS, sizeof(*events), (size_t)index);
}

static char *captured_entry_at(int index) {
  return checked_storage_at(captured_entries, MAX_MENU_ENTRIES,
                            sizeof(*captured_entries), (size_t)index);
}

static void reset_test_state(void) {
  memset(database_objects, 0, sizeof(database_objects));
  database = (GameDatabase){.object_storage = database_objects, .size = 1};
  scheduler = (MuxEventScheduler){.tick = 100};
  memset(events, 0, sizeof(events));
  event_count = 0;
  startup_event_count = 0;
  latest_tech_event = 0;
  repair_stall = 1;
  critical_count = 4;
  started = false;
  dropship = false;
  limit_to_stalls = false;
  abbreviation_calls = 0;
  part_name_calls = 0;
  invalid_part_lookup = false;
  notification_count = 0;
  notification[0] = '\0';
  shown_menu_count = 0;
  captured_entry_count = 0;
  memset(captured_entries, 0, sizeof(captured_entries));
}

static bool entry_contains(const char *needle) {
  for (int index = 0; index < captured_entry_count; index++) {
    if (strstr(captured_entry_at(index), needle) != nullptr)
      return true;
  }
  return false;
}

static bool entries_are_bounded(void) {
  for (int index = 0; index < captured_entry_count; index++) {
    if (strnlen(captured_entry_at(index), LBUF_SIZE) == LBUF_SIZE)
      return false;
  }
  return true;
}

static void add_event(MechEventType type, MuxEventCallback callback,
                      RepairEventPayload payload) {
  if (event_count >= MAX_EVENTS)
    return;
  intptr_t encoded = repair_event_payload_pack(payload);
  *event_at(event_count++) = (MuxEvent){
      .type = (char)type,
      .function = callback,
      .data = TEST_MECH,
      .secondary = {.kind = MUX_EVENT_PAYLOAD_INTEGER, .integer = encoded},
      .tick = scheduler.tick + 120,
      .scheduler = &scheduler,
  };
}

static void normal_callback(MuxEvent *event [[maybe_unused]]) {}

void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return TEST_CONTEXT;
}

GameDatabase *btech_context_database(BtechContext *context [[maybe_unused]]) {
  return &database;
}

EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return TEST_EVALUATION;
}

bool btech_context_limits_repairs_to_stalls(const BtechContext *context
                                            [[maybe_unused]]) {
  return limit_to_stalls;
}

int mech_event_count(const Mech *mech [[maybe_unused]], MechEventType type) {
  return type == EVENT_STARTUP ? startup_event_count : 0;
}

void mech_event_visit(Mech *mech [[maybe_unused]], MechEventType type,
                      MuxEventVisitor visitor, void *context) {
  for (int index = 0; index < event_count; index++) {
    MuxEvent *event = event_at(index);
    if (event->type == type)
      visitor(event, context);
  }
}

bool mech_is_started(const Mech *mech [[maybe_unused]]) { return started; }

bool mech_is_dropship(const Mech *mech [[maybe_unused]]) { return dropship; }

DbRef mech_repair_stall_dbref(const Mech *mech [[maybe_unused]]) {
  return repair_stall;
}

int figure_latest_tech_event(Mech *mech [[maybe_unused]]) {
  return latest_tech_event;
}

int game_lag_time(BtechContext *context [[maybe_unused]], int duration) {
  return duration;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }

MechMovementType mech_movement_type(const Mech *mech [[maybe_unused]]) {
  return MOVE_BIPED;
}

int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return critical_count;
}

ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section) {
  abbreviation_calls++;
  if (section->location < 0 || section->location >= NUM_SECTIONS)
    return (ArmorSectionAbbreviation){.text = "BAD"};
  return (ArmorSectionAbbreviation){.text = "LT"};
}

PartDisplayName pos_part_name(Mech *mech [[maybe_unused]], int location,
                              int position) {
  part_name_calls++;
  if (location < 0 || location >= NUM_SECTIONS || position < 0 ||
      position >= critical_count)
    invalid_part_lookup = true;
  return (PartDisplayName){.text = "Long test component name", .valid = true};
}

MechDisplayId mech_display_id(Mech *mech [[maybe_unused]]) {
  return (MechDisplayId){.text = "TEST-UNIT"};
}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]], const char *message) {
  notification_count++;
  (void)snprintf(notification, sizeof(notification), "%s", message);
}

void cool_menu_entry_add(const CoolMenuEntryRequest *request) {
  if (request->text == nullptr || captured_entry_count >= MAX_MENU_ENTRIES)
    return;
  (void)snprintf(captured_entry_at(captured_entry_count), LBUF_SIZE, "%s",
                 request->text);
  captured_entry_count++;
}

void show_cool_menu(EvaluationContext *evaluation [[maybe_unused]],
                    DbRef player [[maybe_unused]],
                    CoolMenu *menu [[maybe_unused]]) {
  shown_menu_count++;
}

void kill_cool_menu(CoolMenu *menu [[maybe_unused]]) {}

static bool test_state_gates(void) {
  reset_test_state();
  startup_event_count = 1;
  tech_repairs(0, TEST_MECH, nullptr);
  if (notification_count != 1 ||
      strcmp(notification,
             "The mech's starting up! Please stop the sequence first.") != 0 ||
      shown_menu_count != 0)
    return false;

  reset_test_state();
  started = true;
  tech_repairs(0, TEST_MECH, nullptr);
  if (notification_count != 1 ||
      strcmp(notification,
             "The mech's started up ; please shut it down first.") != 0 ||
      shown_menu_count != 0)
    return false;

  reset_test_state();
  limit_to_stalls = true;
  repair_stall = 0;
  tech_repairs(0, TEST_MECH, nullptr);
  if (notification_count != 1 ||
      strcmp(notification, "The 'mech isn't in a repair stall!") != 0 ||
      shown_menu_count != 0)
    return false;

  reset_test_state();
  limit_to_stalls = true;
  repair_stall = 0;
  dropship = true;
  tech_repairs(0, TEST_MECH, nullptr);
  if (notification_count != 1 ||
      strcmp(notification, "This 'mech has no repairs pending!") != 0 ||
      shown_menu_count != 0)
    return false;

  reset_test_state();
  database_objects[1].has_wizard_flag = true;
  startup_event_count = 1;
  started = true;
  limit_to_stalls = true;
  repair_stall = 0;
  tech_repairs(0, TEST_MECH, nullptr);
  return notification_count == 1 &&
         strcmp(notification, "This 'mech has no repairs pending!") == 0 &&
         shown_menu_count == 0;
}

static bool test_all_descriptions_and_high_player(void) {
  reset_test_state();
  latest_tech_event = 1;
  const DbRef HIGH_PLAYER = 5000000000L;
  for (MechEventType type = FIRST_TECH_EVENT; type <= LAST_TECH_EVENT; type++) {
    const bool IS_REAR = type == EVENT_REPAIR_FIX;
    const int EXTRA = (type == EVENT_REPAIR_SCRL || type == EVENT_REPAIR_SCRP ||
                       type == EVENT_REPAIR_SCRG)
                          ? 2
                          : 1;
    add_event(type, normal_callback,
              (RepairEventPayload){.location = IS_REAR ? 10 : 2,
                                   .position = 1,
                                   .extra = EXTRA,
                                   .player = HIGH_PLAYER});
  }
  tech_repairs(0, TEST_MECH, nullptr);

  return notification_count == 0 && shown_menu_count == 1 &&
         entry_contains("5000000000") && entry_contains("Replacement of") &&
         entry_contains("Reattachment") && entry_contains("Unload of") &&
         entry_contains("Repair of armor") &&
         entry_contains("Repair of internals") && entry_contains("Removal") &&
         entry_contains("Scrapping of") &&
         entry_contains("Repair of Long test component name") &&
         entry_contains("Mounting of") && entry_contains("Removing of") &&
         entry_contains("Reseal") && entry_contains("Replacing suit") &&
         entry_contains("LT(R)") && entries_are_bounded();
}

static bool test_failure_markers(void) {
  reset_test_state();
  latest_tech_event = 1;
  MechEventType types[] = {
      EVENT_REPAIR_REPL,   EVENT_REPAIR_REPLG, EVENT_REPAIR_REAT,
      EVENT_REPAIR_RELO,   EVENT_REPAIR_FIX,   EVENT_REPAIR_FIXI,
      EVENT_REPAIR_REPAG,  EVENT_REPAIR_REPAP, EVENT_REPAIR_REPENHCRIT,
      EVENT_REPAIR_MOB,    EVENT_REPAIR_UMOB,  EVENT_REPAIR_RESE,
      EVENT_REPAIR_REPSUIT};
  for (size_t index = 0; index < sizeof(types) / sizeof(*types); index++) {
    MechEventType *type = checked_storage_at(
        types, sizeof(types) / sizeof(*types), sizeof(*types), index);
    const int POSITION =
        (*type == EVENT_REPAIR_FIX || *type == EVENT_REPAIR_FIXI) ? 0 : 1;
    add_event(
        *type, mech_event_failure_marker,
        (RepairEventPayload){
            .location = 2, .position = POSITION, .extra = 0, .player = 7});
  }
  tech_repairs(0, TEST_MECH, nullptr);

  int failures = 0;
  for (int index = 0; index < captured_entry_count; index++) {
    if (strstr(captured_entry_at(index), "(Failure)") != nullptr)
      failures++;
  }
  return shown_menu_count == 1 && failures == 11 &&
         entry_contains("Failed armor repair") &&
         entry_contains("Failed internal repair");
}

static bool test_invalid_payload_is_safe(void) {
  reset_test_state();
  latest_tech_event = 1;
  add_event(EVENT_REPAIR_REPL, normal_callback,
            (RepairEventPayload){.location = NUM_SECTIONS,
                                 .position = critical_count,
                                 .extra = 0,
                                 .player = 9});
  event_at(0)->scheduler = nullptr;
  tech_repairs(0, TEST_MECH, nullptr);
  return shown_menu_count == 1 && entry_contains("Invalid repair event data") &&
         abbreviation_calls == 0 && part_name_calls == 0 &&
         !invalid_part_lookup && entries_are_bounded();
}

static bool test_event_specific_location_validation(void) {
  reset_test_state();
  latest_tech_event = 1;
  MechEventType types[] = {EVENT_REPAIR_FIXI, EVENT_REPAIR_REAT,
                           EVENT_REPAIR_RESE, EVENT_REPAIR_REPSUIT};
  for (size_t index = 0; index < sizeof(types) / sizeof(*types); index++) {
    MechEventType *type = checked_storage_at(
        types, sizeof(types) / sizeof(*types), sizeof(*types), index);
    add_event(
        *type, normal_callback,
        (RepairEventPayload){
            .location = NUM_SECTIONS, .position = 1, .extra = 0, .player = 9});
  }
  tech_repairs(0, TEST_MECH, nullptr);

  int invalid_entries = 0;
  for (int index = 0; index < captured_entry_count; index++) {
    if (strstr(captured_entry_at(index), "Invalid repair event data") !=
        nullptr)
      invalid_entries++;
  }
  return shown_menu_count == 1 && invalid_entries == 4 &&
         abbreviation_calls == 0 && part_name_calls == 0 &&
         !invalid_part_lookup && entries_are_bounded();
}

static bool test_event_specific_amount_and_critical_validation(void) {
  reset_test_state();
  latest_tech_event = 1;
  add_event(EVENT_REPAIR_FIX, normal_callback,
            (RepairEventPayload){
                .location = 2, .position = 0, .extra = 0, .player = 9});
  add_event(EVENT_REPAIR_FIXI, normal_callback,
            (RepairEventPayload){
                .location = 2, .position = 0, .extra = 0, .player = 9});
  add_event(EVENT_REPAIR_SCRL, normal_callback,
            (RepairEventPayload){
                .location = 2, .position = 0, .extra = 0, .player = 9});
  add_event(
      EVENT_REPAIR_REPL, normal_callback,
      (RepairEventPayload){
          .location = 2, .position = critical_count, .extra = 0, .player = 9});
  tech_repairs(0, TEST_MECH, nullptr);

  int invalid_entries = 0;
  for (int index = 0; index < captured_entry_count; index++) {
    if (strstr(captured_entry_at(index), "Invalid repair event data") !=
        nullptr)
      invalid_entries++;
  }
  return shown_menu_count == 1 && invalid_entries == 4 &&
         abbreviation_calls == 0 && part_name_calls == 0 &&
         !invalid_part_lookup && entries_are_bounded();
}

static bool test_section_scrap_rear_location_is_invalid(void) {
  reset_test_state();
  latest_tech_event = 1;
  add_event(
      EVENT_REPAIR_SCRL, normal_callback,
      (RepairEventPayload){
          .location = NUM_SECTIONS, .position = 0, .extra = 1, .player = 9});
  tech_repairs(0, TEST_MECH, nullptr);
  return shown_menu_count == 1 && entry_contains("Invalid repair event data") &&
         abbreviation_calls == 0 && part_name_calls == 0 &&
         !invalid_part_lookup && entries_are_bounded();
}

static bool test_bomb_events_ignore_legacy_payload(void) {
  reset_test_state();
  latest_tech_event = 1;
  add_event(EVENT_REPAIR_MOB, normal_callback,
            (RepairEventPayload){
                .location = -1, .position = 0, .extra = 0, .player = 0});
  add_event(EVENT_REPAIR_UMOB, normal_callback,
            (RepairEventPayload){
                .location = -1, .position = 0, .extra = 0, .player = 0});
  tech_repairs(0, TEST_MECH, nullptr);
  return shown_menu_count == 1 && entry_contains("Mounting of a bomb") &&
         entry_contains("Removing of a bomb") && abbreviation_calls == 0 &&
         part_name_calls == 0 && !invalid_part_lookup && entries_are_bounded();
}

int main(void) {
  return test_state_gates() && test_all_descriptions_and_high_player() &&
                 test_failure_markers() && test_invalid_payload_is_safe() &&
                 test_event_specific_location_validation() &&
                 test_event_specific_amount_and_critical_validation() &&
                 test_section_scrap_rear_location_is_invalid() &&
                 test_bomb_events_ignore_legacy_payload()
             ? 0
             : 1;
}
