
/*
   p.mech.enhanced.criticals.h
*/

#pragma once

#include "mux/server/platform.h"

void getWeapData(Mech *mech, int section, int critical, int *wWeapIndex,
                 int *wWeapSize, int *wFirstCrit);
int getCritAddedBTH(Mech *mech, int section, int critical, int rangeBracket);
int getCritAddedHeat(Mech *mech, int section, int critical);
int getCritSubDamage(Mech *mech, int section, int critical);
int canWeapExplodeFromDamage(Mech *mech, int section, int critical, int roll);
int canWeapJamFromDamage(Mech *mech, int section, int critical, int roll);
int isWeapAmmoFeedLocked(Mech *mech, int section, int critical);
int countDamagedSlots(Mech *mech, int section, int wFirstCrit, int wWeapSize);
int countDamagedSlotsFromCrit(Mech *mech, int section, int critical);
int shouldDestroyWeapon(Mech *mech, int section, int critical,
                        int incrementCount);
void scoreEnhancedWeaponCriticalHit(Mech *mech, Mech *attacker, int LOS,
                                    int section, int critical);
void mech_weaponstatus(DbRef player, Mech *mech, char *buffer);
void showWeaponDamageAndInfo(DbRef player, Mech *mech, int section,
                             int critical);
