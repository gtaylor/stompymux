
/*
   p.mech.tech.commands.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Tue Feb  9 14:31:36 CET 1999 from mech.tech.commands.c */

#pragma once

/* mech.tech.commands.c */
int SomeoneRepairing_s(MECH *mech, int loc, int part, int t);
int SomeoneRepairing(MECH *mech, int loc, int part);
int SomeoneReplacingSuit(MECH *mech, int loc);
int SomeoneFixingA(MECH *mech, int loc);
int SomeoneFixingI(MECH *mech, int loc);
int SomeoneFixing(MECH *mech, int loc);
int SomeoneAttaching(MECH *mech, int loc);
int SomeoneResealing(MECH *mech, int loc);
int SomeoneScrappingLoc(MECH *mech, int loc);
int SomeoneScrappingPart(MECH *mech, int loc, int part);
int CanScrapLoc(MECH *mech, int loc);
int CanScrapPart(MECH *mech, int loc, int part);
int ValidGunPos(MECH *mech, int loc, int pos);
void tech_checkstatus(DbRef player, void *data, char *buffer);
void tech_removegun(DbRef player, void *data, char *buffer);
void tech_removepart(DbRef player, void *data, char *buffer);
int Invalid_Scrap_Path(MECH *mech, int loc);
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
int Invalid_Repair_Path(MECH *mech, int loc);
int unit_is_fixable(MECH *mech);
void tech_reattach(DbRef player, void *data, char *buffer);
void tech_reseal(DbRef player, void *data, char *buffer);
void tech_magic(DbRef player, void *data, char *buffer);
void tech_fixextra(DbRef player, void *data, char *buffer);
void tech_replacesuit(DbRef player, void *data, char *buffer);
