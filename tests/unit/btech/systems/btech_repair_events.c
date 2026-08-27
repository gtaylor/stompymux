#include "mech_tech_events_api.h"

#include <stdbool.h>
#include <stdint.h>

#include "btech_event.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_parts.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_tech_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_do_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/support/checked_storage.h"
#include "repair_job.h"

typedef struct ScheduledEvent {
  int type;
  MuxEventCallback callback;
  int delay;
  intptr_t data;
} ScheduledEvent;

static BtechContext *const test_context = (BtechContext *)(uintptr_t)0x6;
static int armor[NUM_SECTIONS];
static int rear_armor[NUM_SECTIONS];
static int internal[NUM_SECTIONS];
static int original_armor[NUM_SECTIONS];
static int original_rear_armor[NUM_SECTIONS];
static int original_internal[NUM_SECTIONS];
static int critical_type;
static int critical_brand;
static int weapon_size;
static int repair_count;
static CriticalSlotReference repaired_slots[NUM_CRITICALS];
static int destroy_count;
static CriticalSlotReference destroyed_slots[NUM_CRITICALS];
static int temporary_failure_count;
static int brand_set_count;
static int damage_repair_count;
static int first_weapon_critical;
static int filled_ammo_count;
static int data_set_count;
static int added_parts_count;
static int added_part_ids[NUM_CRITICALS];
static int added_brands[NUM_CRITICALS];
static int added_quantities[NUM_CRITICALS];
static int reattach_count;
static int reseal_count;
static int replace_suit_count;
static int detach_count;
static int notification_count;
static UnitClass test_class;
static int maximum_battle_suits;
static bool split_found;
static CriticalSlotReference split_slot;
static ScheduledEvent scheduled;
static int schedule_count;
static bool critical_destroyed;
static bool critical_nonfunctional;
static bool critical_damaged;
static bool critical_disabled;
static bool section_destroyed;
static bool section_flooded;
static int critical_data_value;
static int ammunition_capacity;
static int destroyed_critical_section;
static int destroyed_critical_position;
static int mismatched_critical_section;
static int mismatched_critical_position;

int min(int left, int right);

static int *section_value(int values[NUM_SECTIONS], int section) {
  return checked_storage_at(values, NUM_SECTIONS, sizeof(*values),
                            (size_t)section);
}

static CriticalSlotReference *record_slot(CriticalSlotReference slots[],
                                          int count) {
  return checked_storage_at(slots, NUM_CRITICALS, sizeof(*slots),
                            (size_t)count);
}

static void reset_test_state(void) {
  for (int section = 0; section < NUM_SECTIONS; section++) {
    *section_value(armor, section) = 0;
    *section_value(rear_armor, section) = 0;
    *section_value(internal, section) = 0;
    *section_value(original_armor, section) = 10;
    *section_value(original_rear_armor, section) = 10;
    *section_value(original_internal, section) = 10;
  }
  critical_type = weapon_equipment_index(1);
  critical_brand = 4;
  weapon_size = 1;
  repair_count = 0;
  destroy_count = 0;
  temporary_failure_count = 0;
  brand_set_count = 0;
  damage_repair_count = 0;
  first_weapon_critical = 0;
  filled_ammo_count = 0;
  data_set_count = 0;
  added_parts_count = 0;
  reattach_count = 0;
  reseal_count = 0;
  replace_suit_count = 0;
  detach_count = 0;
  notification_count = 0;
  test_class = CLASS_MECH;
  maximum_battle_suits = 4;
  split_found = false;
  split_slot = (CriticalSlotReference){0};
  scheduled = (ScheduledEvent){0};
  schedule_count = 0;
  critical_destroyed = false;
  critical_nonfunctional = true;
  critical_damaged = true;
  critical_disabled = false;
  section_destroyed = false;
  section_flooded = false;
  critical_data_value = 10;
  ammunition_capacity = 10;
  destroyed_critical_section = destroyed_critical_position = -1;
  mismatched_critical_section = mismatched_critical_position = -1;
}

static MuxEvent event_for(Mech *mech, MuxEventCallback callback,
                          intptr_t payload) {
  return (MuxEvent){
      .data = mech,
      .secondary = {.kind = MUX_EVENT_PAYLOAD_INTEGER, .integer = payload},
      .function = callback};
}

