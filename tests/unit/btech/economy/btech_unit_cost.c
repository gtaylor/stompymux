#include "aero_bomb_api.h"
#include "btech/context.h"
#include "btech/core/context_internal.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_consistency_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_internal.h"
#include "mech_specification_api.h"
#include "mech_status_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "part_cost_api.h"
#include "section_types.h"
#include "template_internal.h"
#include "unit_cost_api.h"
#include "weapon_catalogue_api.h"

#include <stdbool.h>
#include <stdio.h>

const int TEMPLATE_INTERNAL_COUNT = 1;
const int TEMPLATE_CARGO_COUNT = 1;
const int INTERNALSWEIGHT[] = {0};
const int CARGOWEIGHT[] = {0};

static int fixture_slots[NUM_SECTIONS * NUM_CRITICALS];
static int fixture_technology;

static int *fixture_slot(int section, int critical) {
  const size_t index =
      ((size_t)section * (size_t)NUM_CRITICALS) + (size_t)critical;
  return checked_storage_at(fixture_slots, NUM_SECTIONS * NUM_CRITICALS,
                            sizeof(*fixture_slots), index);
}

static const int *fixture_slot_at(size_t index) {
  return checked_storage_at_const(fixture_slots, NUM_SECTIONS * NUM_CRITICALS,
                                  sizeof(*fixture_slots), index);
}

static void fixture_reset(void) {
  for (size_t index = 0; index < NUM_SECTIONS * NUM_CRITICALS; ++index)
    *fixture_slot((int)(index / NUM_CRITICALS), (int)(index % NUM_CRITICALS)) =
        EMPTY;
  fixture_technology = 0;
}

static int fixture_find(bool (*predicate)(int)) {
  for (size_t index = 0; index < NUM_SECTIONS * NUM_CRITICALS; ++index) {
    const int part = *fixture_slot_at(index);
    if (predicate(part))
      return part;
  }
  return EMPTY;
}

int bomb_weight(int index [[maybe_unused]]) { return 0; }

int bounded(int minimum, int value, int maximum) {
  if (value < minimum)
    return minimum;
  return value > maximum ? maximum : value;
}

void btech_channel_send(BtechContext *context [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format [[maybe_unused]], ...) {}

int crit_weight(Mech *mech [[maybe_unused]], int part [[maybe_unused]]) {
  return 0;
}

int find_weapons_advanced(Mech *mech [[maybe_unused]],
                          int section [[maybe_unused]], unsigned char *weapons,
                          unsigned char *data, int *critical,
                          int whine [[maybe_unused]]) {
  /* These fixtures contain at most one weapon, always in the first section. */
  const int part = fixture_find(equipment_is_weapon);
  if (part == EMPTY)
    return 0;
  weapons[0] = (unsigned char)weapon_from_equipment_index(part);
  data[0] = 0;
  critical[0] = 0;
  return section == 0 ? 1 : 0;
}

int find_ammunition(Mech *mech [[maybe_unused]], unsigned char *weapons,
                    unsigned short *ammo, unsigned short *maximum_ammo,
                    unsigned int *modes, int return_all [[maybe_unused]]) {
  const int part = fixture_find(equipment_is_ammunition);
  if (part == EMPTY)
    return 0;
  weapons[0] = (unsigned char)ammunition_to_weapon_index(part);
  ammo[0] = 10;
  maximum_ammo[0] = 10;
  modes[0] = 0;
  return 1;
}

UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_MECH; }

BtechContext *mech_context(const Mech *mech) { return mech->xcode.context; }

int mech_critical_part_type(const Mech *mech [[maybe_unused]], int section,
                            int critical) {
  return *fixture_slot(section, critical);
}

int mech_engine_rating(const Mech *mech [[maybe_unused]]) { return 0; }
int mech_heat_sink_count(const Mech *mech [[maybe_unused]]) { return 0; }
bool mech_is_omni(const Mech *mech [[maybe_unused]]) { return false; }
float mech_jump_speed(const Mech *mech [[maybe_unused]]) { return 0.0F; }
MechMovementType mech_movement_type(const Mech *mech [[maybe_unused]]) {
  return MOVE_QUAD;
}
int mech_section_original_armor(const Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return 0;
}
int mech_section_original_rear_armor(const Mech *mech [[maybe_unused]],
                                     int section [[maybe_unused]]) {
  return 0;
}
int mech_technology_flags(const Mech *mech [[maybe_unused]]) {
  return fixture_technology;
}
int mech_technology_flags_secondary(const Mech *mech [[maybe_unused]]) {
  return 0;
}
int mech_tonnage(const Mech *mech [[maybe_unused]]) { return 0; }

