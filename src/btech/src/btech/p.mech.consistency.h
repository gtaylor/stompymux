
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
int susp_factor(MECH *mech);
int engine_weight(MECH *mech);
int mech_weight_sub_mech(DbRef player, MECH *mech, int interactive);
int mech_weight_sub_veh(DbRef player, MECH *mech, int interactive);
int mech_weight_sub(DbRef player, MECH *mech, int interactive);
void mech_weight(DbRef player, void *data, char *buffer);
void vehicle_int_check(MECH *mech, int noisy);
void mech_int_check(MECH *mech, int noisy);
int crit_weight(MECH *mech, int t);