int min(int left, int right) { return left < right ? left : right; }

void do_magic(Mech *mech [[maybe_unused]]) {}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}

int tech_time_scaled_seconds(BtechContext *context [[maybe_unused]],
                             int units) {
  return units;
}

void btech_context_event_schedule(BtechContext *context [[maybe_unused]],
                                  void *object [[maybe_unused]], int type,
                                  MuxEventCallback callback, int delay,
                                  intptr_t data) {
  scheduled = (ScheduledEvent){
      .type = type, .callback = callback, .delay = delay, .data = data};
  schedule_count++;
}

void mech_event_failure_marker(MuxEvent *event) {
  mux_event_tickmech_scrap_failure(event);
}

int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return NUM_CRITICALS;
}

int mech_section_armor(const Mech *mech [[maybe_unused]], int section) {
  return *section_value(armor, section);
}

int mech_section_rear_armor(const Mech *mech [[maybe_unused]], int section) {
  return *section_value(rear_armor, section);
}

int mech_section_internal(const Mech *mech [[maybe_unused]], int section) {
  return *section_value(internal, section);
}

int mech_section_original_armor(const Mech *mech [[maybe_unused]],
                                int section) {
  return *section_value(original_armor, section);
}

int mech_section_original_rear_armor(const Mech *mech [[maybe_unused]],
                                     int section) {
  return *section_value(original_rear_armor, section);
}

int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int section) {
  return *section_value(original_internal, section);
}

void mech_section_armor_set(Mech *mech [[maybe_unused]], int section,
                            int value) {
  *section_value(armor, section) = value;
}

void mech_section_rear_armor_set(Mech *mech [[maybe_unused]], int section,
                                 int value) {
  *section_value(rear_armor, section) = value;
}

void mech_section_internal_set(Mech *mech [[maybe_unused]], int section,
                               int value) {
  *section_value(internal, section) = value;
}

int mech_critical_part_type(const Mech *mech [[maybe_unused]], int section,
                            int critical) {
  if (section == mismatched_critical_section &&
      critical == mismatched_critical_position)
    return special_equipment_index(HEAT_SINK);
  if (split_found && section == split_slot.section &&
      critical >= split_slot.critical)
    return special_equipment_index(SPLIT_CRIT_RIGHT);
  return critical_type;
}

int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int section [[maybe_unused]],
                       int critical [[maybe_unused]]) {
  return critical_data_value;
}

int mech_critical_brand(const Mech *mech [[maybe_unused]],
                        int section [[maybe_unused]],
                        int critical [[maybe_unused]]) {
  return critical_brand;
}

void mech_critical_destroy(Mech *mech [[maybe_unused]], int section,
                           int critical) {
  *record_slot(destroyed_slots, destroy_count++) =
      (CriticalSlotReference){.section = section, .critical = critical};
  critical_destroyed = true;
}

bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]], int section,
                                int critical) {
  return critical_destroyed || (section == destroyed_critical_section &&
                                critical == destroyed_critical_position);
}

bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int section [[maybe_unused]],
                                    int critical [[maybe_unused]]) {
  return critical_nonfunctional;
}

bool mech_critical_is_damaged(const Mech *mech [[maybe_unused]],
                              int section [[maybe_unused]],
                              int critical [[maybe_unused]]) {
  return critical_damaged;
}

bool mech_part_is_structural_placeholder(int part_type) {
  return part_type == special_equipment_index(SPLIT_CRIT_LEFT) ||
         part_type == special_equipment_index(SPLIT_CRIT_RIGHT);
}

bool valid_gun_pos(const RepairCriticalSelection *selection [[maybe_unused]]) {
  return true;
}

bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]]) {
  return section_destroyed || detach_count > 0;
}

bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int section [[maybe_unused]]) {
  return section_flooded;
}

bool mech_critical_is_disabled(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]],
                               int critical [[maybe_unused]]) {
  return critical_disabled;
}

int full_ammo(const Mech *mech [[maybe_unused]], int section [[maybe_unused]],
              int critical [[maybe_unused]]) {
  return ammunition_capacity;
}