PartDisplayName part_name(BtechContext *context [[maybe_unused]],
                          int type [[maybe_unused]],
                          int brand [[maybe_unused]]) {
  return (PartDisplayName){.text = "fixture", .valid = true};
}

int round_to_halfton(int weight) { return weight; }
int susp_factor(Mech *mech [[maybe_unused]]) { return 0; }

int weapon_catalogue_ammunition_cost(int weapon_index [[maybe_unused]]) {
  return 200;
}
int weapon_catalogue_ammunition_per_ton(int weapon_index [[maybe_unused]]) {
  return 10;
}
int weapon_catalogue_cost(int weapon_index [[maybe_unused]]) { return 1000; }
bool weapon_catalogue_is_energy(int weapon_index [[maybe_unused]]) {
  return false;
}
const char *weapon_catalogue_name(int weapon_index [[maybe_unused]]) {
  return "fixture weapon";
}
int weapon_catalogue_weight(int weapon_index [[maybe_unused]]) { return 0; }

static unsigned long long fixture_cost(Mech *mech) {
  return mech_fasa_cost(mech);
}

static bool check_generic_part_cost(Mech *mech, BtechContext *context) {
  fixture_reset();
  btech_part_costs_reset(context);
  const unsigned long long baseline = fixture_cost(mech);
  const int mace = special_equipment_index(MACE);
  btech_part_cost_set(context, mace, 7);
  *fixture_slot(0, 0) = mace;
  if (fixture_cost(mech) != baseline + 7)
    return false;
  *fixture_slot(0, 1) = mace;
  return fixture_cost(mech) == baseline + 14;
}

static bool check_weapon_and_ammunition_costs(Mech *mech,
                                              BtechContext *context) {
  fixture_reset();
  btech_part_costs_reset(context);
  const unsigned long long baseline = fixture_cost(mech);
  const int weapon = weapon_equipment_index(0);
  btech_part_cost_set(context, weapon, 4444);
  *fixture_slot(0, 0) = weapon;
  if (fixture_cost(mech) != baseline + 1000)
    return false;

  fixture_reset();
  const int ammunition = ammunition_equipment_index(0);
  btech_part_cost_set(context, ammunition, 3333);
  *fixture_slot(0, 0) = ammunition;
  if (fixture_cost(mech) != baseline + 200)
    return false;

  fixture_technology = CLAN_TECH;
  const unsigned long long clan_with_ammo = fixture_cost(mech);
  fixture_reset();
  fixture_technology = CLAN_TECH;
  const unsigned long long clan_baseline = fixture_cost(mech);
  return clan_baseline == baseline && clan_with_ammo == clan_baseline + 50200;
}

static bool check_case_ii_and_markers(Mech *mech, BtechContext *context) {
  fixture_reset();
  btech_part_costs_reset(context);
  const unsigned long long baseline = fixture_cost(mech);
  *fixture_slot(0, 0) = special_equipment_index(CASE_II);
  if (fixture_cost(mech) != baseline + 175000)
    return false;

  fixture_reset();
  const int marker = special_equipment_index(SPLIT_CRIT_RIGHT);
  btech_part_cost_set(context, marker, 999);
  *fixture_slot(0, 0) = marker;
  return fixture_cost(mech) == baseline;
}

int main(void) {
  ServerConfiguration configuration = {};
  BtechContext context = {.configuration = &configuration};
  Mech mech = {.xcode.context = &context};
  btech_part_costs_initialize(&context);

  const bool passed = check_generic_part_cost(&mech, &context) &&
                      check_weapon_and_ammunition_costs(&mech, &context) &&
                      check_case_ii_and_markers(&mech, &context);
  btech_part_costs_destroy(&context);
  if (!passed) {
    fputs("unit cost regression\n", stderr);
    return 1;
  }
  return 0;
}
