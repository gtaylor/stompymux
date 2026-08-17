#include "mech_tech_do_api.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "btech/context.h"
#include "equipment_types.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_parts.h"
#include "mech_tech_api.h"
#include "mech_utils_api.h"
#include "mux/network/network_output.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"

static BtechContext *const test_context = (BtechContext *)(uintptr_t)0x2;
static bool consume_result;
static MechPartRequirement consumed[4];
static size_t consumed_count;
static bool critical_destroyed;
static int critical_part = WEAPON_BASE_INDEX;
static int critical_brand = 7;
static int original_internal = 12;
static int random_roll = 5;
static int skill_roll;
static int last_roll_difficulty;
static int weapon_criticals = 6;
static int section_criticals;
static int destroyed_slots[NUM_CRITICALS];
static int destroyed_sections[NUM_CRITICALS];
static size_t destroyed_slot_count;
static bool split_found;
static CriticalSlotReference split_slot;

static void reset_test_state(void) {
  consume_result = true;
  consumed_count = 0;
  critical_destroyed = false;
  critical_part = WEAPON_BASE_INDEX;
  critical_brand = 7;
  original_internal = 12;
  random_roll = 5;
  skill_roll = -1;
  last_roll_difficulty = -1;
  weapon_criticals = 6;
  section_criticals = NUM_CRITICALS;
  destroyed_slot_count = 0;
  split_found = false;
  split_slot = (CriticalSlotReference){0};
}

BtechContext *mech_context(const Mech *mech [[maybe_unused]]) {
  return test_context;
}

EvaluationContext *btech_context_evaluation(BtechContext *context
                                            [[maybe_unused]]) {
  return nullptr;
}

long btech_random_range(BtechContext *context [[maybe_unused]],
                        long low [[maybe_unused]], long high [[maybe_unused]]) {
  return random_roll;
}

int tech_roll(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
              int difficulty) {
  last_roll_difficulty = difficulty;
  return skill_roll;
}

void notify_printf(EvaluationContext *evaluation [[maybe_unused]],
                   DbRef player [[maybe_unused]],
                   const char *format [[maybe_unused]], ...) {}

void mecha_notify(EvaluationContext *evaluation [[maybe_unused]],
                  DbRef player [[maybe_unused]],
                  const char *message [[maybe_unused]]) {}

int mech_parts_alias(const Mech *mech [[maybe_unused]], int part) {
  return part + 100;
}

bool mech_parts_consume(Mech *mech [[maybe_unused]],
                        DbRef player [[maybe_unused]],
                        const MechPartRequirement requirements[],
                        size_t count) {
  consumed_count = count;
  for (size_t i = 0; i < count; i++) {
    MechPartRequirement *target =
        checked_storage_at(consumed, 4, sizeof(*consumed), i);
    const MechPartRequirement *source =
        checked_storage_at_const(requirements, count, sizeof(*requirements), i);
    *target = *source;
  }
  return consume_result;
}

int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int critical [[maybe_unused]]) {
  return critical_part;
}

int mech_critical_brand(const Mech *mech [[maybe_unused]],
                        int section [[maybe_unused]],
                        int critical [[maybe_unused]]) {
  return critical_brand;
}

bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]],
                                int critical [[maybe_unused]]) {
  return critical_destroyed;
}

void mech_critical_destroy(Mech *mech [[maybe_unused]],
                           int section [[maybe_unused]], int critical) {
  int *slot =
      checked_storage_at(destroyed_slots, NUM_CRITICALS,
                         sizeof(*destroyed_slots), destroyed_slot_count++);
  *slot = critical;
  int *target_section =
      checked_storage_at(destroyed_sections, NUM_CRITICALS,
                         sizeof(*destroyed_sections), destroyed_slot_count - 1);
  *target_section = section;
}

SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  return (SplitCriticalLookup){.found = split_found, .slot = split_slot};
}

int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return section_criticals;
}

int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int section [[maybe_unused]]) {
  return original_internal;
}

int tech_proper_armor_part(const Mech *mech [[maybe_unused]]) { return 501; }
int tech_proper_internal_part(const Mech *mech [[maybe_unused]]) { return 502; }

int get_weapon_crits(Mech *mech [[maybe_unused]],
                     int weapon_index [[maybe_unused]]) {
  return weapon_criticals;
}