void mech_critical_temporary_failure_set(
    const CriticalSlotFailureSet *request) {
  *record_slot(repaired_slots, temporary_failure_count++) = request->slot;
}

void mech_critical_brand_set(const CriticalSlotBrandSet *request
                             [[maybe_unused]]) {
  brand_set_count++;
}

void mech_critical_data_set(Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]], int data) {
  critical_data_value = data;
  data_set_count++;
}

void mech_critical_damage_repair(Mech *mech [[maybe_unused]],
                                 int section [[maybe_unused]],
                                 int critical [[maybe_unused]]) {
  damage_repair_count++;
}

void mech_repair_part(Mech *mech [[maybe_unused]], int section, int critical) {
  *record_slot(repaired_slots, repair_count++) =
      (CriticalSlotReference){.section = section, .critical = critical};
}

int get_weapon_crits(Mech *mech [[maybe_unused]],
                     int weapon_index [[maybe_unused]]) {
  return weapon_size;
}

int mech_weapon_first_critical(const WeaponCriticalSearch *search
                               [[maybe_unused]]) {
  return first_weapon_critical;
}

SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  return (SplitCriticalLookup){.found = split_found,
                               .slot = split_slot,
                               .part_type =
                                   special_equipment_index(SPLIT_CRIT_RIGHT)};
}

int find_ammo_type(Mech *mech [[maybe_unused]], int section [[maybe_unused]],
                   int critical [[maybe_unused]]) {
  return cargo_equipment_index(9);
}

int tech_proper_armor_part(const Mech *mech [[maybe_unused]]) { return 400; }

int tech_proper_internal_part(const Mech *mech [[maybe_unused]]) { return 401; }

void mech_fill_part_ammo(Mech *mech [[maybe_unused]],
                         int section [[maybe_unused]],
                         int critical [[maybe_unused]]) {
  critical_data_value = ammunition_capacity;
  filled_ammo_count++;
}

void mech_parts_add(Mech *mech [[maybe_unused]], int part [[maybe_unused]],
                    int brand, int count) {
  *section_value(added_part_ids, added_parts_count) = part;
  *section_value(added_brands, added_parts_count) = brand;
  *section_value(added_quantities, added_parts_count) = count;
  added_parts_count++;
}

void mech_re_attach(Mech *mech [[maybe_unused]],
                    int location [[maybe_unused]]) {
  section_destroyed = false;
  reattach_count++;
}

void mech_re_seal(Mech *mech [[maybe_unused]], int location [[maybe_unused]]) {
  section_flooded = false;
  reseal_count++;
}

void mech_replace_suit(Mech *mech [[maybe_unused]],
                       int location [[maybe_unused]]) {
  section_destroyed = false;
  replace_suit_count++;
}

void mech_detach(Mech *mech [[maybe_unused]], int location [[maybe_unused]]) {
  detach_count++;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return test_class; }
int mech_maximum_battle_suits(const Mech *mech [[maybe_unused]]) {
  return maximum_battle_suits;
}

MechMovementType mech_movement_type(const Mech *mech [[maybe_unused]]) {
  return MOVE_BIPED;
}

bool mech_is_destroyed(const Mech *mech [[maybe_unused]]) { return false; }

void mech_destroyed_set(Mech *mech [[maybe_unused]],
                        bool destroyed [[maybe_unused]]) {}

void armor_string_from_index(int index [[maybe_unused]],
                             char buffer[static UNIT_SECTION_NAME_CAPACITY],
                             UnitClass type [[maybe_unused]],
                             MechMovementType movement_type [[maybe_unused]]) {
  (void)buffer;
}

PartDisplayName pos_part_name(Mech *mech [[maybe_unused]],
                              int index [[maybe_unused]],
                              int loop [[maybe_unused]]) {
  return (PartDisplayName){.text = "part", .valid = true};
}

void mech_printf(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *format [[maybe_unused]], ...) {
  notification_count++;
}

