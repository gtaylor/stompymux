
/*
   p.mech.update.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 08:51:16 CET 1999 from mech.update.c */

#pragma once

#include "mux/server/platform.h"

/* mech.update.c */
int fiery_death(Mech *mech);
int bridge_w_elevation(Mech *mech);
void bridge_set_elevation(Mech *mech);
int DSOkToNotify(Mech *mech);
int collision_check(Mech *mech, int mode, int le, int lt);
void move_mech(Mech *mech);
void CheckNavalHeight(Mech *mech, int oz);
void CheckVTOLHeight(Mech *mech);
void UpdateHeading(Mech *mech);
float terrain_speed(Mech *mech, float tempspeed, float maxspeed, int terrain,
                    int elev);
void UpdateSpeed(Mech *mech);
int OverheatMods(Mech *mech);
void ammo_explosion(Mech *attacker, Mech *mech, int ammoloc, int ammocritnum,
                    int damage);
void HandleOverheat(Mech *mech);
void UpdateHeat(Mech *mech);
int recycle_weaponry(Mech *mech);
int SkidMod(float Speed);
void NewHexEntered(Mech *mech, BattleMap *mech_map, float deltax, float deltay,
                   int last_z);
void MarkStaggerDamage(Mech *mech, int staggerLevel);
void RemoveStaggerDamage(Mech *mech, int staggerLevel);
void ClearAllStaggerDamage(Mech *mech);
void ClearStaggerDamage(Mech *mech);
int CurrentStaggerDamage(Mech *mech);
int CurrentCountedStaggerDamage(Mech *mech);
void CheckDamage(Mech *wounded);
void UpdatePilotSkillRolls(Mech *mech);
void updateAutoturnTurret(Mech *mech);
void mech_update(DbRef key, void *data);
