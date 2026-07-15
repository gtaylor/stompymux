
/*
   p.econ_cmds.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:32:40 CET 1999 from econ_cmds.c */

#pragma once

/* econ_cmds.c */
void SetCargoWeight(MECH *mech);
int loading_bay_whine(DbRef player, DbRef cargobay, MECH *mech);
void mech_Rfixstuff(DbRef player, void *data, char *buffer);
void list_matching(DbRef player, char *header, DbRef loc, char *buf);
void mech_manifest(DbRef player, void *data, char *buffer);
void mech_stores(DbRef player, void *data, char *buffer);
void mech_Raddstuff(DbRef player, void *data, char *buffer);
void mech_Rremovestuff(DbRef player, void *data, char *buffer);
void mech_loadcargo(DbRef player, void *data, char *buffer);
void mech_unloadcargo(DbRef player, void *data, char *buffer);
void mech_Rresetstuff(DbRef player, void *data, char *buffer);
