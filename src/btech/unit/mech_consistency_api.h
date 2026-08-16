/* Declares the BattleTech unit consistency API. */

#include "mux/server/platform.h"

#pragma once

/* mech.consistency.c */
int susp_factor(Mech *mech);
int engine_weight(Mech *mech);
int mech_weight_sub_mech(DbRef player, Mech *mech, int interactive);
int mech_weight_sub_veh(DbRef player, Mech *mech, int interactive);
int mech_weight_sub(DbRef player, Mech *mech, int interactive);
void mech_weight(DbRef player, Mech *mech, char *buffer);
void vehicle_int_check(Mech *mech, int noisy);
void mech_int_check(Mech *mech, int noisy);
int crit_weight(Mech *mech, int t);
