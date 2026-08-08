
/*
   p.eject.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Wed Feb 17 23:36:31 CET 1999 from eject.c */

#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;

/* eject.c */
int tele_contents(BtechContext *context, DbRef from, DbRef to, int flag);
void discard_mw(Mech *mech);
void enter_mw_bay(Mech *mech, DbRef bay);
void pickup_mw(Mech *mech, Mech *target);
void mech_eject(DbRef player, void *data, char *buffer);
void mech_disembark(DbRef player, void *data, char *buffer);
void mech_udisembark(DbRef player, void *data, const char *buffer);
void mech_embark(DbRef player, void *data, char *buffer);
void autoeject(DbRef player, Mech *mech, int tIsBSuit);
