
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
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>

#include "mech.h"
#include "p.mech.build.h"
#include "p.mech.partnames.h"
#include "p.mech.utils.h"
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
      registry->entries[index].weapon_index = Weapon2I(id);
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

void FillDefaultCriticals(MECH *mech, int index) {
  int loop;

  for (loop = 0; loop < NUM_CRITICALS; loop++) {
    MechSections(mech)[index].criticals[loop].type = EMPTY;
    MechSections(mech)[index].criticals[loop].data = 0;
    MechSections(mech)[index].criticals[loop].firemode = 0;
    MechSections(mech)[index].criticals[loop].ammomode = 0;
  }

  if (MechType(mech) == CLASS_AERO)
    switch (index) {
    case AERO_AFT:
      for (loop = 0; loop < 12; loop++)
        MechSections(mech)[index].criticals[loop].type = I2Special(HEAT_SINK);
      MechSections(mech)[index].criticals[2].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[10].type = I2Special(ENGINE);
      break;
    }
  if (MechType(mech) == CLASS_MECH)
    switch (index) {
    case HEAD:
      MechSections(mech)[index].criticals[0].type = I2Special(LIFE_SUPPORT);
      MechSections(mech)[index].criticals[1].type = I2Special(SENSORS);
      MechSections(mech)[index].criticals[2].type = I2Special(COCKPIT);
      MechSections(mech)[index].criticals[4].type = I2Special(SENSORS);
      MechSections(mech)[index].criticals[5].type = I2Special(LIFE_SUPPORT);
      break;

    case CTORSO:
      MechSections(mech)[index].criticals[0].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[1].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[2].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[3].type = I2Special(GYRO);
      MechSections(mech)[index].criticals[4].type = I2Special(GYRO);
      MechSections(mech)[index].criticals[5].type = I2Special(GYRO);
      MechSections(mech)[index].criticals[6].type = I2Special(GYRO);
      MechSections(mech)[index].criticals[7].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[8].type = I2Special(ENGINE);
      MechSections(mech)[index].criticals[9].type = I2Special(ENGINE);
      break;

    case RTORSO:
    case LTORSO:
      break;

    case LARM:
    case RARM:
    case LLEG:
    case RLEG:
      MechSections(mech)[index].criticals[0].type = I2Special(SHOULDER_OR_HIP);
      MechSections(mech)[index].criticals[1].type = I2Special(UPPER_ACTUATOR);
      MechSections(mech)[index].criticals[2].type = I2Special(LOWER_ACTUATOR);
      MechSections(mech)[index].criticals[3].type =
          I2Special(HAND_OR_FOOT_ACTUATOR);
      break;
    }
}

ArmorSectionAbbreviation armor_section_abbreviation(char type, char mtype,
                                                    int loc) {
  char **locs;
  ArmorSectionAbbreviation abbreviation = {0};
  char *cursor = abbreviation.text;
  int i;

  locs = ProperSectionStringFromType(type, mtype);
  for (i = 0; locs[loc][i]; i++)
    if (isupper(locs[loc][i]) || isdigit(locs[loc][i]))
      *(cursor++) = locs[loc][i];
  *cursor = '\0';
  return abbreviation;
}

int ArmorSectionFromString(char type, char mtype, char *string) {
  char **locs;
  int i, j;
  char *c, *d;

  if (!string[0])
    return -1;
  locs = ProperSectionStringFromType(type, mtype);
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
    if (IsWeapon(id))
      return Weapon2I(id);
  return -1;
}

int FindSpecialItemCodeFromString(BtechContext *context, char *buffer) {
  int id, brand;

  if (find_matching_vlong_part(context, buffer, nullptr, &id, &brand))
    if (IsSpecial(id))
      return Special2I(id);
  return -1;
}
