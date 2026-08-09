/* Declares the BattleTech debug API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

/* debug.c */
void debug_list(DbRef player, void *data, char *buffer);
void debug_savedb(DbRef player, void *data, char *buffer);
void debug_memory(DbRef player, void *data, char *buffer);
void ShutDownMap(BtechContext *context, DbRef player, DbRef mapnumber);
void debug_shutdown(DbRef player, void *data, char *buffer);
void debug_setvrt(DbRef player, void *data, char *buffer);
void debug_setwbv(DbRef player, void *data, char *buffer);
