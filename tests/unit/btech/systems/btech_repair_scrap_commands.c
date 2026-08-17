#include "mech_tech_commands_api.h"

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"

typedef struct ScheduledEvent {
  int type;
  MuxEventCallback callback;
  int delay;
  RepairEventPayload payload;
} ScheduledEvent;

static BtechContext *const test_context = (BtechContext *)(uintptr_t)1;
static Mech *const test_mech = (Mech *)(uintptr_t)2;
static EvaluationContext *const test_evaluation =
    (EvaluationContext *)(uintptr_t)3;
static GameObject database_objects[8];
static GameDatabase test_database = {.object_storage = database_objects,
                                     .size = 7};
static char test_buffer[] = "ignored";
static TechPartParseResult parse_result;
static int gun_parse_result, gun_location, gun_position;
static UnitClass unit_class;
static bool started, starting, require_stall, section_destroyed[NUM_SECTIONS];
static bool critical_destroyed, structural_placeholder, valid_position;
static int destroyed_critical_location, destroyed_critical_position;
static bool scrapping_part, scrapping_location, scrap_part_allowed;
static bool scrap_location_allowed;
static int scrapping_part_location, scrapping_part_position;
static int repair_conflict_location, repair_conflict_position;
static int scrapping_section;
static bool split_found;
static CriticalSlotReference split_slot;
static int repair_stall, critical_type, technology_flags, weapon_crits;
static int technology_time, maximum_technology_time;
static int roll_results[2], roll_count, weapon_roll_count;
static int last_roll_difficulty, last_weapon_roll_difficulty;
static int notifications, techtime_units, schedule_count;
static ScheduledEvent scheduled;

static int *roll_result_at(int index) {
  return checked_storage_at(roll_results, 2, sizeof(*roll_results),
                            (size_t)index);
}

static bool *destroyed_at(int location) {
  return checked_storage_at(section_destroyed, NUM_SECTIONS,
                            sizeof(*section_destroyed), (size_t)location);
}

