
/*
 * $Id: mech.build.c,v 1.1.1.1 2005/01/11 21:18:11 kstevens Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Last modified: Wed Apr 29 21:04:14 1998 fingon
 *
 */

#include <assert.h>
#include <ctype.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "mech_build_api.h"
#include "mech_equipment_api.h"
#include "mech_internal.h"
#include "mech_lifecycle.h"
#include "mech_partnames_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "weapon_settings.h"
#include "weapons.h"

const int num_def_weapons = NUM_DEF_WEAPONS;

bool btech_weapon_settings_initialize(BtechWeaponSettings *settings) {
  *settings = (BtechWeaponSettings){0};
  settings->values = calloc(num_def_weapons, sizeof(*settings->values));
  if (settings->values == nullptr)
    return false;
  settings->count = num_def_weapons;
  for (size_t index = 0; index < settings->count; index++) {
    settings->values[index] = (BtechWeaponRuntimeValues){
        .recycle_time = MechWeapons[index].vrt,
        .battle_value = MechWeapons[index].battlevalue,
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
  return settings->values[weapon_index].recycle_time;
}

int btech_weapon_settings_battle_value(const BtechWeaponSettings *settings,
                                       int weapon_index) {
  assert(btech_weapon_settings_contains(settings, weapon_index));
  return settings->values[weapon_index].battle_value;
}

bool btech_weapon_settings_set_recycle_time(BtechWeaponSettings *settings,
                                            int weapon_index, int value) {
  if (!btech_weapon_settings_contains(settings, weapon_index))
    return false;
  settings->values[weapon_index].recycle_time = value;
  return true;
}

bool btech_weapon_settings_set_battle_value(BtechWeaponSettings *settings,
                                            int weapon_index, int value) {
  if (!btech_weapon_settings_contains(settings, weapon_index))
    return false;
  settings->values[weapon_index].battle_value = value;
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
    int id;
    int brand;

    registry->entries[index] = MISSILE_HIT_DEFINITIONS[index];
    if (find_matching_vlong_part(context, registry->entries[index].name,
                                 nullptr, &id, &brand))
      registry->entries[index].weapon_index = weapon_from_equipment_index(id);
    else
      registry->entries[index].weapon_index = -1;
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
    if (registry->entries[index].weapon_index == weapon_index)
      return &registry->entries[index];
  return nullptr;
}

const MissileHitEntry *
missile_hit_registry_find_name(const MissileHitRegistry *registry,
                               const char *name) {
  if (registry == nullptr || name == nullptr)
    return nullptr;
  for (size_t index = 0; index < registry->count; index++)
    if (strcmp(registry->entries[index].name, name) == 0)
      return &registry->entries[index];
  return nullptr;
}

void FillDefaultCriticals(Mech *mech, int index) {
  int loop;

  for (loop = 0; loop < NUM_CRITICALS; loop++) {
    ((mech)->ud.sections)[index].criticals[loop].type = EMPTY;
    ((mech)->ud.sections)[index].criticals[loop].data = 0;
    ((mech)->ud.sections)[index].criticals[loop].firemode = 0;
    ((mech)->ud.sections)[index].criticals[loop].ammomode = 0;
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
armor_section_abbreviation(UnitClass type, MechMovementType movement_type,
                           int loc) {
  const char *const *locs;
  ArmorSectionAbbreviation abbreviation = {0};
  char *cursor = abbreviation.text;
  int i;

  locs = ProperSectionStringFromType(type, movement_type);
  for (i = 0; locs[loc][i]; i++)
    if (isupper(locs[loc][i]) || isdigit(locs[loc][i]))
      *(cursor++) = locs[loc][i];
  *cursor = '\0';
  return abbreviation;
}

int ArmorSectionFromString(UnitClass type, MechMovementType movement_type,
                           const char *string) {
  const char *const *locs;
  int i, j;
  char *c, *d;

  if (!string[0])
    return -1;
  locs = ProperSectionStringFromType(type, movement_type);
  if (!locs)
    return -1;
  /* Then, methodically compare against each other until a suitable
     match is found */
  for (i = 0; locs[i]; i++)
    if (!strcasecmp(string, locs[i]))
      return i;
  for (i = 0; locs[i]; i++) {
    if (toupper(string[0]) != locs[i][0])
      continue;
    for (j = (i + 1); locs[j]; j++)
      if (toupper(string[0]) == locs[j][0])
        break;
    if (!locs[j])
      return i;
    /* Ok, comparison between these two, then */
    c = strstr(locs[i], " ");
    d = strstr(locs[j], " ");
    if (!c && !string[1] && d)
      return i;
    if (!c && !d)
      return -1;
    if (!string[1])
      continue;
    if (c && toupper(string[1]) == *(++c))
      return i;
    if (d && toupper(string[1]) == *(++d))
      return j;
  }
  return -1;
}

int WeaponIndexFromString(BtechContext *context, char *string) {
  int id, brand;

  if (find_matching_vlong_part(context, string, nullptr, &id, &brand))
    if (equipment_is_weapon(id))
      return weapon_from_equipment_index(id);
  return -1;
}

int FindSpecialItemCodeFromString(BtechContext *context, char *buffer) {
  int id, brand;

  if (find_matching_vlong_part(context, buffer, nullptr, &id, &brand))
    if (equipment_is_special(id))
      return special_from_equipment_index(id);
  return -1;
}
