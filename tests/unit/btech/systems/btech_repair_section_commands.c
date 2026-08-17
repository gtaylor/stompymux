#include "mech_tech_commands_api.h"

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "bsuit_api.h"
#include "btech/context.h"
#include "command_handlers_api.h"
#include "econ_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_do_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/network/network_output.h"
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
static GameObject database_objects[10];
static GameDatabase test_database = {.object_storage = database_objects,
                                     .size = 9};
static char test_buffer[] = "ignored";
static TechPartParseResult parse_result;
static UnitClass unit_class;
static bool started, starting, require_stall, destroyed[NUM_SECTIONS];
static bool flooded[NUM_SECTIONS], attaching, replacing, resealing, scrapping;
static int repair_stall, internals[NUM_SECTIONS], originals[NUM_SECTIONS];
static int inventory_internal, inventory_electrics, technology_time, max_time;
static int roll_results[2], roll_count, last_roll_difficulty, notifications,
    inventory_changes;
static bool variable_technology_time;
static int technology_time_modifier;
static int fixextra_calls, magic_calls, int_check_calls, techtime_units;
static ScheduledEvent scheduled;

static bool *destroyed_at(int location) {
  return checked_storage_at(destroyed, NUM_SECTIONS, sizeof(*destroyed),
                            (size_t)location);
}
static bool *flooded_at(int location) {
  return checked_storage_at(flooded, NUM_SECTIONS, sizeof(*flooded),
                            (size_t)location);
}
static int *internal_at(int location) {
  return checked_storage_at(internals, NUM_SECTIONS, sizeof(*internals),
                            (size_t)location);
}
static int *original_at(int location) {
  return checked_storage_at(originals, NUM_SECTIONS, sizeof(*originals),
                            (size_t)location);
}

static void reset_test_state(void) {
  parse_result =
      (TechPartParseResult){.status = TECH_PART_PARSE_OK, .location = RARM};
  unit_class = CLASS_MECH;
  started = starting = require_stall = attaching = replacing = resealing =
      scrapping = false;
  repair_stall = 1;
  for (int i = 0; i < NUM_SECTIONS; i++) {
    *destroyed_at(i) = *flooded_at(i) = false;
    *internal_at(i) = *original_at(i) = 5;
  }
  inventory_internal = inventory_electrics = 30;
  technology_time = 0;
  max_time = 1000;
  variable_technology_time = false;
  technology_time_modifier = 0;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 0) = 0;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 1) = 0;
  roll_count = notifications = inventory_changes = fixextra_calls =
      magic_calls = int_check_calls = techtime_units = last_roll_difficulty = 0;
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
bool btech_context_uses_variable_technology_time(const BtechContext *context
                                                 [[maybe_unused]]) {
  return variable_technology_time;
}
int btech_context_technology_time_modifier(const BtechContext *context
                                           [[maybe_unused]]) {
  return technology_time_modifier;
}
int btech_context_maximum_technology_time(const BtechContext *context
                                          [[maybe_unused]]) {
  return max_time;
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
int player_techtime(BtechContext *context [[maybe_unused]],
                    DbRef player [[maybe_unused]]) {
  return technology_time;
}
int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
              int difficulty) {
  last_roll_difficulty = difficulty;
  int index = roll_count < 2 ? roll_count : 1;
  roll_count++;
  return *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results),
                                    (size_t)index);
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
  if (!variable_technology_time || roll <= 0)
    return base_units;
  int reduction = technology_time_modifier * roll;
  if (reduction > 100)
    reduction = 100;
  return max(1, (base_units * (100 - reduction)) / 100);
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]], int loc) {
  return loc >= 0 && loc < NUM_SECTIONS && *destroyed_at(loc);
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]], int loc) {
  return loc >= 0 && loc < NUM_SECTIONS && *flooded_at(loc);
}
int mech_section_original_internal(const Mech *mech [[maybe_unused]], int loc) {
  return *original_at(loc);
}
int mech_section_internal(const Mech *mech [[maybe_unused]], int loc) {
  return *internal_at(loc);
}
int someone_attaching(Mech *mech [[maybe_unused]], int loc [[maybe_unused]]) {
  return attaching;
}
int someone_replacing_suit(Mech *mech [[maybe_unused]],
                           int loc [[maybe_unused]]) {
  return replacing;
}
int someone_resealing(Mech *mech [[maybe_unused]], int loc [[maybe_unused]]) {
  return resealing;
}
int someone_scrapping_loc(Mech *mech [[maybe_unused]],
                          int loc [[maybe_unused]]) {
  return scrapping;
}
int bsuit_member_count(const Mech *mech [[maybe_unused]]) {
  int count = 0;
  for (int i = 0; i < mech_maximum_battle_suits(mech); i++)
    count += !*destroyed_at(i);
  return count;
}
int mech_maximum_battle_suits(const Mech *mech [[maybe_unused]]) { return 4; }
const char *bsuit_formation_name_lowercase(const Mech *mech [[maybe_unused]]) {
  return "squad";
}
DbRef mech_parts_store_dbref(const Mech *mech [[maybe_unused]]) { return 5; }
int mech_parts_alias(const Mech *mech [[maybe_unused]], int part) {
  return part;
}
int tech_proper_internal_part(const Mech *mech [[maybe_unused]]) {
  return cargo_equipment_index(S_INTERNAL);
}
void mech_parts_add(Mech *mech [[maybe_unused]], int part [[maybe_unused]],
                    int brand [[maybe_unused]], int count [[maybe_unused]]) {}
