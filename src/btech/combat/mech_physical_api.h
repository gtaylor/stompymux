
/*
   p.mech.physical.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Wed Feb 17 23:36:34 CET 1999 from mech.physical.c */

#pragma once

#include "mux/server/platform.h"

/* mech.physical.c */
void mech_punch(DbRef player, void *data, char *buffer);
void mech_club(DbRef player, void *data, char *buffer);
int have_axe(Mech *mech, int loc);
int have_sword(Mech *mech, int loc);
int have_mace(Mech *mech, int loc);
int have_saw(Mech *mech, int loc);
int have_claw(Mech *mech, int loc);
void mech_saw(DbRef player, void *data, char *buffer);
void mech_axe(DbRef player, void *data, char *buffer);
void mech_sword(DbRef player, void *data, char *buffer);
void mech_mace(DbRef player, void *data, char *buffer);
void mech_claw(DbRef player, void *data, char *buffer);
void mech_kick(DbRef player, void *data, char *buffer);
void mech_trip(DbRef player, void *data, char *buffer);
void mech_kickortrip(DbRef player, void *data, char *buffer, int AttackType);
void mech_charge(DbRef player, void *data, char *buffer);
char *phys_form(int AttackType, int add_s);
void phys_succeed(Mech *mech, Mech *target, int at);
void phys_fail(Mech *mech, Mech *target, int at);
void PhysicalAttack(Mech *mech, int damageweight, int baseToHit, int AttackType,
                    int argc, char **args, BattleMap *mech_map, int sect);
void PhysicalTrip(Mech *mech, Mech *target);
void PhysicalDamage(Mech *mech, Mech *target, int weightdmg, int AttackType,
                    int sect, int glance);
int DeathFromAbove(Mech *mech, Mech *target);
void ChargeMech(Mech *mech, Mech *target);
int checkGrabClubLocation(Mech *mech, int section, int emit);
void mech_grabclub(DbRef player, void *data, char *buffer);
