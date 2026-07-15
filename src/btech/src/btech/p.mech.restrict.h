
/*
   p.mech.restrict.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Mon Mar 22 10:40:20 CET 1999 from mech.restrict.c */

#pragma once

/* mech.restrict.c */
void clear_mech_from_LOS(MECH *mech);
void mech_Rsetxy(DbRef player, void *data, char *buffer);
void mech_Rsetmapindex(DbRef player, void *data, char *buffer);
void mech_Rsetteam(DbRef player, void *data, char *buffer);
void newfreemech(DbRef key, void **data, int selector);
