
/*
   p.mech.fire.h
*/

#pragma once

#include "mux/server/platform.h"

void inferno_burn(Mech *mech, int time);
void vehicle_start_burn(Mech *objMech, Mech *objAttacker);
void checkVehicleInFire(Mech *objMech, int fromHexFire);
void vehicle_extinquish_fire_event(MuxEvent *e);
void vehicle_extinquish_fire(DbRef player, Mech *mech, char *buffer);
void water_extinguish_inferno(Mech *mech);