static bool test_armor_and_internal_events(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x7;
  constexpr DbRef player = 1073741824;

  reset_test_state();
  *section_value(armor, 1) = 3;
  MuxEvent armor_event =
      event_for(mech, mux_event_tickmech_repairarmor,
                repair_event_payload_pack((RepairEventPayload){
                    .location = 1, .position = 2, .player = player}));
  mux_event_tickmech_repairarmor(&armor_event);
  RepairEventPayload next = repair_event_payload_unpack(scheduled.data);
  if (*section_value(armor, 1) != 4 || schedule_count != 1 ||
      scheduled.callback != mux_event_tickmech_repairarmor ||
      scheduled.delay != FIXARMOR_TIME || next.location != 1 ||
      next.position != 1 || next.player != player)
    return false;

  MuxEvent final_armor = event_for(mech, scheduled.callback, scheduled.data);
  mux_event_tickmech_repairarmor(&final_armor);
  if (*section_value(armor, 1) != 5 || schedule_count != 1)
    return false;

  reset_test_state();
  RepairEventPayload extended_armor_payload = {.location = 1, .player = player};
  if (!repair_fix_event_payload_with_amount(&extended_armor_payload, 16))
    return false;
  MuxEvent extended_armor =
      event_for(mech, mux_event_tickmech_repairarmor,
                repair_event_payload_pack(extended_armor_payload));
  mux_event_tickmech_repairarmor(&extended_armor);
  next = repair_event_payload_unpack(scheduled.data);
  if (*section_value(armor, 1) != 1 || schedule_count != 1 ||
      repair_fix_event_amount(next) != 15 || next.player != player)
    return false;

  reset_test_state();
  *section_value(rear_armor, 1) = 6;
  MuxEvent rear_event =
      event_for(mech, mux_event_tickmech_repairarmor,
                repair_event_payload_pack(
                    (RepairEventPayload){.location = 9, .position = 1}));
  mux_event_tickmech_repairarmor(&rear_event);
  if (*section_value(rear_armor, 1) != 7 || schedule_count != 0)
    return false;

  reset_test_state();
  *section_value(armor, 1) = 6;
  MuxEvent zero_armor =
      event_for(mech, mux_event_tickmech_repairarmor,
                repair_event_payload_pack((RepairEventPayload){.location = 1}));
  mux_event_tickmech_repairarmor(&zero_armor);
  if (*section_value(armor, 1) != 6 || schedule_count != 0)
    return false;

  reset_test_state();
  *section_value(internal, 2) = 3;
  MuxEvent internal_event =
      event_for(mech, mux_event_tickmech_repairinternal,
                repair_event_payload_pack(
                    (RepairEventPayload){.location = 2, .position = 1}));
  mux_event_tickmech_repairinternal(&internal_event);
  if (*section_value(internal, 2) != 4 || schedule_count != 0)
    return false;

  reset_test_state();
  RepairEventPayload extended_internal_payload = {.location = 2,
                                                  .player = player};
  if (!repair_fix_event_payload_with_amount(&extended_internal_payload, 17))
    return false;
  MuxEvent extended_internal =
      event_for(mech, mux_event_tickmech_repairinternal,
                repair_event_payload_pack(extended_internal_payload));
  mux_event_tickmech_repairinternal(&extended_internal);
  next = repair_event_payload_unpack(scheduled.data);
  if (*section_value(internal, 2) != 1 || schedule_count != 1 ||
      repair_fix_event_amount(next) != 16 || next.player != player)
    return false;

  reset_test_state();
  *section_value(internal, 2) = 4;
  MuxEvent zero_internal =
      event_for(mech, mux_event_tickmech_repairinternal,
                repair_event_payload_pack((RepairEventPayload){.location = 2}));
  mux_event_tickmech_repairinternal(&zero_internal);
  if (*section_value(internal, 2) != 4 || schedule_count != 0)
    return false;

  reset_test_state();
  *section_value(armor, 1) = *section_value(original_armor, 1);
  RepairEventPayload stale_armor_payload = {.location = 1};
  if (!repair_fix_event_payload_with_amount(&stale_armor_payload, 3))
    return false;
  MuxEvent stale_armor =
      event_for(mech, mux_event_tickmech_repairarmor,
                repair_event_payload_pack(stale_armor_payload));
  mux_event_tickmech_repairarmor(&stale_armor);
  if (*section_value(armor, 1) != 10 || schedule_count != 0 ||
      notification_count != 0)
    return false;

  reset_test_state();
  *section_value(internal, 2) = 9;
  RepairEventPayload shortened_internal_payload = {.location = 2};
  if (!repair_fix_event_payload_with_amount(&shortened_internal_payload, 3))
    return false;
  MuxEvent shortened_internal =
      event_for(mech, mux_event_tickmech_repairinternal,
                repair_event_payload_pack(shortened_internal_payload));
  mux_event_tickmech_repairinternal(&shortened_internal);
  if (*section_value(internal, 2) != 10 || schedule_count != 0 ||
      notification_count != 1)
    return false;
  return true;
}

