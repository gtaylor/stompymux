#pragma once

#include "mech_combat_api.h"
#include "mech_utils_api.h"

typedef struct WeaponFirePreparation {
  bool ready;
  bool swarm_attack;
  Mech *target;
  AmmunitionCheckResult ammunition;
  int base_to_hit;
  DbRef c3_reference;
  Mech *c3_mech;
} WeaponFirePreparation;

WeaponFirePreparation weapon_fire_prepare(const WeaponFireRequest *request,
                                          float range);
int weapon_fire_roll(const WeaponFireRequest *request, float range);
