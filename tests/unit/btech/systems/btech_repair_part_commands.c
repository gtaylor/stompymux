#include "mech_tech_commands_api.h"

#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_tech_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "repair_job.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

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
static GameObject database_objects[16];
static GameDatabase test_database = {.object_storage = database_objects,
                                     .size = 15};
static char command_buffer[] = "ignored";
static TechPartParseResult part_parse;
static int gun_parse_status, gun_location, gun_position, gun_brand;
static bool started, starting, require_stall, section_destroyed, flooded;
static int destroyed_section, flooded_section;
static bool weapon, ammunition, actuator, structural, nonfunctional;
static bool destroyed, disabled, temporary_failure, damaged, valid_gun;
static bool repairing[NUM_SECTIONS][NUM_CRITICALS];
static bool scrapping[NUM_SECTIONS][NUM_CRITICALS];
static bool scrapping_location;
static int scrapping_section;
static int critical_type, current_brand, brand_sets, brand_set_value;
static int section_criticals, weapon_crits, technology_flags;
static SplitCriticalLookup split_lookup;
static int techtime, maximum_techtime, roll_values[2], roll_count;
static int weapon_roll_values[2], weapon_roll_count, last_difficulty;
static int inventory, inventory_part, inventory_brand, inventory_delta;
static int alias_result;
static int inventory_changes, resource_result, failure_result, success_result;
static int resource_calls, failure_calls, success_calls, techtime_units;
static int schedule_count, notifications, invalid_critical_reads;
static bool variable_time;
static int time_modifier;
static ScheduledEvent scheduled;

static int *int_at(int *values, size_t count, size_t index) {
  return checked_storage_at(values, count, sizeof(*values), index);
}

static bool *busy_at(bool values[NUM_SECTIONS][NUM_CRITICALS], int location,
                     int part) {
  bool *row = checked_storage_at(values, NUM_SECTIONS, sizeof(*values),
                                 (size_t)location);
  return checked_storage_at(row, NUM_CRITICALS, sizeof(*row), (size_t)part);
}

