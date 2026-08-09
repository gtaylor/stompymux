/* Declares the BattleTech unit restrict API. */

#pragma once

#include "mux/server/platform.h"
#include "special_object.h"

/* mech.restrict.c */
void clear_mech_from_LOS(Mech *mech);
void mech_Rsetxy(DbRef player, void *data, char *buffer);
void mech_Rsetmapindex(DbRef player, void *data, char *buffer);
void mech_Rsetteam(DbRef player, void *data, char *buffer);
void newfreemech(DbRef key, void **data,
                 BtechSpecialLifecycleOperation operation);
