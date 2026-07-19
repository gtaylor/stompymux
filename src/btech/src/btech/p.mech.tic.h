
/*
   p.mech.tic.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Fri Jan 15 15:33:01 CET 1999 from mech.tic.c */

#pragma once

/* mech.tic.c */
int cleartic_sub_func(MECH *mech, DbRef player, int low, int high,
                      void *context);
void cleartic_sub(DbRef player, MECH *mech, char *buffer);
int addtic_sub_func(MECH *mech, DbRef player, int low, int high, void *context);
void addtic_sub(DbRef player, MECH *mech, char *buffer);
int deltic_sub_func(MECH *mech, DbRef player, int low, int high, void *context);
void deltic_sub(DbRef player, MECH *mech, char *buffer);
int firetic_sub_func(MECH *mech, DbRef player, int low, int high,
                     void *context);
void firetic_sub(DbRef player, MECH *mech, char *buffer);
void listtic_sub(DbRef player, MECH *mech, char *buffer);
void mech_cleartic(DbRef player, void *data, char *buffer);
void mech_addtic(DbRef player, void *data, char *buffer);
void mech_deltic(DbRef player, void *data, char *buffer);
void mech_firetic(DbRef player, void *data, char *buffer);
void mech_listtic(DbRef player, void *data, char *buffer);
void heat_cutoff(DbRef player, void *data, char *buffer);