static void reset_state(void) {
  part_parse = (TechPartParseResult){
      .status = TECH_PART_PARSE_OK, .location = LARM, .position = 2};
  gun_parse_status = 0;
  gun_location = LARM;
  gun_position = 2;
  gun_brand = 0;
  started = starting = require_stall = section_destroyed = flooded = false;
  destroyed_section = flooded_section = -1;
  scrapping_section = -1;
  weapon = ammunition = actuator = structural = nonfunctional = destroyed =
      disabled = temporary_failure = damaged = false;
  valid_gun = true;
  critical_type = special_equipment_index(HEAT_SINK);
  current_brand = 7;
  brand_sets = brand_set_value = 0;
  section_criticals = NUM_CRITICALS;
  weapon_crits = 3;
  technology_flags = 0;
  split_lookup = (SplitCriticalLookup){0};
  techtime = 0;
  maximum_techtime = 1000;
  roll_values[0] = roll_values[1] = 0;
  weapon_roll_values[0] = weapon_roll_values[1] = 0;
  roll_count = weapon_roll_count = last_difficulty = 0;
  inventory = 5;
  alias_result = 600;
  inventory_part = inventory_brand = inventory_delta = inventory_changes = 0;
  resource_result = failure_result = 0;
  success_result = 0;
  resource_calls = failure_calls = success_calls = techtime_units = 0;
  schedule_count = notifications = invalid_critical_reads = 0;
  variable_time = false;
  time_modifier = 0;
  scheduled = (ScheduledEvent){0};
  memset(repairing, 0, sizeof(repairing));
  memset(scrapping, 0, sizeof(scrapping));
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
  return maximum_techtime;
}
bool btech_context_uses_variable_technology_time(const BtechContext *context
                                                 [[maybe_unused]]) {
  return variable_time;
}
int btech_context_technology_time_modifier(const BtechContext *context
                                           [[maybe_unused]]) {
  return time_modifier;
}
bool mech_is_dropship(const Mech *mech [[maybe_unused]]) { return false; }
bool mech_is_started(const Mech *mech [[maybe_unused]]) { return started; }
DbRef mech_repair_stall_dbref(const Mech *mech [[maybe_unused]]) { return 1; }
int mech_event_count(const Mech *mech [[maybe_unused]], MechEventType type) {
  return type == EVENT_STARTUP && starting;
}
DbRef mech_dbref(const Mech *mech [[maybe_unused]]) { return 4; }
DbRef mech_bay_dbref(const Mech *mech [[maybe_unused]],
                     int bay [[maybe_unused]]) {
  return 6;
}
UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }
int mech_technology_flags(const Mech *mech [[maybe_unused]]) {
  return technology_flags;
}
TechPartParseResult tech_part_parse(const TechPartParseRequest *request
                                    [[maybe_unused]]) {
  return part_parse;
}
int tech_parsegun(Mech *mech [[maybe_unused]], char *buffer [[maybe_unused]],
                  int *location, int *position, int *brand) {
  *location = gun_location;
  *position = gun_position;
  if (brand)
    *brand = gun_brand;
  return gun_parse_status;
}
int player_techtime(BtechContext *context [[maybe_unused]],
                    DbRef player [[maybe_unused]]) {
  return techtime;
}
int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
              int difficulty) {
  last_difficulty = difficulty;
  int index = roll_count++ == 0 ? 0 : 1;
  return *int_at(roll_values, 2, (size_t)index);
}
int tech_weapon_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                     int difficulty) {
  last_difficulty = difficulty;
  int index = weapon_roll_count++ == 0 ? 0 : 1;
  return *int_at(weapon_roll_values, 2, (size_t)index);
}
int tech_addtechtime(const TechTimeAddition *addition) {
  techtime_units = addition->units;
  return addition->units;
}
int tech_time_scaled_seconds(BtechContext *context [[maybe_unused]],
                             int units) {
  return units;
}
int tech_adjusted_time_for_roll(BtechContext *context [[maybe_unused]],
                                int base_units, int roll) {
  if (!variable_time || roll <= 0)
    return base_units;
  int reduction = time_modifier * roll;
  if (reduction < 0)
    reduction = 0;
  if (reduction > 100)
    reduction = 100;
  return max(1, (base_units * (100 - reduction)) / 100);
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int location [[maybe_unused]]) {
  return section_destroyed || location == destroyed_section;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int location [[maybe_unused]]) {
  return flooded || location == flooded_section;
}
int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int location [[maybe_unused]]) {
  return section_criticals;
}
int mech_critical_part_type(const Mech *mech [[maybe_unused]], int location,
                            int part) {
  if (split_lookup.found && location == split_lookup.slot.section &&
      part >= split_lookup.slot.critical)
    return special_equipment_index(SPLIT_CRIT_RIGHT);
  if (weapon)
    return WEAPON_BASE_INDEX + 1;
  if (ammunition)
    return AMMO_BASE_INDEX;
  if (actuator)
    return special_equipment_index(SHOULDER_OR_HIP);
  return critical_type;
}
int mech_critical_brand(const Mech *mech [[maybe_unused]],
                        int location [[maybe_unused]],
                        int part [[maybe_unused]]) {
  return current_brand;
}
void mech_critical_brand_set(const CriticalSlotBrandSet *set) {
  brand_sets++;
  brand_set_value = set->brand;
  current_brand = set->brand;
}
bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int location [[maybe_unused]],
                                    int part [[maybe_unused]]) {
  return nonfunctional;
}
bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]], int location,
                                int part) {
  if (location < 0 || location >= NUM_SECTIONS || part < 0 ||
      part >= section_criticals) {
    invalid_critical_reads++;
    return true;
  }
  if (split_lookup.found && location == split_lookup.slot.section &&
      part == split_lookup.slot.critical)
    return false;
  return destroyed;
}
bool mech_critical_is_disabled(const Mech *mech [[maybe_unused]],
                               int location [[maybe_unused]],
                               int part [[maybe_unused]]) {
  return disabled;
}
int mech_critical_temporary_failure(const Mech *mech [[maybe_unused]],
                                    int location [[maybe_unused]],
                                    int part [[maybe_unused]]) {
  return temporary_failure;
}
bool mech_critical_is_damaged(const Mech *mech [[maybe_unused]],
                              int location [[maybe_unused]],
                              int part [[maybe_unused]]) {
  return damaged;
}
bool mech_part_is_structural_placeholder(int part_type [[maybe_unused]]) {
  return structural;
}
bool mech_has_double_heat_sinks(const Mech *mech [[maybe_unused]]) {
  return false;
}
int mech_parts_alias(const Mech *mech [[maybe_unused]],
                     int part [[maybe_unused]]) {
  return alias_result;
}
int get_weapon_crits(Mech *mech [[maybe_unused]],
                     int weapon_index [[maybe_unused]]) {
  return weapon_crits;
}
int weapon_catalogue_critical_slots(int weapon_index [[maybe_unused]]) {
  return weapon_crits;
}
bool valid_gun_pos(const RepairCriticalSelection *selection [[maybe_unused]]) {
  return valid_gun;
}
SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  SplitCriticalLookup result = split_lookup;
  result.part_type = special_equipment_index(SPLIT_CRIT_RIGHT);
  return result;
}
int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int location [[maybe_unused]],
                       int part [[maybe_unused]]) {
  return gun_position;
}
int mech_weapon_first_critical(const WeaponCriticalSearch *search
                               [[maybe_unused]]) {
  return gun_position;
}
bool someone_repairing(Mech *mech [[maybe_unused]], int location, int part) {
  return location >= 0 && location < NUM_SECTIONS && part >= 0 &&
         part < NUM_CRITICALS && *busy_at(repairing, location, part);
}
bool someone_scrapping_part(Mech *mech [[maybe_unused]], int location,
                            int part) {
  return location >= 0 && location < NUM_SECTIONS && part >= 0 &&
         part < NUM_CRITICALS && *busy_at(scrapping, location, part);
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]], int location) {
  return scrapping_location || location == scrapping_section;
}
int econ_find_items(BtechContext *context [[maybe_unused]],
                    DbRef store [[maybe_unused]], int part, int brand) {
  inventory_part = part;
  inventory_brand = brand;
  return inventory;
}
void economy_inventory_change(const EconomyInventoryChange *change) {
  inventory_changes++;
  inventory_part = change->part.id;
  inventory_brand = change->part.brand;
  inventory_delta = change->quantity_delta;
  inventory += change->quantity_delta;
}
PartDisplayName part_name(BtechContext *context [[maybe_unused]],
                          int part [[maybe_unused]],
                          int brand [[maybe_unused]]) {
  return (PartDisplayName){.text = "test part"};
}
void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {
  notifications++;
}
void mecha_notifyf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {
  notifications++;
}
void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {
  notifications++;
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
void mux_event_tickmech_replacegun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairgun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairenhcrit(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairpart(MuxEvent *event [[maybe_unused]]) {}
int repair_econ(const RepairOperationCall *call [[maybe_unused]]) {
  resource_calls++;
  return resource_result;
}
int repairenhcrit_econ(const RepairOperationCall *call [[maybe_unused]]) {
  resource_calls++;
  return resource_result;
}
int repairg_fail(const RepairOperationCall *call [[maybe_unused]]) {
  failure_calls++;
  return failure_result;
}
int repairenhcrit_fail(const RepairOperationCall *call [[maybe_unused]]) {
  failure_calls++;
  return failure_result;
}
int repairp_fail(const RepairOperationCall *call [[maybe_unused]]) {
  failure_calls++;
  return failure_result;
}
int repairg_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_calls++;
  return success_result;
}
int repairenhcrit_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_calls++;
  return success_result;
}
int repairp_succ(const RepairOperationCall *call [[maybe_unused]]) {
  success_calls++;
  return success_result;
}
int max(int left, int right) { return left > right ? left : right; }
int min(int left, int right) { return left < right ? left : right; }