static bool test_gun_and_part_events(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x8;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = 3, .critical = 4};
  intptr_t payload = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 10, .extra = 7});
  MuxEvent repair_gun = event_for(mech, mux_event_tickmech_repairgun, payload);
  mux_event_tickmech_repairgun(&repair_gun);
  if (repair_count != 5 || temporary_failure_count != 5 ||
      repaired_slots[2].section != 3 || repaired_slots[2].critical != 4)
    return false;

  reset_test_state();
  weapon_size = 5;
  payload = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 10});
  repair_gun = event_for(mech, mux_event_tickmech_repairgun, payload);
  mux_event_tickmech_repairgun(&repair_gun);
  if (repair_count != 0 || temporary_failure_count != 0)
    return false;

  reset_test_state();
  weapon_size = 3;
  mismatched_critical_section = 1;
  mismatched_critical_position = 3;
  repair_gun = event_for(mech, mux_event_tickmech_repairgun,
                         repair_event_payload_pack((RepairEventPayload){
                             .location = 1, .position = 2}));
  mux_event_tickmech_repairgun(&repair_gun);
  if (repair_count != 0 || temporary_failure_count != 0)
    return false;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot =
      (CriticalSlotReference){.section = 3, .critical = NUM_CRITICALS - 1};
  repair_gun = event_for(mech, mux_event_tickmech_repairgun, payload);
  mux_event_tickmech_repairgun(&repair_gun);
  if (repair_count != 0 || temporary_failure_count != 0)
    return false;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = 3, .critical = 4};
  payload = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 10, .extra = 7});
  MuxEvent replace_gun =
      event_for(mech, mux_event_tickmech_replacegun, payload);
  mux_event_tickmech_replacegun(&replace_gun);
  if (repair_count != 5 || temporary_failure_count != 5 || brand_set_count != 5)
    return false;

  reset_test_state();
  critical_nonfunctional = false;
  replace_gun = event_for(mech, mux_event_tickmech_replacegun,
                          repair_event_payload_pack((RepairEventPayload){
                              .location = 1, .position = 2, .extra = 7}));
  mux_event_tickmech_replacegun(&replace_gun);
  if (repair_count != 0 || temporary_failure_count != 0 || brand_set_count != 0)
    return false;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = 3, .critical = 4};
  MuxEvent remove_gun =
      event_for(mech, mux_event_tickmech_removegun,
                repair_event_payload_pack((RepairEventPayload){
                    .location = 1, .position = 10, .extra = 2}));
  mux_event_tickmech_removegun(&remove_gun);
  if (destroy_count != 5 || destroyed_slots[2].section != 3 ||
      destroyed_slots[2].critical != 4 || added_parts_count != 1 ||
      added_part_ids[0] != cargo_equipment_index(9) || added_brands[0] != 4 ||
      added_quantities[0] != 1)
    return false;
  mux_event_tickmech_removegun(&remove_gun);
  if (destroy_count != 5 || added_parts_count != 1)
    return false;

  reset_test_state();
  section_flooded = true;
  remove_gun = event_for(mech, mux_event_tickmech_removegun,
                         repair_event_payload_pack((RepairEventPayload){
                             .location = 1, .position = 2, .extra = 2}));
  mux_event_tickmech_removegun(&remove_gun);
  if (destroy_count != 0 || added_parts_count != 0)
    return false;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = NUM_SECTIONS, .critical = 0};
  remove_gun = event_for(mech, mux_event_tickmech_removegun,
                         repair_event_payload_pack((RepairEventPayload){
                             .location = 1, .position = 10, .extra = 2}));
  mux_event_tickmech_removegun(&remove_gun);
  if (destroy_count || added_parts_count)
    return false;

  reset_test_state();
  weapon_size = 5;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = 3, .critical = 4};
  destroyed_critical_section = 3;
  destroyed_critical_position = 6;
  remove_gun = event_for(mech, mux_event_tickmech_removegun,
                         repair_event_payload_pack((RepairEventPayload){
                             .location = 1, .position = 10, .extra = 2}));
  mux_event_tickmech_removegun(&remove_gun);
  if (destroy_count || added_parts_count)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  MuxEvent repair_part =
      event_for(mech, mux_event_tickmech_repairpart,
                repair_event_payload_pack(
                    (RepairEventPayload){.location = 2, .position = 3}));
  mux_event_tickmech_repairpart(&repair_part);
  if (repair_count != 1 || repaired_slots[0].section != 2 ||
      repaired_slots[0].critical != 3)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  critical_nonfunctional = false;
  repair_part = event_for(mech, mux_event_tickmech_repairpart,
                          repair_event_payload_pack((RepairEventPayload){
                              .location = 2, .position = 3}));
  mux_event_tickmech_repairpart(&repair_part);
  if (repair_count != 0)
    return false;
  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  detach_count = 1;
  repair_part = event_for(mech, mux_event_tickmech_repairpart,
                          repair_event_payload_pack((RepairEventPayload){
                              .location = 2, .position = 3}));
  mux_event_tickmech_repairpart(&repair_part);
  if (repair_count != 0)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  MuxEvent remove_part =
      event_for(mech, mux_event_tickmech_removepart,
                repair_event_payload_pack((RepairEventPayload){
                    .location = 2, .position = 3, .extra = 2}));
  mux_event_tickmech_removepart(&remove_part);
  mux_event_tickmech_removepart(&remove_part);
  if (destroy_count != 1 || added_parts_count != 1 ||
      added_part_ids[0] != cargo_equipment_index(9) || added_brands[0] != 4 ||
      added_quantities[0] != 1)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  remove_part.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 2, .position = 3, .extra = 1});
  mux_event_tickmech_removepart(&remove_part);
  if (destroy_count || added_parts_count)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(SPLIT_CRIT_RIGHT);
  remove_part.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 2, .position = 3, .extra = 2});
  mux_event_tickmech_removepart(&remove_part);
  if (destroy_count || added_parts_count)
    return false;

  reset_test_state();
  critical_type = special_equipment_index(HEAT_SINK);
  remove_part = event_for(mech, mech_event_failure_marker,
                          repair_event_payload_pack((RepairEventPayload){
                              .location = 2, .position = 3, .extra = 3}));
  remove_part.type = EVENT_REPAIR_SCRP;
  mech_event_failure_marker(&remove_part);
  if (destroy_count != 1 || added_parts_count != 0)
    return false;

  reset_test_state();
  critical_type = weapon_equipment_index(1);
  MuxEvent failed_gun =
      event_for(mech, mech_event_failure_marker,
                repair_event_payload_pack((RepairEventPayload){
                    .location = 1, .position = 2, .extra = 3}));
  failed_gun.type = EVENT_REPAIR_SCRG;
  mech_event_failure_marker(&failed_gun);
  if (destroy_count != 1 || added_parts_count != 0)
    return false;

  reset_test_state();
  critical_type = weapon_equipment_index(1);
  failed_gun = event_for(mech, mech_event_failure_marker,
                         repair_event_payload_pack((RepairEventPayload){
                             .location = 1, .position = 2, .extra = 2}));
  failed_gun.type = EVENT_REPAIR_SCRG;
  mech_event_failure_marker(&failed_gun);
  if (destroy_count != 1 || added_parts_count != 0)
    return false;

  reset_test_state();
  failed_gun.type = EVENT_REPAIR_REPAG;
  mech_event_failure_marker(&failed_gun);
  if (destroy_count || added_parts_count)
    return false;

  reset_test_state();
  first_weapon_critical = -1;
  MuxEvent repair_enhanced =
      event_for(mech, mux_event_tickmech_repairenhcrit,
                repair_event_payload_pack(
                    (RepairEventPayload){.location = 2, .position = 3}));
  mux_event_tickmech_repairenhcrit(&repair_enhanced);
  if (notification_count != 0 || damage_repair_count != 0 ||
      temporary_failure_count != 0)
    return false;
  reset_test_state();
  detach_count = 1;
  repair_enhanced = event_for(mech, mux_event_tickmech_repairenhcrit,
                              repair_event_payload_pack((RepairEventPayload){
                                  .location = 2, .position = 3}));
  mux_event_tickmech_repairenhcrit(&repair_enhanced);
  if (notification_count != 0 || damage_repair_count != 0 ||
      temporary_failure_count != 0)
    return false;
  reset_test_state();
  critical_damaged = false;
  repair_enhanced = event_for(mech, mux_event_tickmech_repairenhcrit,
                              repair_event_payload_pack((RepairEventPayload){
                                  .location = 2, .position = 3}));
  mux_event_tickmech_repairenhcrit(&repair_enhanced);
  if (notification_count != 0 || damage_repair_count != 0 ||
      temporary_failure_count != 0)
    return false;
  return true;
}