static bool test_resource_requirements(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x3;
  int amount = 1;
  RepairOperationCall call = {.player = 17,
                              .mech = mech,
                              .selection = {.location = 4, .part = 5},
                              .amount = &amount};

  reset_test_state();
  if (fixarmor_econ(&call) != 0 || consumed_count != 1 ||
      consumed[0].part != 601 || consumed[0].brand != 0 ||
      consumed[0].count != 1)
    return false;

  reset_test_state();
  amount = 1000;
  if (fixinternal_econ(&call) != 0 || consumed_count != 1 ||
      consumed[0].part != 602 || consumed[0].count != 1000)
    return false;

  reset_test_state();
  critical_part = ammunition_equipment_index(4);
  if (replace_econ(&call) != 0 || consumed_count != 0)
    return false;

  reset_test_state();
  critical_part = weapon_equipment_index(4);
  if (replace_econ(&call) != 0 || consumed_count != 1 ||
      consumed[0].part != critical_part + 100 || consumed[0].brand != 7 ||
      consumed[0].count != 1)
    return false;

  reset_test_state();
  critical_destroyed = true;
  consume_result = false;
  if (repair_econ(&call) != -1 || consumed_count != 2 ||
      consumed[0].part != cargo_equipment_index(S_ELECTRONIC) + 100 ||
      consumed[0].count != 3 || consumed[1].part != 602 ||
      consumed[1].count != 3)
    return false;

  reset_test_state();
  consume_result = false;
  if (reattach_econ(&call) != -1 || consumed_count != 2 ||
      consumed[0].part != 602 || consumed[0].count != 12 ||
      consumed[1].part != cargo_equipment_index(S_ELECTRONIC) + 100 ||
      consumed[1].count != 12)
    return false;

  reset_test_state();
  consume_result = false;
  if (replacesuit_econ(&call) != -1 || consumed_count != 4 ||
      consumed[0].part != 602 || consumed[0].count != 10 ||
      consumed[1].part != cargo_equipment_index(BSUIT_SENSOR) + 100 ||
      consumed[1].count != 2 ||
      consumed[2].part != cargo_equipment_index(BSUIT_LIFESUPPORT) + 100 ||
      consumed[2].count != 2 ||
      consumed[3].part != cargo_equipment_index(BSUIT_ELECTRONIC) + 100 ||
      consumed[3].count != 10)
    return false;
  return true;
}

static bool test_failed_amount_boundaries(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x4;
  int amount = 0;
  RepairOperationCall call = {.player = 23, .mech = mech, .amount = &amount};

  reset_test_state();
  if (fixarmor_fail(&call) != 0 || amount != 0)
    return false;

  reset_test_state();
  amount = 1;
  if (fixarmor_fail(&call) != 0 || amount != 1)
    return false;

  reset_test_state();
  amount = 1000;
  random_roll = 44;
  skill_roll = 0;
  if (fixinternal_fail(&call) != 0 || amount < 1 || amount >= 1000 ||
      last_roll_difficulty != FIXINTERNAL_DIFFICULTY)
    return false;
  return true;
}

static bool test_large_weapon_failure(void) {
  Mech *const mech = (Mech *)(uintptr_t)0x5;
  RepairOperationCall call = {
      .player = 99, .mech = mech, .selection = {.location = 2, .part = 4}};

  reset_test_state();
  critical_destroyed = true;
  if (repairg_fail(&call) != -1 || destroyed_slot_count != 1 ||
      destroyed_sections[0] != 2 || destroyed_slots[0] != 5)
    return false;

  reset_test_state();
  critical_destroyed = true;
  call.selection.part = NUM_CRITICALS - 1;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = 6, .critical = 2};
  if (repairg_fail(&call) != -1 || destroyed_slot_count != 1 ||
      destroyed_sections[0] != 6 || destroyed_slots[0] != 2)
    return false;

  reset_test_state();
  critical_destroyed = true;
  call.selection.part = NUM_CRITICALS - 1;
  split_found = true;
  split_slot = (CriticalSlotReference){.section = NUM_SECTIONS, .critical = 0};
  if (repairg_fail(&call) != -1 || destroyed_slot_count != 0)
    return false;

  reset_test_state();
  critical_destroyed = true;
  call.selection.part = NUM_CRITICALS - 1;
  if (repairg_fail(&call) != -1 || destroyed_slot_count != 0)
    return false;

  reset_test_state();
  critical_destroyed = true;
  weapon_criticals = 4;
  if (repairg_fail(&call) != -1 || destroyed_slot_count != 0)
    return false;
  return true;
}

int main(void) {
  return test_resource_requirements() && test_failed_amount_boundaries() &&
                 test_large_weapon_failure()
             ? 0
             : 1;
}