static void reset_test_state(void) {
  parse_result = (TechPartParseResult){
      .status = TECH_PART_PARSE_OK, .location = LARM, .position = 2};
  gun_parse_result = 0;
  gun_location = LARM;
  gun_position = 2;
  unit_class = CLASS_MECH;
  started = starting = require_stall = critical_destroyed =
      structural_placeholder = scrapping_part = scrapping_location = false;
  valid_position = scrap_part_allowed = scrap_location_allowed = true;
  destroyed_critical_location = destroyed_critical_position = -1;
  scrapping_part_location = scrapping_part_position = repair_conflict_location =
      repair_conflict_position = scrapping_section = -1;
  split_found = false;
  split_slot = (CriticalSlotReference){0};
  repair_stall = 1;
  critical_type = special_equipment_index(HEAT_SINK);
  technology_flags = 0;
  weapon_crits = 3;
  technology_time = 0;
  maximum_technology_time = 1000;
  roll_results[0] = roll_results[1] = 0;
  roll_count = weapon_roll_count = notifications = techtime_units =
      schedule_count = 0;
  last_roll_difficulty = last_weapon_roll_difficulty = 0;
  scheduled = (ScheduledEvent){0};
  memset(section_destroyed, 0, sizeof(section_destroyed));
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
UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return unit_class; }
TechPartParseResult tech_part_parse(const TechPartParseRequest *request
                                    [[maybe_unused]]) {
  return parse_result;
}
int tech_parsegun(Mech *mech [[maybe_unused]], char *buffer [[maybe_unused]],
                  int *location, int *position, int *brand [[maybe_unused]]) {
  *location = gun_location;
  *position = gun_position;
  return gun_parse_result;
}
int player_techtime(BtechContext *context [[maybe_unused]],
                    DbRef player [[maybe_unused]]) {
  return technology_time;
}
int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
              int difficulty) {
  last_roll_difficulty = difficulty;
  int index = roll_count < 2 ? roll_count : 1;
  roll_count++;
  return *roll_result_at(index);
}
int tech_weapon_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                     int difficulty) {
  last_weapon_roll_difficulty = difficulty;
  int index = weapon_roll_count < 2 ? weapon_roll_count : 1;
  weapon_roll_count++;
  return *roll_result_at(index);
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
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {
  notifications++;
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int location) {
  return location >= 0 && location < NUM_SECTIONS ? *destroyed_at(location)
                                                  : false;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int location [[maybe_unused]]) {
  return false;
}
int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int location [[maybe_unused]],
                            int part [[maybe_unused]]) {
  return critical_type;
}
int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int location [[maybe_unused]],
                       int part [[maybe_unused]]) {
  return 0;
}
bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]], int location,
                                int part) {
  return critical_destroyed || (location == destroyed_critical_location &&
                                part == destroyed_critical_position);
}
bool mech_part_is_structural_placeholder(int part_type) {
  return structural_placeholder ||
         part_type == special_equipment_index(SPLIT_CRIT_LEFT) ||
         part_type == special_equipment_index(SPLIT_CRIT_RIGHT);
}
bool valid_gun_pos(const RepairCriticalSelection *selection [[maybe_unused]]) {
  return valid_position;
}
bool someone_scrapping_part(Mech *mech [[maybe_unused]], int location,
                            int part) {
  return scrapping_part || (location == scrapping_part_location &&
                            part == scrapping_part_position);
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]], int location) {
  return scrapping_location || location == scrapping_section;
}
bool can_scrap_part(Mech *mech [[maybe_unused]], int location, int part) {
  return scrap_part_allowed && !(location == repair_conflict_location &&
                                 part == repair_conflict_position);
}
bool can_scrap_loc(Mech *mech [[maybe_unused]], int location [[maybe_unused]]) {
  return scrap_location_allowed;
}
int mech_technology_flags(const Mech *mech [[maybe_unused]]) {
  return technology_flags;
}
int get_weapon_crits(Mech *mech [[maybe_unused]], int weapon [[maybe_unused]]) {
  return weapon_crits;
}
int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int location [[maybe_unused]]) {
  return NUM_CRITICALS;
}
SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  return (SplitCriticalLookup){.found = split_found, .slot = split_slot};
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
void mux_event_tickmech_removegun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_removepart(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_removesection(MuxEvent *event [[maybe_unused]]) {}
int max(int left, int right) { return left > right ? left : right; }
int min(int left, int right) { return left < right ? left : right; }

static bool untouched(void) {
  return schedule_count == 0 && techtime_units == 0 && roll_count == 0 &&
         weapon_roll_count == 0;
}

static bool test_context_parse_and_common_guards(void) {
  reset_test_state();
  started = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID_POSITION;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  gun_parse_result = -1;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  technology_time = maximum_technology_time;
  tech_removesection(4, test_mech, test_buffer);
  return untouched();
}

static bool test_part_rejection_matrix(void) {
  reset_test_state();
  critical_type = EMPTY;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  section_destroyed[LARM] = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_destroyed = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  structural_placeholder = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = special_equipment_index(SPLIT_CRIT_LEFT);
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  const int prohibited[] = {ENDO_STEEL, FERRO_FIBROUS, STEALTH_ARMOR,
                            HVY_FERRO_FIBROUS, LT_FERRO_FIBROUS};
  for (size_t index = 0; index < sizeof(prohibited) / sizeof(*prohibited);
       index++) {
    reset_test_state();
    const int *part = checked_storage_at_const(
        prohibited, sizeof(prohibited) / sizeof(*prohibited),
        sizeof(*prohibited), index);
    critical_type = special_equipment_index(*part);
    tech_removepart(4, test_mech, test_buffer);
    if (!untouched())
      return false;
  }
  reset_test_state();
  scrapping_part = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  scrapping_location = true;
  tech_removepart(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  scrap_part_allowed = false;
  tech_removepart(4, test_mech, test_buffer);
  return untouched();
}

static bool test_gun_rejections(void) {
  reset_test_state();
  section_destroyed[LARM] = true;
  critical_type = weapon_equipment_index(0);
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  critical_destroyed = true;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  valid_position = false;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  gun_position = 10;
  weapon_crits = 5;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  destroyed_critical_location = LARM;
  destroyed_critical_position = 4;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  scrapping_part_location = LARM;
  scrapping_part_position = 4;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  repair_conflict_location = LARM;
  repair_conflict_position = 4;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  gun_position = 10;
  weapon_crits = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = RTORSO, .critical = 4};
  scrapping_part_location = RTORSO;
  scrapping_part_position = 6;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  gun_position = 10;
  weapon_crits = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = RTORSO, .critical = 4};
  destroyed_critical_location = RTORSO;
  destroyed_critical_position = 6;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  gun_position = 10;
  weapon_crits = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = RTORSO, .critical = 4};
  scrapping_section = RTORSO;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  scrapping_part = true;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  scrap_part_allowed = false;
  tech_removegun(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  scrapping_location = true;
  tech_removegun(4, test_mech, test_buffer);
  return untouched();
}

static bool test_part_roll_branches(void) {
  reset_test_state();
  tech_removepart(999999, test_mech, test_buffer);
  if (roll_count != 1 || last_roll_difficulty != REMOVEP_DIFFICULTY ||
      scheduled.type != EVENT_REPAIR_SCRP ||
      scheduled.callback != mux_event_tickmech_removepart ||
      techtime_units != 40 || scheduled.delay != 40 ||
      scheduled.payload.location != LARM || scheduled.payload.position != 2 ||
      scheduled.payload.extra != 2 || scheduled.payload.player != 999999)
    return false;
  reset_test_state();
  roll_results[0] = -1;
  roll_results[1] = 0;
  tech_removepart(17, test_mech, test_buffer);
  if (roll_count != 2 || scheduled.callback != mux_event_tickmech_removepart ||
      techtime_units != 40 || scheduled.payload.extra != 2)
    return false;
  reset_test_state();
  roll_results[0] = roll_results[1] = -1;
  tech_removepart(17, test_mech, test_buffer);
  return roll_count == 2 && scheduled.callback == mech_event_failure_marker &&
         techtime_units == 60 && scheduled.delay == 60 &&
         scheduled.payload.extra == 3 && scheduled.payload.player == 17;
}

