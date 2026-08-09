/* Declares the BattleTech economy cmds API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

/* econ_cmds.c */
void mech_cargo_weight_recalculate(Mech *mech);
int loading_bay_whine(DbRef player, DbRef cargobay, Mech *mech);
void econ_fix_stuff(BtechContext *context, DbRef player, DbRef loc);
void mech_Rfixstuff(DbRef player, void *data, char *buffer);
void list_matching(BtechContext *context, DbRef player, char *header, DbRef loc,
                   char *buf);
void mech_manifest(DbRef player, void *data, char *buffer);
void mech_stores(DbRef player, void *data, char *buffer);
void mech_Raddstuff(DbRef player, void *data, char *buffer);
void mech_Rremovestuff(DbRef player, void *data, char *buffer);
void mech_loadcargo(DbRef player, void *data, char *buffer);
void mech_unloadcargo(DbRef player, void *data, char *buffer);
void mech_Rresetstuff(DbRef player, void *data, char *buffer);
