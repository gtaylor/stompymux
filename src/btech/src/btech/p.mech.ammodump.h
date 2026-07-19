
/* p.mech.ammodump.h */

#pragma once

int Dump_Decrease(MECH *mech, int loc, int pos, int *hm);
void mech_dump(DbRef player, void *data, char *buffer);
void BlowDumpingAmmo(MECH *mech, MECH *attacker, int wHitLoc);
typedef struct BtechContext BtechContext;

int FindMaxAmmoDamage(BtechContext *context, int wWeapIdx);

struct objDumpingAmmo {
  int wDamage;
  int wLocation;
  int wSlot;
  int wWeapIdx;
  int wPartType;
};
