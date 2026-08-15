
/* Implements unit construction and initialization. */

#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "equipment_types.h"
#include "mech_build_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"
#include "weapon_settings.h"

// clang-format off: table rows intentionally remain one logical row each.
static const MissileHitEntry MISSILE_HIT_DEFINITIONS[] = {{"CL.LB10-XAC", 0, {3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"CL.LB20-XAC", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"CL.LB2-XAC", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.LB5-XAC", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"CL.LRM-10", 0, {3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"CL.LRM-15", 0, {5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15}},
                                                          {"CL.LRM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"CL.StreakLRM-5", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"CL.StreakLRM-10", 0, {3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"CL.StreakLRM-15", 0, {5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15}},
                                                          {"CL.StreakLRM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"CL.LRM-5", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"CL.SRM-2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.SRM-4", 0, {1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4}},
                                                          {"CL.SRM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"CL.StreakSRM-2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.StreakSRM-4", 0, {1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4}},
                                                          {"CL.StreakSRM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"CL.ATM-3", 0, {1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3}},
                                                          {"CL.ATM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"CL.ATM-9", 0, {2, 2, 3, 4, 4, 5, 5, 6, 7, 8, 9}},
                                                          {"CL.ATM-12", 0, {4, 4, 6, 6, 8, 8, 8, 10, 10, 12, 12}},
                                                          {"CL.UltraAC/10", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.UltraAC/20", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.UltraAC/2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.UltraAC/5", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.AC/2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.AC/5", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.AC/10", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"CL.AC/20", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.LRM-5", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"IS.LRM-10", 0, {3, 4, 4, 5, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"IS.LRM-15", 0, {5, 5, 9, 9, 9, 9, 9, 12, 12, 15, 15}},
                                                          {"IS.LRM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.SRM-2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.SRM-4", 0, {1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4}},
                                                          {"IS.SRM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"IS.StreakSRM-2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.StreakSRM-4", 0, {1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4}},
                                                          {"IS.StreakSRM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"IS.LB20-XAC", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.LB10-XAC", 0, {3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"IS.LB2-XAC", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.LB5-XAC", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"IS.UltraAC/20", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.UltraAC/10", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.UltraAC/5", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.UltraAC/2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.AC/2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.AC/5", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.AC/10", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.AC/20", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.LightAC/2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.LightAC/5", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.Thunderbolt-5", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.Thunderbolt-10", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.Thunderbolt-15", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.Thunderbolt-20", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.ELRM-10", 0, {3, 4, 4, 5, 6, 6, 6, 7, 7, 10, 10}},
                                                          {"IS.ELRM-5", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"IS.ELRM-15", 0, {5, 5, 9, 9, 9, 9, 12, 12, 12, 15, 15}},
                                                          {"IS.ELRM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.LR_DFM-10", 0, {3, 4, 4, 5, 6, 6, 6, 7, 7, 10, 10}},
                                                          {"IS.LR_DFM-5", 0, {1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5}},
                                                          {"IS.LR_DFM-15", 0, {5, 5, 9, 9, 9, 9, 12, 12, 12, 15, 15}},
                                                          {"IS.LR_DFM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.SR_DFM-2", 0, {1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2}},
                                                          {"IS.SR_DFM-4", 0, {1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4}},
                                                          {"IS.SR_DFM-6", 0, {2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6}},
                                                          {"IS.NarcBeacon", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"CL.NarcBeacon", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.MRM-10", 0, {2, 3, 4, 5, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"IS.MRM-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.MRM-30", 0, {10, 10, 12, 18, 18, 18, 18, 24, 24, 30, 30}},
                                                          {"IS.MRM-40", 0, {12, 12, 18, 24, 24, 24, 24, 32, 32, 40, 40}},
                                                          {"IS.iNarcBeacon", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.RL-10", 0, {3, 4, 4, 5, 6, 6, 6, 8, 8, 10, 10}},
                                                          {"IS.RL-15", 0, {5, 5, 9, 9, 9, 9, 9, 12, 12, 15, 15}},
                                                          {"IS.RL-20", 0, {6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20}},
                                                          {"IS.InfantrySRM", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"IS.InfantryLRM", 0, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}},
                                                          {"NoWeapon", -1, {1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}}};
// clang-format on

static BtechWeaponRuntimeValues *
weapon_runtime_values(BtechWeaponSettings *settings, size_t index) {
  return checked_storage_at(settings->values, settings->count,
                            sizeof(*settings->values), index);
}

static const BtechWeaponRuntimeValues *
weapon_runtime_values_const(const BtechWeaponSettings *settings, size_t index) {
  return checked_storage_at_const(settings->values, settings->count,
                                  sizeof(*settings->values), index);
}

static MissileHitEntry *missile_hit_entry(MissileHitRegistry *registry,
                                          size_t index) {
  return checked_storage_at(registry->entries, registry->count,
                            sizeof(*registry->entries), index);
}

static const MissileHitEntry *
missile_hit_entry_const(const MissileHitRegistry *registry, size_t index) {
  return checked_storage_at_const(registry->entries, registry->count,
                                  sizeof(*registry->entries), index);
}

bool btech_weapon_settings_initialize(BtechWeaponSettings *settings) {
  *settings = (BtechWeaponSettings){};
  settings->values = checked_storage_try_allocate_array(
      (size_t)DEFAULT_WEAPON_COUNT, sizeof(*settings->values));
  if (settings->values == nullptr)
    return false;
  settings->count = (size_t)DEFAULT_WEAPON_COUNT;
  for (size_t index = 0; index < settings->count; index++) {
    *weapon_runtime_values(settings, index) = (BtechWeaponRuntimeValues){
        .recycle_time = weapon_catalogue_recycle_time((int)index),
        .battle_value = weapon_catalogue_battle_value((int)index),
    };
  }
  return true;
}

void btech_weapon_settings_destroy(BtechWeaponSettings *settings) {
  if (settings == nullptr)
    return;
  free(settings->values);
  *settings = (BtechWeaponSettings){};
}

static bool btech_weapon_settings_contains(const BtechWeaponSettings *settings,
                                           int weapon_index) {
  return (settings != nullptr && weapon_index >= 0 &&
          (size_t)weapon_index < settings->count) != 0;
}

int btech_weapon_settings_recycle_time(const BtechWeaponSettings *settings,
                                       int weapon_index) {
  assert(btech_weapon_settings_contains(settings, weapon_index));
  return weapon_runtime_values_const(settings, (size_t)weapon_index)
      ->recycle_time;
}

int btech_weapon_settings_battle_value(const BtechWeaponSettings *settings,
                                       int weapon_index) {
  assert(btech_weapon_settings_contains(settings, weapon_index));
  return weapon_runtime_values_const(settings, (size_t)weapon_index)
      ->battle_value;
}

bool btech_weapon_settings_set_recycle_time(BtechWeaponSettings *settings,
                                            int weapon_index, int value) {
  if (!btech_weapon_settings_contains(settings, weapon_index))
    return false;
  weapon_runtime_values(settings, (size_t)weapon_index)->recycle_time = value;
  return true;
}

bool btech_weapon_settings_set_battle_value(BtechWeaponSettings *settings,
                                            int weapon_index, int value) {
  if (!btech_weapon_settings_contains(settings, weapon_index))
    return false;
  weapon_runtime_values(settings, (size_t)weapon_index)->battle_value = value;
  return true;
}

bool missile_hit_registry_initialize(MissileHitRegistry *registry,
                                     BtechContext *context) {
  const size_t DEFINITION_COUNT =
      (sizeof(MISSILE_HIT_DEFINITIONS) / sizeof(*MISSILE_HIT_DEFINITIONS)) - 1;

  *registry = (MissileHitRegistry){};
  registry->entries = checked_storage_try_allocate_array(
      DEFINITION_COUNT, sizeof(*registry->entries));
  if (registry->entries == nullptr)
    return false;
  registry->count = DEFINITION_COUNT;

  for (size_t index = 0; index < DEFINITION_COUNT; index++) {
    MissileHitEntry *entry = missile_hit_entry(registry, index);
    *entry = *(const MissileHitEntry *)checked_storage_at_const(
        MISSILE_HIT_DEFINITIONS, DEFINITION_COUNT + 1,
        sizeof(*MISSILE_HIT_DEFINITIONS), index);
    PartMatchResult match =
        part_match_next(&(PartMatchRequest){.context = context,
                                            .pattern = entry->name,
                                            .kind = PART_MATCH_VERY_LONG,
                                            .cursor = -1});
    if (match.found)
      entry->weapon_index = weapon_from_equipment_index(match.part.id);
    else
      entry->weapon_index = -1;
  }
  return true;
}

void missile_hit_registry_destroy(MissileHitRegistry *registry) {
  if (registry == nullptr)
    return;
  free(registry->entries);
  *registry = (MissileHitRegistry){};
}

const MissileHitEntry *
missile_hit_registry_find_weapon(const MissileHitRegistry *registry,
                                 int weapon_index) {
  if (registry == nullptr)
    return nullptr;
  for (size_t index = 0; index < registry->count; index++)
    if (missile_hit_entry_const(registry, index)->weapon_index == weapon_index)
      return missile_hit_entry_const(registry, index);
  return nullptr;
}

const MissileHitEntry *
missile_hit_registry_find_name(const MissileHitRegistry *registry,
                               const char *name) {
  if (registry == nullptr || name == nullptr)
    return nullptr;
  for (size_t index = 0; index < registry->count; index++)
    if (strcmp(missile_hit_entry_const(registry, index)->name, name) == 0)
      return missile_hit_entry_const(registry, index);
  return nullptr;
}

void fill_default_criticals(Mech *mech, int index) {
  int loop;

  for (loop = 0; loop < NUM_CRITICALS; loop++) {
    mech_critical_configure(&(CriticalSlotConfiguration){
        .mech = mech,
        .slot = {.section = index, .critical = loop},
        .part_type = EMPTY});
  }

  if (((mech)->ud.type) == CLASS_AERO) {
    switch (index) {
    case AERO_AFT:
      for (loop = 0; loop < 12; loop++)
        mech_critical_part_type_set(mech, index, loop,
                                    special_equipment_index(HEAT_SINK));
      mech_critical_part_type_set(mech, index, 2,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 10,
                                  special_equipment_index(ENGINE));
      break;
    }
  }
  if (((mech)->ud.type) == CLASS_MECH) {
    switch (index) {
    case HEAD:
      mech_critical_part_type_set(mech, index, 0,
                                  special_equipment_index(LIFE_SUPPORT));
      mech_critical_part_type_set(mech, index, 1,
                                  special_equipment_index(SENSORS));
      mech_critical_part_type_set(mech, index, 2,
                                  special_equipment_index(COCKPIT));
      mech_critical_part_type_set(mech, index, 4,
                                  special_equipment_index(SENSORS));
      mech_critical_part_type_set(mech, index, 5,
                                  special_equipment_index(LIFE_SUPPORT));
      break;

    case CTORSO:
      mech_critical_part_type_set(mech, index, 0,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 1,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 2,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 3,
                                  special_equipment_index(GYRO));
      mech_critical_part_type_set(mech, index, 4,
                                  special_equipment_index(GYRO));
      mech_critical_part_type_set(mech, index, 5,
                                  special_equipment_index(GYRO));
      mech_critical_part_type_set(mech, index, 6,
                                  special_equipment_index(GYRO));
      mech_critical_part_type_set(mech, index, 7,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 8,
                                  special_equipment_index(ENGINE));
      mech_critical_part_type_set(mech, index, 9,
                                  special_equipment_index(ENGINE));
      break;

    case RTORSO:
    case LTORSO:
      break;

    case LARM:
    case RARM:
    case LLEG:
    case RLEG:
      mech_critical_part_type_set(mech, index, 0,
                                  special_equipment_index(SHOULDER_OR_HIP));
      mech_critical_part_type_set(mech, index, 1,
                                  special_equipment_index(UPPER_ACTUATOR));
      mech_critical_part_type_set(mech, index, 2,
                                  special_equipment_index(LOWER_ACTUATOR));
      mech_critical_part_type_set(
          mech, index, 3, special_equipment_index(HAND_OR_FOOT_ACTUATOR));
      break;
    }
  }
}

ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section) {
  ArmorSectionAbbreviation abbreviation = {0};
  UnitClass type = section->unit_class;
  MechMovementType movement_type = section->movement_type;
  int loc = section->location;
  if (loc < 0)
    return abbreviation;
  const UnitSectionCatalog CATALOG = {.unit_type = type,
                                      .movement_type = movement_type};
  const char *name = unit_section_name(&CATALOG, (size_t)loc);
  if (name == nullptr)
    return abbreviation;
  const size_t LENGTH = strlen(name);
  size_t output = 0;
  for (size_t input = 0;
       input < LENGTH && output + 1 < sizeof(abbreviation.text); input++) {
    const char CHARACTER = *checked_string_suffix(name, input);
    if ((CHARACTER >= 'A' && CHARACTER <= 'Z') ||
        (CHARACTER >= '0' && CHARACTER <= '9')) {
      *(char *)checked_storage_at(abbreviation.text, sizeof(abbreviation.text),
                                  sizeof(char), output++) = CHARACTER;
    }
  }
  *(char *)checked_storage_at(abbreviation.text, sizeof(abbreviation.text),
                              sizeof(char), output) = '\0';
  return abbreviation;
}

int armor_section_from_string(UnitClass type, MechMovementType movement_type,
                              const char *string) {
  int i;
  int j;
  const char *c;
  const char *d;

  if (string == nullptr || !*string)
    return -1;
  const UnitSectionCatalog CATALOG = {.unit_type = type,
                                      .movement_type = movement_type};
  const size_t LOCATION_COUNT = unit_section_name_count(&CATALOG);
  if (LOCATION_COUNT == 0)
    return -1;
  /* Then, methodically compare against each other until a suitable
     match is found */
  for (i = 0; (size_t)i < LOCATION_COUNT; i++)
    if (!strcasecmp(string, unit_section_name(&CATALOG, (size_t)i)))
      return i;
  for (i = 0; (size_t)i < LOCATION_COUNT; i++) {
    const char FIRST = ascii_to_upper(*string);
    const char *left = unit_section_name(&CATALOG, (size_t)i);
    if (FIRST != *left)
      continue;
    for (j = i + 1; (size_t)j < LOCATION_COUNT; j++)
      if (FIRST == *unit_section_name(&CATALOG, (size_t)j))
        break;
    if ((size_t)j == LOCATION_COUNT)
      return i;
    /* Ok, comparison between these two, then */
    c = strstr(left, " ");
    d = strstr(unit_section_name(&CATALOG, (size_t)j), " ");
    const char SECOND = *checked_string_suffix(string, 1);
    if (!c && !SECOND && d)
      return i;
    if (!c && !d)
      return -1;
    if (!SECOND)
      continue;
    if (c && ascii_to_upper(SECOND) == *checked_string_suffix(c, 1))
      return i;
    if (d && ascii_to_upper(SECOND) == *checked_string_suffix(d, 1))
      return j;
  }
  return -1;
}

int weapon_index_from_string(BtechContext *context, const char *string) {
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = context,
                                          .pattern = string,
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (match.found && equipment_is_weapon(match.part.id))
    return weapon_from_equipment_index(match.part.id);
  return -1;
}

int find_special_item_code_from_string(BtechContext *context,
                                       const char *buffer) {
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = context,
                                          .pattern = buffer,
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (match.found && equipment_is_special(match.part.id))
    return special_from_equipment_index(match.part.id);
  return -1;
}
