/* Declares the BattleTech unit restrict API. */

#pragma once

#include "mux/server/platform.h"
#include "special_object.h"

/* mech.restrict.c */
void clear_mech_from_los(Mech *mech);
void mech_rsetxy(DbRef player, void *data, char *buffer);
void mech_rsetmapindex(DbRef player, void *data, char *buffer);
void mech_rsetteam(DbRef player, void *data, char *buffer);
void newfreemech(DbRef key, void **data,
                 BtechSpecialLifecycleOperation operation);
