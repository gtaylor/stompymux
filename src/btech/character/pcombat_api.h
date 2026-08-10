/* Declares the BattleTech pcombat API. */

#pragma once

#include "mux/server/platform.h"

/* pcombat.c */
typedef struct PersonalCombatDamageConversion {
  Mech *target;
  int weapon_index;
  int damage;
} PersonalCombatDamageConversion;

typedef struct PersonalArmorDamageRequest {
  Mech *wounded;
  int cause;
  int hit_location;
  int damage;
  int damage_identifier;
} PersonalArmorDamageRequest;

int personal_combat_damage_to_unit(
    const PersonalCombatDamageConversion *conversion);
int unit_damage_to_personal_combat(
    const PersonalCombatDamageConversion *conversion);
int personal_armor_reduce_damage(const PersonalArmorDamageRequest *request);