static bool untouched(void) {
  return schedule_count == 0 && resource_calls == 0 && inventory_changes == 0 &&
         roll_count == 0 && weapon_roll_count == 0;
}

static bool test_repairpart_guards_and_jobs(void) {
  reset_state();
  started = true;
  tech_repairpart(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  part_parse.status = TECH_PART_PARSE_INVALID_POSITION;
  tech_repairpart(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  temporary_failure = true;
  *busy_at(scrapping, LARM, 2) = true;
  tech_repairpart(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  temporary_failure = true;
  tech_repairpart(INT_MAX, test_mech, command_buffer);
  if (resource_calls != 1 || success_calls != 1 || schedule_count != 1 ||
      scheduled.type != EVENT_REPAIR_REPAP ||
      scheduled.callback != mux_event_tickmech_repairpart ||
      scheduled.delay != REPAIRPART_TIME ||
      scheduled.payload.player != INT_MAX ||
      scheduled.payload.location != LARM || scheduled.payload.position != 2)
    return false;
  reset_state();
  temporary_failure = true;
  roll_values[0] = -1;
  failure_result = -1;
  tech_repairpart(5, test_mech, command_buffer);
  return resource_calls == 1 && failure_calls == 1 && schedule_count == 1 &&
         scheduled.callback == mech_event_failure_marker &&
         scheduled.delay == (REPAIRPART_TIME * 3) / 2;
}

static bool test_replacepart_paths_and_payload(void) {
  reset_state();
  nonfunctional = true;
  *busy_at(scrapping, LARM, 2) = true;
  tech_replacepart(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  nonfunctional = true;
  variable_time = true;
  time_modifier = 10;
  roll_values[0] = 2;
  tech_replacepart(INT_MAX, test_mech, command_buffer);
  if (schedule_count != 1 ||
      scheduled.callback != mux_event_tickmech_repairpart ||
      scheduled.payload.player != INT_MAX || techtime_units != 36 ||
      inventory_changes != 1 || inventory_delta != -1 ||
      inventory_part != alias_result)
    return false;
  reset_state();
  nonfunctional = true;
  roll_values[0] = roll_values[1] = -1;
  tech_replacepart(5, test_mech, command_buffer);
  if (schedule_count != 1 || scheduled.callback != mech_event_failure_marker ||
      inventory_changes != 1 || techtime_units != (REPLACEPART_TIME * 3) / 2)
    return false;
  reset_state();
  ammunition = nonfunctional = true;
  tech_replacepart(5, test_mech, command_buffer);
  if (schedule_count != 1 || inventory_changes != 0)
    return false;
  reset_state();
  ammunition = nonfunctional = true;
  roll_values[0] = roll_values[1] = -1;
  tech_replacepart(5, test_mech, command_buffer);
  if (schedule_count != 1 || inventory_changes != 0)
    return false;
  reset_state();
  nonfunctional = true;
  roll_values[0] = -1;
  roll_values[1] = 1;
  tech_replacepart(5, test_mech, command_buffer);
  return schedule_count == 1 &&
         scheduled.callback == mech_event_failure_marker &&
         inventory_changes == 0 && techtime_units == (REPLACEPART_TIME * 3) / 2;
}

static bool test_replacegun_brand_conflicts_and_rolls(void) {
  reset_state();
  weapon = nonfunctional = true;
  gun_brand = 9;
  inventory = 0;
  tech_replacegun(5, test_mech, command_buffer);
  if (brand_sets != 0 || current_brand != 7 || inventory_brand != 9 ||
      !untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  weapon_crits = 4;
  *busy_at(scrapping, LARM, 4) = true;
  tech_replacegun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  gun_position = NUM_CRITICALS - 1;
  weapon_crits = 3;
  split_lookup = (SplitCriticalLookup){
      .found = true, .slot = {.section = RARM, .critical = 0}};
  *busy_at(scrapping, RARM, 1) = true;
  tech_replacegun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  gun_position = NUM_CRITICALS - 1;
  weapon_crits = 3;
  split_lookup = (SplitCriticalLookup){
      .found = true, .slot = {.section = RARM, .critical = 0}};
  flooded_section = RARM;
  tech_replacegun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  gun_position = NUM_CRITICALS - 1;
  weapon_crits = 3;
  split_lookup = (SplitCriticalLookup){
      .found = true, .slot = {.section = RARM, .critical = 0}};
  destroyed_section = RARM;
  tech_replacegun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  gun_position = NUM_CRITICALS - 1;
  weapon_crits = 3;
  split_lookup = (SplitCriticalLookup){
      .found = true, .slot = {.section = RARM, .critical = 0}};
  scrapping_section = RARM;
  tech_replacegun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = nonfunctional = true;
  gun_brand = 9;
  weapon_roll_values[0] = 1;
  variable_time = true;
  time_modifier = 10;
  tech_replacegun(INT_MAX, test_mech, command_buffer);
  if (schedule_count != 1 ||
      scheduled.callback != mux_event_tickmech_replacegun ||
      scheduled.payload.player != INT_MAX || scheduled.payload.extra != 9 ||
      techtime_units != 162 || inventory_brand != 9 || inventory_delta != -1 ||
      brand_sets != 0)
    return false;
  reset_state();
  weapon = nonfunctional = true;
  technology_flags = CLAN_TECH;
  weapon_crits = 3;
  tech_replacegun(5, test_mech, command_buffer);
  return techtime_units == 60 && schedule_count == 1;
}

static bool test_repairgun_split_and_job_paths(void) {
  reset_state();
  weapon = temporary_failure = true;
  weapon_crits = 4;
  *busy_at(repairing, LARM, 4) = true;
  tech_repairgun(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = destroyed = true;
  weapon_crits = 6;
  gun_position = NUM_CRITICALS - 1;
  split_lookup = (SplitCriticalLookup){
      .found = true, .slot = {.section = RARM, .critical = 0}};
  tech_repairgun(INT_MAX, test_mech, command_buffer);
  if (invalid_critical_reads != 0 || resource_calls != 1 ||
      schedule_count != 1 || scheduled.type != EVENT_REPAIR_REPAG ||
      scheduled.payload.player != INT_MAX ||
      scheduled.callback != mux_event_tickmech_repairgun ||
      techtime_units != REPAIRGUN_TIME)
    return false;
  reset_state();
  weapon = temporary_failure = true;
  weapon_roll_values[0] = -1;
  failure_result = -1;
  tech_repairgun(5, test_mech, command_buffer);
  return failure_calls == 1 &&
         scheduled.callback == mech_event_failure_marker &&
         techtime_units == (REPAIRGUN_TIME * 3) / 2;
}

static bool test_fixenhcrit_paths(void) {
  reset_state();
  weapon = damaged = true;
  *busy_at(scrapping, LARM, 3) = true;
  weapon_crits = 3;
  tech_fixenhcrit(5, test_mech, command_buffer);
  if (!untouched())
    return false;
  reset_state();
  weapon = damaged = true;
  tech_fixenhcrit(INT_MAX, test_mech, command_buffer);
  if (resource_calls != 1 || success_calls != 1 || schedule_count != 1 ||
      scheduled.type != EVENT_REPAIR_REPENHCRIT ||
      scheduled.callback != mux_event_tickmech_repairenhcrit ||
      scheduled.payload.player != INT_MAX ||
      techtime_units != REPAIRENHCRIT_TIME)
    return false;
  reset_state();
  weapon = damaged = true;
  resource_result = -1;
  tech_fixenhcrit(5, test_mech, command_buffer);
  return resource_calls == 1 && schedule_count == 0 && weapon_roll_count == 0;
}

int main(void) {
  if (!test_repairpart_guards_and_jobs())
    return 1;
  if (!test_replacepart_paths_and_payload())
    return 2;
  if (!test_replacegun_brand_conflicts_and_rolls())
    return 3;
  if (!test_repairgun_split_and_job_paths())
    return 4;
  if (!test_fixenhcrit_paths())
    return 5;
  return 0;
}
