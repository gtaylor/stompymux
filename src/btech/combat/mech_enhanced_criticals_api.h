
/*
   p.mech.enhanced.criticals.h
*/

#pragma once

#include <stdbool.h>

#include "mux/server/platform.h"

int mech_weapon_critical_to_hit_modifier(Mech *mech, int section, int critical,
                                         int range_bracket);
int mech_weapon_critical_heat_modifier(Mech *mech, int section, int critical);
int mech_weapon_critical_damage_penalty(Mech *mech, int section, int critical);
bool mech_weapon_critical_can_explode(Mech *mech, int section, int critical,
                                      int roll);
bool mech_weapon_critical_can_jam(Mech *mech, int section, int critical,
                                  int roll);
bool mech_weapon_ammo_feed_is_locked(Mech *mech, int section, int critical);
int mech_weapon_damaged_slot_count(Mech *mech, int section, int first_critical,
                                   int weapon_size);
int mech_weapon_damaged_slot_count_at(Mech *mech, int section, int critical);
bool mech_weapon_critical_should_destroy(Mech *mech, int section, int critical,
                                         bool increment_count);
void mech_weapon_critical_apply(Mech *mech, Mech *attacker, int line_of_sight,
                                int section, int critical);
void mech_weapon_status(DbRef player, Mech *mech, char *buffer);
