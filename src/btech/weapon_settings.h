#pragma once

#include <stdbool.h>
#include <stddef.h>

typedef struct BtechWeaponRuntimeValues {
  int recycle_time;
  int battle_value;
} BtechWeaponRuntimeValues;

typedef struct BtechWeaponSettings {
  BtechWeaponRuntimeValues *values;
  size_t count;
} BtechWeaponSettings;

bool btech_weapon_settings_initialize(BtechWeaponSettings *settings);
void btech_weapon_settings_destroy(BtechWeaponSettings *settings);
int btech_weapon_settings_recycle_time(const BtechWeaponSettings *settings,
                                       int weapon_index);
int btech_weapon_settings_battle_value(const BtechWeaponSettings *settings,
                                       int weapon_index);
bool btech_weapon_settings_set_recycle_time(BtechWeaponSettings *settings,
                                            int weapon_index, int value);
bool btech_weapon_settings_set_battle_value(BtechWeaponSettings *settings,
                                            int weapon_index, int value);
