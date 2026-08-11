
/* p.mech.combat.misc.h */

#pragma once

#include "mech_api_types.h"
#include "mux/server/platform.h"

typedef struct AmmunitionDecrementRequest {
  Mech *mech;
  int weapon_index;
  CriticalSlotReference weapon;
  CriticalSlotLookupResult primary_ammunition;
  CriticalSlotLookupResult secondary_ammunition;
  int gatling_shots;
} AmmunitionDecrementRequest;
void mech_ammunition_decrement(const AmmunitionDecrementRequest *request);
typedef struct AmmunitionExpenditureCheck {
  Mech *mech;
  int weapon_index;
  int rounds_remaining;
} AmmunitionExpenditureCheck;
void mech_ammunition_expenditure_check(const AmmunitionExpenditureCheck *check);
void mech_heat_effect_apply(Mech *mech, Mech *temp_mech, int heatdam,
                            bool from_inferno);
void mech_inferno_hit(Mech *mech, Mech *hit_mech, int missiles, bool los);
void mech_plasma_hit(Mech *target);
void mech_contents_kill_if_in_character(Mech *mech);
void mech_destroy(Mech *target, Mech *mech, bool showboom, const char *reason);
const char *mech_hex_target_short_name(const Mech *mech);
