
/*
   p.mech.combat.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 10:40:19 CET 1999 from mech.combat.c */

#pragma once

#include "mux/server/platform.h"

/* mech.combat.c */
void mech_target(DbRef player, void *data, char *buffer);
void sixth_sense_check(Mech *mech, Mech *target);
void mech_settarget(DbRef player, void *data, char *buffer);
void mech_fireweapon(DbRef player, void *data, char *buffer);
int FireWeaponNumber(DbRef player, Mech *mech, BattleMap *mech_map, int weapnum,
                     int argc, char **args, int sight);
char *hex_target_id(Mech *mech);
int canClearOrIgnite(int weapindx);
void possibly_ignite(Mech *mech, BattleMap *map, int weapindx, int ammoMode,
                     int x, int y, int intentional);
void possibly_clear(Mech *mech, BattleMap *map, int weapindx, int ammoMode,
                    int damage, int x, int y, int intentional);
void possibly_ignite_or_clear(Mech *mech, int weapindx, int ammoMode,
                              int damage, int x, int y, int intentional);
void hex_hit(Mech *mech, int x, int y, int weapindx, int ammoMode, int damage,
             int ishit);
int weapon_failure_stuff(Mech *mech, int *weapnum, int *weapindx, int *section,
                         int *critical, int *ammoLoc, int *ammoCrit,
                         int *ammoLoc1, int *ammoCrit1, int *modifier,
                         int *type, float range, int *range_ok,
                         int wGattlingShots);
void FireWeapon(Mech *mech, BattleMap *mech_map, Mech *target, int LOS,
                int weapindx, int weapnum, int section, int critical,
                float enemyX, float enemyY, int mapx, int mapy, float range,
                int indirectFire, int sight, int ishex);
int determineDamageFromHit(Mech *mech, int wSection, int wCritSlot,
                           Mech *hitMech, int hitX, int hitY, int weapindx,
                           int wGattlingShots, int wBaseWeapDamage,
                           int wAmmoMode, int type, int modifier,
                           int isTempCalc);
void HitTarget(Mech *mech, int weapindx, int wSection, int wCritSlot,
               Mech *hitMech, int hitX, int hitY, int LOS, int type,
               int modifier, int reallyhit, int bth, int wGattlingShots,
               int tIsSwarmAttack, int player_roll);
