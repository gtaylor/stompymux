/* Declares the BattleTech eject API. */

#include "mux/server/platform.h"

#pragma once

typedef struct BtechContext BtechContext;

/* eject.c */
typedef struct ContentsTeleportRequest {
  BtechContext *context;
  DbRef source;
  DbRef destination;
  int options;
} ContentsTeleportRequest;

int contents_teleport(const ContentsTeleportRequest *request);
void discard_mw(Mech *mech);
void enter_mw_bay(Mech *mech, DbRef bay);
void pickup_mw(Mech *mech, Mech *target);
void mech_eject(DbRef player, void *data, char *buffer);
void mech_disembark(DbRef player, void *data, char *buffer);
void mech_udisembark(DbRef player, void *data, const char *buffer);
void mech_embark(DbRef player, void *data, char *buffer);
void autoeject(DbRef player, Mech *mech, int t_is_b_suit);
