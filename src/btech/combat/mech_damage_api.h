
/* p.mech.damage.h */

#pragma once

#include "mux/server/platform.h"

int cause_armordamage(Mech *wounded, Mech *attacker, int LOS, int attackPilot,
                      int isrear, int iscritical, int hitloc, int damage,
                      int *crits, int wWeapIndx, int wAmmoMode);
int cause_internaldamage(Mech *wounded, Mech *attacker, int LOS,
                         int attackPilot, int isrear, int hitloc, int intDamage,
                         int weapindx, int *crits);
void DamageMech(Mech *wounded, Mech *attacker, int LOS, int attackPilot,
                int hitloc, int isrear, int iscritical, int damage,
                int intDamage, int cause, int bth, int wWeapIndx, int wAmmoMode,
                int tIgnoreSwarmers);
void DestroyWeapon(Mech *wounded, int hitloc, int type, int startCrit,
                   int numcrits, int totalcrits);
int CountWeaponsInLoc(Mech *mech, int loc);
int FindWeaponTypeNumInLoc(Mech *mech, int loc, int num);
void LoseWeapon(Mech *mech, int hitloc);
void DestroyHeatSink(Mech *mech, int hitloc);
void DestroySection(Mech *wounded, Mech *attacker, int LOS, int hitloc);
char *setarmorstatus_func(Mech *mech, char *sectstr, char *typestr,
                          char *valuestr);
int dodamage_func(DbRef player, Mech *mech, int totaldam, int clustersize,
                  int direction, int critical, char *mechmsg,
                  char *mechbroadcast);
void mech_damage(DbRef player, Mech *mech, char *buffer);
void mech_damage_section(DbRef player, Mech *mech, char *buffer);
