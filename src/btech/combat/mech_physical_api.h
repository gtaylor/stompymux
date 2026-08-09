/* Declares the BattleTech unit physical API. */

#pragma once

#include "mech_physical.h"
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
void mech_kickortrip(DbRef player, void *data, char *buffer,
                     PhysicalAttackType attack_type);
void mech_charge(DbRef player, void *data, char *buffer);
const char *phys_form(PhysicalAttackType attack_type, int add_s);
void phys_succeed(Mech *mech, Mech *target, PhysicalAttackType attack_type);
void phys_fail(Mech *mech, Mech *target, PhysicalAttackType attack_type);
void PhysicalAttack(Mech *mech, int damageweight, int baseToHit,
                    PhysicalAttackType attack_type, int argc, char **args,
                    BattleMap *mech_map, int sect);
void PhysicalTrip(Mech *mech, Mech *target);
void PhysicalDamage(Mech *mech, Mech *target, int weightdmg,
                    PhysicalAttackType attack_type, int sect, int glance);
int DeathFromAbove(Mech *mech, Mech *target);
void ChargeMech(Mech *mech, Mech *target);
int checkGrabClubLocation(Mech *mech, int section, int emit);
void mech_grabclub(DbRef player, void *data, char *buffer);
