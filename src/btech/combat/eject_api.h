/* Declares the BattleTech eject API. */

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