static bool test_gun_rolls_and_clan_timing(void) {
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  weapon_crits = 3;
  tech_removegun(999999, test_mech, test_buffer);
  if (weapon_roll_count != 1 ||
      last_weapon_roll_difficulty != REMOVEG_DIFFICULTY ||
      scheduled.type != EVENT_REPAIR_SCRG ||
      scheduled.callback != mux_event_tickmech_removegun ||
      techtime_units != 120 || scheduled.delay != 120 ||
      scheduled.payload.location != LARM || scheduled.payload.position != 2 ||
      scheduled.payload.extra != 2 || scheduled.payload.player != 999999)
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  technology_flags = CLAN_TECH;
  weapon_crits = 3;
  tech_removegun(17, test_mech, test_buffer);
  if (techtime_units != 40 || scheduled.delay != 40)
    return false;
  const int clan_crits[] = {1, 2, 3, 5};
  const int clan_times[] = {40, 40, 40, 80};
  for (size_t index = 0; index < sizeof(clan_crits) / sizeof(*clan_crits);
       index++) {
    const int *crits = checked_storage_at_const(
        clan_crits, sizeof(clan_crits) / sizeof(*clan_crits),
        sizeof(*clan_crits), index);
    const int *expected = checked_storage_at_const(
        clan_times, sizeof(clan_times) / sizeof(*clan_times),
        sizeof(*clan_times), index);
    reset_test_state();
    critical_type = weapon_equipment_index(0);
    technology_flags = CLAN_TECH;
    weapon_crits = *crits;
    tech_removegun(17, test_mech, test_buffer);
    if (techtime_units != *expected || scheduled.delay != *expected)
      return false;
  }
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  roll_results[0] = -1;
  roll_results[1] = 0;
  tech_removegun(17, test_mech, test_buffer);
  if (weapon_roll_count != 2 ||
      scheduled.callback != mux_event_tickmech_removegun ||
      scheduled.payload.extra != 2)
    return false;
  reset_test_state();
  critical_type = weapon_equipment_index(0);
  roll_results[0] = roll_results[1] = -1;
  tech_removegun(17, test_mech, test_buffer);
  return weapon_roll_count == 2 &&
         scheduled.callback == mech_event_failure_marker &&
         techtime_units == 180 && scheduled.delay == 180 &&
         scheduled.payload.extra == 3;
}

static bool test_scrap_path_matrix(void) {
  reset_test_state();
  if (!invalid_scrap_path(test_mech, CTORSO) ||
      !invalid_scrap_path(test_mech, LTORSO) ||
      !invalid_scrap_path(test_mech, RTORSO) ||
      invalid_scrap_path(test_mech, LARM) || invalid_scrap_path(test_mech, -1))
    return false;
  section_destroyed[HEAD] = section_destroyed[LTORSO] =
      section_destroyed[RTORSO] = section_destroyed[LARM] =
          section_destroyed[RARM] = true;
  if (invalid_scrap_path(test_mech, CTORSO) ||
      invalid_scrap_path(test_mech, LTORSO) ||
      invalid_scrap_path(test_mech, RTORSO))
    return false;
  reset_test_state();
  unit_class = CLASS_VEH_GROUND;
  return !invalid_scrap_path(test_mech, CTORSO);
}

static bool test_section_guards_and_multipliers(void) {
  reset_test_state();
  section_destroyed[LARM] = true;
  tech_removesection(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  parse_result.location = LTORSO;
  tech_removesection(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  scrapping_location = true;
  tech_removesection(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  scrap_location_allowed = false;
  tech_removesection(4, test_mech, test_buffer);
  if (!untouched())
    return false;
  reset_test_state();
  tech_removesection(999999, test_mech, test_buffer);
  if (roll_count != 1 || last_roll_difficulty != REMOVES_DIFFICULTY ||
      scheduled.type != EVENT_REPAIR_SCRL ||
      scheduled.callback != mux_event_tickmech_removesection ||
      techtime_units != 120 || scheduled.delay != 120 ||
      scheduled.payload.location != LARM || scheduled.payload.position != 0 ||
      scheduled.payload.extra != 2 || scheduled.payload.player != 999999)
    return false;
  reset_test_state();
  roll_results[0] = -1;
  tech_removesection(17, test_mech, test_buffer);
  return roll_count == 1 && techtime_units == 180 && scheduled.delay == 180 &&
         scheduled.payload.extra == 3;
}

int main(void) {
  if (!test_context_parse_and_common_guards())
    return 1;
  if (!test_part_rejection_matrix())
    return 2;
  if (!test_gun_rejections())
    return 3;
  if (!test_part_roll_branches())
    return 4;
  if (!test_gun_rolls_and_clan_timing())
    return 5;
  if (!test_scrap_path_matrix())
    return 6;
  return test_section_guards_and_multipliers() ? 0 : 7;
}
