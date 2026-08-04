
/*
   p.aero.move.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:10 CET 1999 from aero.move.c */

#pragma once

#include "mux/server/platform.h"

/* aero.move.c */
void aero_takeoff(DbRef player, void *data, char *buffer);
void DS_BlastNearbyMechsAndTrees(Mech *mech, char *hitmsg, char *hitmsg1,
                                 char *nearhitmsg, char *nearhitmsg1,
                                 char *treehitmsg, int damage);
void aero_land(DbRef player, void *data, char *buffer);
void aero_ControlEffect(Mech *mech);
void ds_BridgeHit(Mech *mech);
void aero_UpdateHeading(Mech *mech);
double length_hypotenuse(double x, double y);
double my_sqrtm(double x, double y);
void aero_UpdateSpeed(Mech *mech);
int FuelCheck(Mech *mech);
void aero_update(Mech *mech);
void aero_thrust(DbRef player, void *data, char *arg);
void aero_vheading(DbRef player, void *data, char *arg, int flag);
void aero_climb(DbRef player, Mech *mech, char *arg);
void aero_dive(DbRef player, Mech *mech, char *arg);
int ImproperLZ(Mech *mech, int x, int y);
void DS_LandWarning(Mech *mech, int serious);
void aero_checklz(DbRef player, Mech *mech, char *buffer);
