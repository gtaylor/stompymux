#include "mech_tech_commands_api.h"

#include <stdbool.h>
#include <stdint.h>

#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "repair_job.h"

typedef struct TestEvent {
  MechEventType type;
  intptr_t data;
} TestEvent;

static TestEvent events[32];
static size_t event_count;
static int weapon_critical_positions[MAX_WEAPS_SECTION];
static int weapon_count;

int min(int left, int right) { return left < right ? left : right; }

static TestEvent *event_at(size_t index) {
  return checked_storage_at(events, sizeof(events) / sizeof(*events),
                            sizeof(*events), index);
}

static const TestEvent *event_at_const(size_t index) {
  return checked_storage_at_const(events, sizeof(events) / sizeof(*events),
                                  sizeof(*events), index);
}

static int *weapon_critical_position_at(size_t index) {
  return checked_storage_at(weapon_critical_positions,
                            sizeof(weapon_critical_positions) /
                                sizeof(*weapon_critical_positions),
                            sizeof(*weapon_critical_positions), index);
}

static void reset_test_state(void) {
  event_count = 0;
  weapon_count = 0;
}

static void add_event(MechEventType type, RepairEventPayload payload) {
  *event_at(event_count++) = (TestEvent){
      .type = type,
      .data = repair_event_payload_pack(payload),
  };
}

void mech_event_visit(Mech *mech [[maybe_unused]], MechEventType type,
                      MuxEventVisitor visitor, void *context) {
  for (size_t index = 0; index < event_count; index++) {
    const TestEvent *test_event = event_at_const(index);
    if (test_event->type != type)
      continue;
    MuxEvent event = {.data = mech,
                      .secondary = {.kind = MUX_EVENT_PAYLOAD_INTEGER,
                                    .integer = test_event->data}};
    visitor(&event, context);
  }
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }
int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return NUM_CRITICALS;
}
int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return weapon_equipment_index(1);
}
int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int section [[maybe_unused]],
                       int critical [[maybe_unused]]) {
  return 0;
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]]) {
  return false;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int section [[maybe_unused]]) {
  return false;
}
int get_weapon_crits(Mech *mech [[maybe_unused]],
                     int weapon_index [[maybe_unused]]) {
  return 5;
}
SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source) {
  if (source.section == RTORSO && source.critical == 10)
    return (SplitCriticalLookup){
        .found = true,
        .slot = {.section = RARM, .critical = 0},
    };
  return (SplitCriticalLookup){0};
}

int find_weapons_advanced(Mech *mech [[maybe_unused]],
                          int location [[maybe_unused]],
                          unsigned char *weapon_types [[maybe_unused]],
                          unsigned char *weapon_data [[maybe_unused]],
                          int *critical, int whine [[maybe_unused]]) {
  for (int index = 0; index < weapon_count; index++)
    *(int *)checked_storage_at(critical, MAX_WEAPS_SECTION, sizeof(*critical),
                               (size_t)index) =
        *weapon_critical_position_at((size_t)index);
  return weapon_count;
}

typedef struct SectionScrapBlocker {
  MechEventType type;
  int location;
} SectionScrapBlocker;

