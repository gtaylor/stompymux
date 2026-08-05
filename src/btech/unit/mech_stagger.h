#pragma once

#include <stdbool.h>
#include <time.h>

#include "mech_api_types.h"

typedef struct {
  int amount;
  time_t occurred_at;
  DbRef attacker;
  bool counted;
} MechStaggerDamageSnapshot;

bool mech_stagger_damage_history_is_empty(const Mech *mech);
bool mech_stagger_damage_append(Mech *mech, int amount, time_t occurred_at,
                                DbRef attacker, bool counted);
void mech_stagger_damage_mark(Mech *mech, int stagger_level);
void mech_stagger_damage_remove(Mech *mech, int stagger_level);
void mech_stagger_damage_clear(Mech *mech);
void mech_stagger_damage_expire(Mech *mech, time_t now);
int mech_stagger_damage_current(const Mech *mech, time_t now);
int mech_stagger_damage_current_counted(const Mech *mech, time_t now);
bool mech_stagger_damage_get(const Mech *mech, int index,
                             MechStaggerDamageSnapshot *snapshot);
void mech_stagger_tracking_reset(Mech *mech);
int mech_stagger_level(const Mech *mech);