int econ_find_items(BtechContext *context [[maybe_unused]],
                    DbRef store [[maybe_unused]], int id,
                    int brand [[maybe_unused]]) {
  return id == cargo_equipment_index(S_INTERNAL) ? inventory_internal
                                                 : inventory_electrics;
}
void economy_inventory_change(const EconomyInventoryChange *change) {
  inventory_changes++;
  if (change->part.id == cargo_equipment_index(S_INTERNAL))
    inventory_internal += change->quantity_delta;
  else
    inventory_electrics += change->quantity_delta;
}
PartDisplayName part_name(BtechContext *context [[maybe_unused]],
                          int part [[maybe_unused]],
                          int brand [[maybe_unused]]) {
  return (PartDisplayName){.text = "Internal"};
}
bool mech_parts_consume(Mech *mech [[maybe_unused]],
                        DbRef player [[maybe_unused]],
                        const MechPartRequirement requirements[],
                        size_t count) {
  for (size_t i = 0; i < count; i++) {
    const MechPartRequirement *requirement =
        checked_storage_at_const(requirements, count, sizeof(*requirements), i);
    if (requirement->part == cargo_equipment_index(S_INTERNAL) &&
        inventory_internal < requirement->count)
      return false;
    if (requirement->part != cargo_equipment_index(S_INTERNAL) &&
        inventory_electrics < requirement->count)
      return false;
  }
  for (size_t i = 0; i < count; i++) {
    const MechPartRequirement *requirement =
        checked_storage_at_const(requirements, count, sizeof(*requirements), i);
    if (requirement->part == cargo_equipment_index(S_INTERNAL))
      inventory_internal -= requirement->count;
    else
      inventory_electrics -= requirement->count;
  }
  return true;
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
  scheduled = (ScheduledEvent){.type = type,
                               .callback = callback,
                               .delay = delay,
                               .payload = repair_event_payload_unpack(data)};
}
void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reattach(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_replacesuit(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reseal(MuxEvent *event [[maybe_unused]]) {}
void do_fixextra(Mech *mech [[maybe_unused]]) { fixextra_calls++; }
void do_magic(Mech *mech [[maybe_unused]]) { magic_calls++; }
void mech_int_check(Mech *mech [[maybe_unused]], int check [[maybe_unused]]) {
  int_check_calls++;
}
int btech_random_range_int(BtechContext *context [[maybe_unused]], int low,
                           int high [[maybe_unused]]) {
  return low;
}
int max(int left, int right) { return left > right ? left : right; }
int mech_technology_flags(const Mech *mech [[maybe_unused]]) { return 0; }
int mech_technology_flags_secondary(const Mech *mech [[maybe_unused]]) {
  return 0;
}
int mech_infantry_technology_flags(const Mech *mech [[maybe_unused]]) {
  return 0;
}

static bool test_path_and_fixability(void) {
  reset_test_state();
  *destroyed_at(RTORSO) = true;
  if (!invalid_repair_path(test_mech, RARM) ||
      invalid_repair_path(test_mech, LLEG) ||
      invalid_repair_path(test_mech, CTORSO))
    return false;
  *destroyed_at(CTORSO) = true;
  if (!invalid_repair_path(test_mech, LLEG) ||
      invalid_repair_path(test_mech, CTORSO))
    return false;
  unit_class = CLASS_VTOL;
  if (invalid_repair_path(test_mech, RARM))
    return false;
  reset_test_state();
  *destroyed_at(CTORSO) = true;
  if (unit_is_fixable(test_mech))
    return false;
  unit_class = CLASS_VTOL;
  if (unit_is_fixable(test_mech))
    return false;
  reset_test_state();
  unit_class = CLASS_VTOL;
  *destroyed_at(TURRET) = true;
  if (unit_is_fixable(test_mech))
    return false;
  reset_test_state();
  unit_class = CLASS_VTOL;
  *destroyed_at(ROTOR) = true;
  if (!unit_is_fixable(test_mech))
    return false;
  reset_test_state();
  unit_class = CLASS_VEH_GROUND;
  *destroyed_at(TURRET) = true;
  return unit_is_fixable(test_mech);
}

static bool test_reattach_rejections_and_rolls(void) {
  reset_test_state();
  parse_result.status = TECH_PART_PARSE_INVALID;
  tech_reattach(7, test_mech, test_buffer);
  if (notifications != 1 || inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  tech_reattach(7, test_mech, test_buffer);
  if (notifications != 1 || inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  technology_time = max_time;
  tech_reattach(7, test_mech, test_buffer);
  if (inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  *destroyed_at(CTORSO) = true;
  tech_reattach(7, test_mech, test_buffer);
  if (inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  inventory_internal = 4;
  tech_reattach(7, test_mech, test_buffer);
  if (inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  attaching = true;
  tech_reattach(7, test_mech, test_buffer);
  if (inventory_changes || scheduled.type)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 0) = -1;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 1) = -1;
  tech_reattach(1234567, test_mech, test_buffer);
  if (inventory_changes != 2 || scheduled.type != EVENT_REPAIR_REAT ||
      scheduled.callback != mech_event_failure_marker ||
      scheduled.delay != REATTACH_TIME * 3 / 2 ||
      scheduled.payload.location != RARM || scheduled.payload.player != 1234567)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  variable_technology_time = true;
  technology_time_modifier = 6;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 0) = 1;
  tech_reattach(7, test_mech, test_buffer);
  if (scheduled.delay != 225 || techtime_units != 225)
    return false;
  reset_test_state();
  *destroyed_at(RARM) = true;
  variable_technology_time = true;
  technology_time_modifier = 6;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 0) = -1;
  *(int *)checked_storage_at(roll_results, 2, sizeof(*roll_results), 1) = 1;
  tech_reattach(17, test_mech, test_buffer);
  return inventory_changes == 2 && scheduled.type == EVENT_REPAIR_REAT &&
         scheduled.callback == mux_event_tickmech_reattach &&
         scheduled.delay == 338 && techtime_units == 338;
}

static bool test_replacesuit_job_paths(void) {
  reset_test_state();
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  for (int location = 0; location < 4; location++)
    *destroyed_at(location) = false;
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  for (int location = 0; location < 4; location++)
    *destroyed_at(location) = true;
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  parse_result.location = 4;
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  *destroyed_at(RARM) = true;
  replacing = true;
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  *destroyed_at(RARM) = true;
  inventory_internal = 9;
  tech_replacesuit(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 9)
    return false;
  reset_test_state();
  unit_class = CLASS_BSUIT;
  *destroyed_at(RARM) = true;
  tech_replacesuit(999999, test_mech, test_buffer);
  if (!(scheduled.type == EVENT_REPAIR_REPSUIT &&
        scheduled.callback == mux_event_tickmech_replacesuit &&
        scheduled.delay == REPLACESUIT_TIME &&
        techtime_units == REPLACESUIT_TIME &&
        scheduled.payload.location == RARM &&
        scheduled.payload.player == 999999 && inventory_internal == 20))
    return false;
  reset_test_state();
  RepairOperationCall call = {.player = 7, .mech = test_mech};
  (void)replacesuit_fail(&call);
  return last_roll_difficulty == REPLACESUIT_DIFFICULTY;
}

static bool test_reseal_and_synchronous_commands(void) {
  reset_test_state();
  *flooded_at(RARM) = true;
  scrapping = true;
  tech_reseal(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  *flooded_at(RARM) = true;
  technology_time = max_time;
  tech_reseal(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  *flooded_at(RARM) = true;
  inventory_electrics = 4;
  tech_reseal(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30 || inventory_electrics != 4)
    return false;
  reset_test_state();
  *flooded_at(RARM) = true;
  resealing = true;
  tech_reseal(7, test_mech, test_buffer);
  if (scheduled.type || inventory_internal != 30)
    return false;
  reset_test_state();
  *flooded_at(RARM) = true;
  tech_reseal(31, test_mech, test_buffer);
  if (scheduled.type != EVENT_REPAIR_RESE ||
      scheduled.callback != mux_event_tickmech_reseal ||
      scheduled.delay != RESEAL_TIME || techtime_units != RESEAL_TIME ||
      scheduled.payload.location != RARM || scheduled.payload.player != 31 ||
      inventory_internal != 25 || inventory_electrics != 25)
    return false;
  reset_test_state();
  started = true;
  tech_fixextra(7, test_mech, test_buffer);
  tech_magic(7, test_mech, test_buffer);
  if (fixextra_calls || magic_calls || int_check_calls)
    return false;
  reset_test_state();
  tech_fixextra(7, test_mech, test_buffer);
  tech_magic(7, test_mech, test_buffer);
  return fixextra_calls == 1 && magic_calls == 1 && int_check_calls == 1;
}

int main(void) {
  if (!test_path_and_fixability())
    return 1;
  if (!test_reattach_rejections_and_rolls())
    return 2;
  if (!test_replacesuit_job_paths())
    return 3;
  return test_reseal_and_synchronous_commands() ? 0 : 4;
}
