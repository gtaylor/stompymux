
/* p.mech.ammodump.h */

#pragma once

#include "mux/server/platform.h"

int Dump_Decrease(Mech *mech, int loc, int pos, int *hm);
void mech_dump(DbRef player, void *data, char *buffer);
void BlowDumpingAmmo(Mech *mech, Mech *attacker, int wHitLoc);
typedef struct BtechContext BtechContext;

int FindMaxAmmoDamage(BtechContext *context, int wWeapIdx);

struct objDumpingAmmo {
  int wDamage;
  int wLocation;
  int wSlot;
  int wWeapIdx;
  int wPartType;
};
