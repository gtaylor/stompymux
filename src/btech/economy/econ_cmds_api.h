/* Declares the BattleTech economy cmds API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

/* econ_cmds.c */
void mech_cargo_weight_recalculate(Mech *mech);
typedef struct LoadingBayCheck {
  DbRef actor;
  DbRef cargo_bay;
  Mech *mech;
} LoadingBayCheck;
bool loading_bay_blocks_transfer(const LoadingBayCheck *check);

typedef struct EconomyRepairRequest {
  BtechContext *context;
  DbRef actor;
  DbRef location;
} EconomyRepairRequest;
void economy_manifest_repair(const EconomyRepairRequest *request);
void mech_rfixstuff(DbRef player, void *data, char *buffer);
void list_matching(BtechContext *context, DbRef player, char *header, DbRef loc,
                   const char *buf);
void mech_manifest(DbRef player, void *data, char *buffer);
void mech_stores(DbRef player, void *data, char *buffer);
void mech_raddstuff(DbRef player, void *data, char *buffer);
void mech_rremovestuff(DbRef player, void *data, char *buffer);
void mech_loadcargo(DbRef player, void *data, char *buffer);
void mech_unloadcargo(DbRef player, void *data, char *buffer);
void mech_rresetstuff(DbRef player, void *data, char *buffer);
