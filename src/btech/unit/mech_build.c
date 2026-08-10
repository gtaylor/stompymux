
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
#include "weapons.h"

const int num_def_weapons = NUM_DEF_WEAPONS;

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
  *settings = (BtechWeaponSettings){0};
  settings->values = calloc(num_def_weapons, sizeof(*settings->values));
  if (settings->values == nullptr)
    return false;
  settings->count = num_def_weapons;
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
  *settings = (BtechWeaponSettings){0};
}

static bool btech_weapon_settings_contains(const BtechWeaponSettings *settings,
                                           int weapon_index) {
  return settings != nullptr && weapon_index >= 0 &&
         (size_t)weapon_index < settings->count;
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
  const size_t definition_count =
      sizeof(MISSILE_HIT_DEFINITIONS) / sizeof(*MISSILE_HIT_DEFINITIONS) - 1;

  *registry = (MissileHitRegistry){0};
  registry->entries = calloc(definition_count, sizeof(*registry->entries));
  if (registry->entries == nullptr)
    return false;
  registry->count = definition_count;

  for (size_t index = 0; index < definition_count; index++) {
    MissileHitEntry *entry = missile_hit_entry(registry, index);
    *entry = *(const MissileHitEntry *)checked_storage_at_const(
        MISSILE_HIT_DEFINITIONS, definition_count + 1,
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
  *registry = (MissileHitRegistry){0};
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

void FillDefaultCriticals(Mech *mech, int index) {
  int loop;

  for (loop = 0; loop < NUM_CRITICALS; loop++) {
    mech_critical_configure(&(CriticalSlotConfiguration){
        .mech = mech,
        .slot = {.section = index, .critical = loop},
        .part_type = EMPTY});
  }

  if (((mech)->ud.type) == CLASS_AERO)
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
  if (((mech)->ud.type) == CLASS_MECH)
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

ArmorSectionAbbreviation
armor_section_abbreviation(const ArmorSectionReference *section) {
  ArmorSectionAbbreviation abbreviation = {0};
  UnitClass type = section->unit_class;
  MechMovementType movement_type = section->movement_type;
  int loc = section->location;
  if (loc < 0)
    return abbreviation;
  const UnitSectionCatalog catalog = {.unit_type = type,
                                      .movement_type = movement_type};
  const char *name = unit_section_name(&catalog, (size_t)loc);
  if (name == nullptr)
    return abbreviation;
  const size_t length = strlen(name);
  size_t output = 0;
  for (size_t input = 0;
       input < length && output + 1 < sizeof(abbreviation.text); input++) {
    const char character = *checked_string_suffix(name, input);
    if ((character >= 'A' && character <= 'Z') ||
        (character >= '0' && character <= '9')) {
      *(char *)checked_storage_at(abbreviation.text, sizeof(abbreviation.text),
                                  sizeof(char), output++) = character;
    }
  }
  *(char *)checked_storage_at(abbreviation.text, sizeof(abbreviation.text),
                              sizeof(char), output) = '\0';
  return abbreviation;
}

int ArmorSectionFromString(UnitClass type, MechMovementType movement_type,
                           const char *string) {
  int i, j;
  const char *c, *d;

  if (string == nullptr || !*string)
    return -1;
  const UnitSectionCatalog catalog = {.unit_type = type,
                                      .movement_type = movement_type};
  const size_t location_count = unit_section_name_count(&catalog);
  if (location_count == 0)
    return -1;
  /* Then, methodically compare against each other until a suitable
     match is found */
  for (i = 0; (size_t)i < location_count; i++)
    if (!strcasecmp(string, unit_section_name(&catalog, (size_t)i)))
      return i;
  for (i = 0; (size_t)i < location_count; i++) {
    const char first = ascii_to_upper(*string);
    const char *left = unit_section_name(&catalog, (size_t)i);
    if (first != *left)
      continue;
    for (j = i + 1; (size_t)j < location_count; j++)
      if (first == *unit_section_name(&catalog, (size_t)j))
        break;
    if ((size_t)j == location_count)
      return i;
    /* Ok, comparison between these two, then */
    c = strstr(left, " ");
    d = strstr(unit_section_name(&catalog, (size_t)j), " ");
    const char second = *checked_string_suffix(string, 1);
    if (!c && !second && d)
      return i;
    if (!c && !d)
      return -1;
    if (!second)
      continue;
    if (c && ascii_to_upper(second) == *checked_string_suffix(c, 1))
      return i;
    if (d && ascii_to_upper(second) == *checked_string_suffix(d, 1))
      return j;
  }
  return -1;
}

int WeaponIndexFromString(BtechContext *context, char *string) {
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = context,
                                          .pattern = string,
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (match.found && equipment_is_weapon(match.part.id))
    return weapon_from_equipment_index(match.part.id);
  return -1;
}

int FindSpecialItemCodeFromString(BtechContext *context, char *buffer) {
  PartMatchResult match =
      part_match_next(&(PartMatchRequest){.context = context,
                                          .pattern = buffer,
                                          .kind = PART_MATCH_VERY_LONG,
                                          .cursor = -1});
  if (match.found && equipment_is_special(match.part.id))
    return special_from_equipment_index(match.part.id);
  return -1;
}
