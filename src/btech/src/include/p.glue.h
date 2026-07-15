
/*
   p.glue.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Feb 22 14:59:36 CET 1999 from glue.c */

#pragma once

/* glue.c */
int HandledCommand_sub(DbRef player, DbRef location, char *command);
int HandledCommand(DbRef player, DbRef loc, char *command);
void mech_remove_from_all_maps(MECH *mech);
void mech_remove_from_all_maps_except(MECH *mech, int num);
void LoadSpecialObjects(void);
void UpdateSpecialObjects(void);
void *NewSpecialObject(long id, int type);
void CreateNewSpecialObject(DbRef player, DbRef key);
void DisposeSpecialObject(DbRef player, DbRef key);
void Dump_Mech(DbRef player, int type, char *typestr);
void DumpMechs(DbRef player);
void DumpMaps(DbRef player);
int WhichSpecial(DbRef key);
int IsMech(DbRef num);
int IsAuto(DbRef num);
int IsMap(DbRef num);
void *FindObjectsData(DbRef key);
char *center_string(char *c, int len);
void InitSpecialHash(int which);
void handle_xcode(DbRef player, DbRef obj, int from, int to);
void initialize_colorize(void);
char *colorize(DbRef player, char *from);
void mecha_notify(DbRef player, char *msg);
void mecha_notify_except(DbRef loc, DbRef player, DbRef exception, char *msg);
void list_chashstats(DbRef player);
void ResetSpecialObjects(void);
MAP *getMap(DbRef d);
MECH *getMech(DbRef d);
