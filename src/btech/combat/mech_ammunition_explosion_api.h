#pragma once

#include "mech_api_types.h"

typedef struct AmmunitionExplosionRequest {
  Mech *attacker;
  Mech *target;
  CriticalSlotReference ammunition;
  int damage;
} AmmunitionExplosionRequest;

void mech_ammunition_explode(const AmmunitionExplosionRequest *request);
