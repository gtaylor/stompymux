#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct MechDamageHistory {
  int turn_damage;
  bool staggered_last_turn;
  int stagger_stamp;
} MechDamageHistory;

MechDamageHistory mech_damage_history(const Mech *mech);
void mech_turn_damage_clear(Mech *mech);
void mech_turn_damage_add(Mech *mech, int damage);
void mech_damage_taken_add(Mech *mech, int damage);
void mech_damage_inflicted_add(Mech *mech, int damage);
void mech_stagger_stamp_set(Mech *mech, int stamp);
