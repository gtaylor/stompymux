
/* p.mech.ammodump.h */

#pragma once

int Dump_Decrease(MECH *mech, int loc, int pos, int *hm);
void mech_dump(dbref player, void *data, char *buffer);
void BlowDumpingAmmo(MECH *mech, MECH *attacker, int wHitLoc);
int FindMaxAmmoDamage(int wWeapIdx);

struct objDumpingAmmo {
  int wDamage;
  int wLocation;
  int wSlot;
  int wWeapIdx;
  int wPartType;
};