static bool test_reload_and_payload_validation(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x9;

  reset_test_state();
  *section_value(internal, 2) = 6;
  *section_value(armor, 2) = 4;
  MuxEvent remove_section =
      event_for(mech, mux_event_tickmech_removesection,
                repair_event_payload_pack(
                    (RepairEventPayload){.location = 2, .extra = 2}));
  mux_event_tickmech_removesection(&remove_section);
  mux_event_tickmech_removesection(&remove_section);
  if (detach_count != 1 || added_parts_count != 3 || added_part_ids[0] != 401 ||
      added_quantities[0] != 6 || added_part_ids[1] != 400 ||
      added_quantities[1] != 4 ||
      added_part_ids[2] != cargo_equipment_index(S_ELECTRONIC) ||
      added_quantities[2] != 3)
    return false;

  reset_test_state();
  *section_value(internal, 2) = 6;
  *section_value(armor, 2) = 4;
  remove_section = event_for(mech, mux_event_tickmech_removesection,
                             repair_event_payload_pack((RepairEventPayload){
                                 .location = 2, .extra = 3}));
  mux_event_tickmech_removesection(&remove_section);
  if (detach_count != 1 || added_parts_count != 3 || added_quantities[0] != 4 ||
      added_quantities[1] != 2 || added_quantities[2] != 2)
    return false;

  reset_test_state();
  remove_section.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 2, .extra = 1});
  mux_event_tickmech_removesection(&remove_section);
  if (detach_count || added_parts_count)
    return false;

  reset_test_state();
  critical_type = ammunition_equipment_index(2);
  critical_nonfunctional = false;
  critical_data_value = 0;
  MuxEvent reload = event_for(mech, mux_event_tickmech_reload,
                              repair_event_payload_pack((RepairEventPayload){
                                  .location = 2, .position = 3}));
  mux_event_tickmech_reload(&reload);
  if (filled_ammo_count != 1 || data_set_count != 0)
    return false;

  reset_test_state();
  critical_type = ammunition_equipment_index(2);
  critical_nonfunctional = false;
  reload.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 2, .position = 3, .extra = 2});
  mux_event_tickmech_reload(&reload);
  if (filled_ammo_count != 0 || data_set_count != 1 || added_parts_count != 1)
    return false;
  mux_event_tickmech_reload(&reload);
  if (filled_ammo_count != 0 || data_set_count != 1 || added_parts_count != 1)
    return false;

  reset_test_state();
  critical_type = ammunition_equipment_index(2);
  critical_nonfunctional = true;
  reload = event_for(mech, mux_event_tickmech_reload,
                     repair_event_payload_pack((RepairEventPayload){
                         .location = 2, .position = 3, .extra = 2}));
  mux_event_tickmech_reload(&reload);
  if (data_set_count != 0 || added_parts_count != 0)
    return false;

  reset_test_state();
  constexpr DbRef player = 1073741824;
  intptr_t section_payload = repair_event_payload_pack(
      (RepairEventPayload){.location = 2, .player = player});
  MuxEvent section_event =
      event_for(mech, mux_event_tickmech_reattach, section_payload);
  section_destroyed = true;
  mux_event_tickmech_reattach(&section_event);
  section_flooded = true;
  section_event.function = mux_event_tickmech_reseal;
  mux_event_tickmech_reseal(&section_event);
  test_class = CLASS_BSUIT;
  section_destroyed = true;
  section_event.function = mux_event_tickmech_replacesuit;
  mux_event_tickmech_replacesuit(&section_event);
  if (reattach_count != 1 || reseal_count != 1 || replace_suit_count != 1)
    return false;
  section_event.function = mux_event_tickmech_reattach;
  mux_event_tickmech_reattach(&section_event);
  section_event.function = mux_event_tickmech_reseal;
  mux_event_tickmech_reseal(&section_event);
  section_event.function = mux_event_tickmech_replacesuit;
  mux_event_tickmech_replacesuit(&section_event);
  if (reattach_count != 1 || reseal_count != 1 || replace_suit_count != 1)
    return false;

  reset_test_state();
  section_event =
      event_for(mech, mux_event_tickmech_replacesuit,
                repair_event_payload_pack((RepairEventPayload){.location = 2}));
  mux_event_tickmech_replacesuit(&section_event);
  if (replace_suit_count != 0)
    return false;
  test_class = CLASS_BSUIT;
  maximum_battle_suits = 2;
  mux_event_tickmech_replacesuit(&section_event);
  if (replace_suit_count != 0)
    return false;
  maximum_battle_suits = 3;
  section_destroyed = true;
  mux_event_tickmech_replacesuit(&section_event);
  if (replace_suit_count != 1)
    return false;

  reset_test_state();
  MuxEvent corrupt = event_for(mech, mux_event_tickmech_repairarmor, -1);
  mux_event_tickmech_repairarmor(&corrupt);
  corrupt.function = mux_event_tickmech_repairinternal;
  mux_event_tickmech_repairinternal(&corrupt);
  corrupt.function = mux_event_tickmech_reattach;
  mux_event_tickmech_reattach(&corrupt);
  corrupt.function = mux_event_tickmech_replacesuit;
  mux_event_tickmech_replacesuit(&corrupt);
  corrupt.function = mux_event_tickmech_reseal;
  mux_event_tickmech_reseal(&corrupt);
  corrupt.function = mux_event_tickmech_repairpart;
  mux_event_tickmech_repairpart(&corrupt);
  corrupt.function = mux_event_tickmech_removegun;
  mux_event_tickmech_removegun(&corrupt);
  corrupt.function = mux_event_tickmech_replacegun;
  mux_event_tickmech_replacegun(&corrupt);
  corrupt.function = mux_event_tickmech_repairgun;
  mux_event_tickmech_repairgun(&corrupt);
  corrupt.function = mux_event_tickmech_repairenhcrit;
  mux_event_tickmech_repairenhcrit(&corrupt);
  corrupt.function = mux_event_tickmech_removepart;
  mux_event_tickmech_removepart(&corrupt);
  corrupt.function = mux_event_tickmech_reload;
  mux_event_tickmech_reload(&corrupt);
  corrupt.function = mux_event_tickmech_removesection;
  mux_event_tickmech_removesection(&corrupt);
  if (repair_count != 0 || reattach_count != 0 || replace_suit_count != 0 ||
      reseal_count != 0 || detach_count != 0 || data_set_count != 0 ||
      filled_ammo_count != 0 || schedule_count != 0)
    return false;

  reset_test_state();
  MuxEvent invalid_critical =
      event_for(mech, mux_event_tickmech_repairgun,
                repair_event_payload_pack((RepairEventPayload){
                    .location = NUM_SECTIONS, .position = 0}));
  mux_event_tickmech_repairgun(&invalid_critical);
  if (repair_count != 0 || temporary_failure_count != 0)
    return false;
  return true;
}

int main(void) {
  if (!test_armor_and_internal_events())
    return 1;
  if (!test_gun_and_part_events())
    return 2;
  return test_reload_and_payload_validation() ? 0 : 3;
}
