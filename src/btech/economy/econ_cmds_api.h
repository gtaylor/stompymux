/* Declares the BattleTech economy cmds API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;
typedef struct BtechSpecialObject BtechSpecialObject;

/* econ_cmds.c */
void mech_cargo_weight_recalculate(Mech *mech);
typedef struct LoadingBayCheck {
  DbRef actor;
  DbRef cargo_bay;
  Mech *mech;
} LoadingBayCheck;
bool loading_bay_blocks_transfer(const LoadingBayCheck *check);
bool mech_cargo_command_access(BtechContext *context, DbRef player);

typedef struct EconomyRepairRequest {
  BtechContext *context;
  DbRef actor;
  DbRef location;
} EconomyRepairRequest;
void economy_manifest_repair(const EconomyRepairRequest *request);
void mech_rfixstuff(DbRef player, BtechSpecialObject *object, char *buffer);
void list_matching(BtechContext *context, DbRef player, char *header, DbRef loc,
                   const char *buf);
void mech_manifest(DbRef player, BtechSpecialObject *object, char *buffer);
void mech_stores(DbRef player, Mech *mech, char *buffer);
void mech_raddstuff(DbRef player, BtechSpecialObject *object, char *buffer);
void mech_rremovestuff(DbRef player, BtechSpecialObject *object, char *buffer);
void mech_loadcargo(DbRef player, Mech *mech, char *buffer);
void mech_unloadcargo(DbRef player, Mech *mech, char *buffer);
void mech_rresetstuff(DbRef player, BtechSpecialObject *object, char *buffer);