static bool test_all_pending_work_blocks_section_scrapping(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x1;
  const SectionScrapBlocker BLOCKERS[] = {
      {.type = EVENT_REPAIR_REPL, .location = 4},
      {.type = EVENT_REPAIR_REPLG, .location = 4},
      {.type = EVENT_REPAIR_REPAP, .location = 4},
      {.type = EVENT_REPAIR_REPAG, .location = 4},
      {.type = EVENT_REPAIR_MOB, .location = 4},
      {.type = EVENT_REPAIR_REPENHCRIT, .location = 4},
      {.type = EVENT_REPAIR_RELO, .location = 4},
      {.type = EVENT_REPAIR_FIXI, .location = 4},
      {.type = EVENT_REPAIR_REAT, .location = 4},
      {.type = EVENT_REPAIR_REPSUIT, .location = 4},
      {.type = EVENT_REPAIR_RESE, .location = 4},
      {.type = EVENT_REPAIR_SCRP, .location = 4},
      {.type = EVENT_REPAIR_SCRG, .location = 4},
      {.type = EVENT_REPAIR_UMOB, .location = 4},
      {.type = EVENT_REPAIR_FIX, .location = 4},
      {.type = EVENT_REPAIR_FIX, .location = 4 + NUM_SECTIONS},
  };

  for (size_t index = 0; index < (sizeof(BLOCKERS) / sizeof(*BLOCKERS));
       index++) {
    const SectionScrapBlocker *blocker =
        checked_storage_at_const(BLOCKERS, sizeof(BLOCKERS) / sizeof(*BLOCKERS),
                                 sizeof(*BLOCKERS), index);
    reset_test_state();
    add_event(blocker->type, (RepairEventPayload){.location = blocker->location,
                                                  .position = 2,
                                                  .extra = 2,
                                                  .player = 1073741824});
    if (can_scrap_loc(mech, 4) || !can_scrap_loc(mech, 5))
      return false;
  }
  return true;
}

static bool test_section_scrap_lookup_normalizes_rear_armor_locations(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x4;

  reset_test_state();
  add_event(EVENT_REPAIR_SCRL,
            (RepairEventPayload){.location = 2, .extra = 2, .player = 18});
  if (!someone_scrapping_loc(mech, 2) ||
      !someone_scrapping_loc(mech, 2 + NUM_SECTIONS))
    return false;

  return true;
}

static bool test_split_gun_blocks_both_sections(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x5;
  const MechEventType TYPES[] = {EVENT_REPAIR_SCRG, EVENT_REPAIR_REPLG,
                                 EVENT_REPAIR_REPAG};
  for (size_t index = 0; index < sizeof(TYPES) / sizeof(*TYPES); index++) {
    reset_test_state();
    add_event(
        *(const MechEventType *)checked_storage_at_const(
            TYPES, sizeof(TYPES) / sizeof(*TYPES), sizeof(*TYPES), index),
        (RepairEventPayload){.location = RTORSO, .position = 10, .extra = 2});
    if (can_scrap_loc(mech, RTORSO) || can_scrap_loc(mech, RARM) ||
        !can_scrap_loc(mech, LARM))
      return false;
  }
  return true;
}

static bool test_part_guards_are_location_and_position_specific(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x2;

  reset_test_state();
  add_event(EVENT_REPAIR_REPLG,
            (RepairEventPayload){.location = 2, .position = 3, .player = 99});
  add_event(EVENT_REPAIR_SCRP,
            (RepairEventPayload){.location = 2, .position = 4, .player = 99});
  if (!someone_repairing(mech, 2, 3) || someone_repairing(mech, 2, 4))
    return false;
  if (can_scrap_part(mech, 2, 3) || !can_scrap_part(mech, 2, 4))
    return false;
  if (!someone_scrapping_part(mech, 2, 4) || someone_scrapping_part(mech, 2, 3))
    return false;
  return true;
}

static bool test_weapon_selection_accepts_only_reported_criticals(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x3;

  reset_test_state();
  *weapon_critical_position_at(0) = 1;
  *weapon_critical_position_at(1) = 5;
  weapon_count = 2;
  if (!valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = 0, .position = 1}))
    return false;
  if (!valid_gun_pos(&(RepairCriticalSelection){
          .mech = mech, .location = 0, .position = 5}))
    return false;
  weapon_count = -1;
  return !valid_gun_pos(
      &(RepairCriticalSelection){.mech = mech, .location = 0, .position = 1});
}

int main(void) {
  return test_all_pending_work_blocks_section_scrapping() &&
                 test_section_scrap_lookup_normalizes_rear_armor_locations() &&
                 test_split_gun_blocks_both_sections() &&
                 test_part_guards_are_location_and_position_specific() &&
                 test_weapon_selection_accepts_only_reported_criticals()
             ? 0
             : 1;
}
