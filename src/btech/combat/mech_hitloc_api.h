
/*
   p.mech.hitloc.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Tue Feb  9 14:31:33 CET 1999 from mech.hitloc.c */

#pragma once

#include "mux/server/platform.h"

/* mech.hitloc.c */
int FindPunchLocation(Mech *target, int hitGroup);
int FindKickLocation(Mech *target, int hitGroup);
int get_bsuit_hitloc(Mech *mech);
int TransferTarget(Mech *mech, int hitloc);
int crittable(Mech *m, int loc, int tres);
int FindHitLocation(Mech *mech, int hitGroup, int *iscritical, int *isrear);
int FindFasaHitLocation(Mech *mech, int hitGroup, int *iscritical, int *isrear);
void mech_motive_system_hit(Mech *mech, int wRollMod);
int FindAdvFasaVehicleHitLocation(Mech *mech, int hitGroup, int *iscritical,
                                  int *isrear);
int findNARCHitLoc(Mech *mech, Mech *hitMech, int *tIsRearHit);
int FindTargetHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical);
int FindTCHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical);
int FindAimHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical);
int FindAreaHitGroup(Mech *mech, Mech *target);
