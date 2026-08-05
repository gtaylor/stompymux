
/*
   p.mech.fire.h
*/

#pragma once

#include "mux/server/platform.h"

void mech_inferno_burn(Mech *mech, int time);
void vehicle_fire_start(Mech *mech, Mech *attacker);
void vehicle_fire_check(Mech *mech, int from_hex_fire);
void vehicle_fire_extinguish_event(MuxEvent *event);
void vehicle_fire_extinguish(DbRef player, Mech *mech, char *buffer);
void mech_inferno_extinguish_in_water(Mech *mech);
