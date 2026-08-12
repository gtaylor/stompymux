
/*
   p.mech.enhanced.criticals.h
*/

#pragma once

#include <stdbool.h>

#include "mech_api_types.h"
#include "mech_bth_api.h"
#include "mux/server/platform.h"

typedef struct WeaponCriticalToHitRequest {
  Mech *mech;
  CriticalSlotReference slot;
  WeaponRangeBracket range_bracket;
} WeaponCriticalToHitRequest;

int mech_weapon_critical_to_hit_modifier(
    const WeaponCriticalToHitRequest *request);
int mech_weapon_critical_heat_modifier(Mech *mech, int section, int critical);
int mech_weapon_critical_damage_penalty(Mech *mech, int section, int critical);
typedef struct WeaponCriticalRoll {
  Mech *mech;
  CriticalSlotReference slot;
  int roll;
} WeaponCriticalRoll;

bool mech_weapon_critical_can_explode(const WeaponCriticalRoll *request);
bool mech_weapon_critical_can_jam(const WeaponCriticalRoll *request);
bool mech_weapon_ammo_feed_is_locked(Mech *mech, int section, int critical);
int mech_weapon_damaged_slot_count(Mech *mech, int section, int w_first_crit,
                                   int w_weap_size);
int mech_weapon_damaged_slot_count_at(Mech *mech, int section, int critical);
bool mech_weapon_critical_should_destroy(Mech *mech, int section, int critical,
                                         bool increment_count);
typedef struct WeaponCriticalApplication {
  Mech *mech;
  CriticalSlotReference slot;
} WeaponCriticalApplication;

void mech_weapon_critical_apply(const WeaponCriticalApplication *application);
void mech_weapon_status(DbRef player, Mech *mech, char *buffer);
