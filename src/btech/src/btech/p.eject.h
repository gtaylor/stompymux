
/*
   p.eject.h

   Automatically created by protomaker (C) 1998 Markus Stenberg (fingon@iki.fi)
   Protomaker is actually only a wrapper script for cproto, but well.. I like
   fancy headers and stuff :)
   */

/* Generated at Wed Feb 17 23:36:31 CET 1999 from eject.c */

#include "mux/server/platform.h"

#pragma once

/* eject.c */
int tele_contents(DbRef from, DbRef to, int flag);
void discard_mw(MECH *mech);
void enter_mw_bay(MECH *mech, DbRef bay);
void pickup_mw(MECH *mech, MECH *target);
void mech_eject(DbRef player, void *data, char *buffer);
void mech_disembark(DbRef player, void *data, char *buffer);
void mech_udisembark(DbRef player, void *data, char *buffer);
void mech_embark(DbRef player, void *data, char *buffer);
void autoeject(DbRef player, MECH *mech, int tIsBSuit);
