
/*
   p.failures.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:41 CET 1999 from failures.c */

#pragma once

#include "mux/server/platform.h"

#include "mech.h"

/* failures.c */
int GetBrandIndex(int type);
char *GetPartBrandName(int type, int level);
void FailureRadioStatic(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type);
void FailureRadioShort(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type);
void FailureRadioRange(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type);
void FailureComputerShutdown(Mech *mech, int weapnum, int weaptype, int section,
                             int critical, int roll, int *modifier, int *type);
void FailureComputerScanner(Mech *mech, int weapnum, int weaptype, int section,
                            int critical, int roll, int *modifier, int *type);
void FailureComputerTarget(Mech *mech, int weapnum, int weaptype, int section,
                           int critical, int roll, int *modifier, int *type);
void FailureWeaponMissiles(Mech *mech, int weapnum, int weaptype, int section,
                           int critical, int roll, int *modifier, int *type);
void FailureWeaponDud(Mech *mech, int weapnum, int weaptype, int section,
                      int critical, int roll, int *modifier, int *type);
void FailureWeaponJammed(Mech *mech, int weapnum, int weaptype, int section,
                         int critical, int roll, int *modifier, int *type);
void FailureWeaponRange(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type);
void FailureWeaponDamage(Mech *mech, int weapnum, int weaptype, int section,
                         int critical, int roll, int *modifier, int *type);
void FailureWeaponHeat(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int roll, int *modifier, int *type);
void FailureWeaponSpike(Mech *mech, int weapnum, int weaptype, int section,
                        int critical, int roll, int *modifier, int *type);
void CheckGenericFail(Mech *mech, int type, int *result, int *mod);
void CheckWeaponFailed(Mech *mech, int weapnum, int weaptype, int section,
                       int critical, int *modifier, int *type);
