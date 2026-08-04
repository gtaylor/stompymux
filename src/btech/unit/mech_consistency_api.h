
/*
   p.mech.consistency.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:46 CET 1999 from mech.consistency.c */

#include "mux/server/platform.h"

#pragma once

/* mech.consistency.c */
int susp_factor(Mech *mech);
int engine_weight(Mech *mech);
int mech_weight_sub_mech(DbRef player, Mech *mech, int interactive);
int mech_weight_sub_veh(DbRef player, Mech *mech, int interactive);
int mech_weight_sub(DbRef player, Mech *mech, int interactive);
void mech_weight(DbRef player, void *data, char *buffer);
void vehicle_int_check(Mech *mech, int noisy);
void mech_int_check(Mech *mech, int noisy);
int crit_weight(Mech *mech, int t);
