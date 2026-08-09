/* Declares the BattleTech unit tech commands API. */

#pragma once

#include "mux/server/platform.h"

/* mech.tech.commands.c */
int SomeoneRepairing_s(Mech *mech, int loc, int part, int t);
int SomeoneRepairing(Mech *mech, int loc, int part);
int SomeoneReplacingSuit(Mech *mech, int loc);
int SomeoneFixingA(Mech *mech, int loc);
int SomeoneFixingI(Mech *mech, int loc);
int SomeoneFixing(Mech *mech, int loc);
int SomeoneAttaching(Mech *mech, int loc);
int SomeoneResealing(Mech *mech, int loc);
int SomeoneScrappingLoc(Mech *mech, int loc);
int SomeoneScrappingPart(Mech *mech, int loc, int part);
int CanScrapLoc(Mech *mech, int loc);
int CanScrapPart(Mech *mech, int loc, int part);
int ValidGunPos(Mech *mech, int loc, int pos);
void tech_checkstatus(DbRef player, void *data, char *buffer);
void tech_removegun(DbRef player, void *data, char *buffer);
void tech_removepart(DbRef player, void *data, char *buffer);
int Invalid_Scrap_Path(Mech *mech, int loc);
void tech_removesection(DbRef player, void *data, char *buffer);
void tech_replacegun(DbRef player, void *data, char *buffer);
void tech_repairgun(DbRef player, void *data, char *buffer);
void tech_fixenhcrit(DbRef player, void *data, char *buffer);
void tech_replacepart(DbRef player, void *data, char *buffer);
void tech_repairpart(DbRef player, void *data, char *buffer);
void tech_toggletype(DbRef player, void *data, char *buffer);
void tech_reload(DbRef player, void *data, char *buffer);
void tech_unload(DbRef player, void *data, char *buffer);
void tech_fixarmor(DbRef player, void *data, char *buffer);
void tech_fixinternal(DbRef player, void *data, char *buffer);
int Invalid_Repair_Path(Mech *mech, int loc);
int unit_is_fixable(Mech *mech);
void tech_reattach(DbRef player, void *data, char *buffer);
void tech_reseal(DbRef player, void *data, char *buffer);
void tech_magic(DbRef player, void *data, char *buffer);
void tech_fixextra(DbRef player, void *data, char *buffer);
void tech_replacesuit(DbRef player, void *data, char *buffer);
