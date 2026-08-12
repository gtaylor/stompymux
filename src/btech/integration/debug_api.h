/* Declares the BattleTech debug API. */

#pragma once

#include "mux/server/platform.h"

typedef struct BtechContext BtechContext;

/* debug.c */
void debug_list(DbRef player, void *data, char *buffer);
void debug_savedb(DbRef player, void *data, char *buffer);
void debug_memory(DbRef player, void *data, const char *buffer);
typedef struct MapShutdownRequest {
  BtechContext *context;
  DbRef actor;
  DbRef map;
} MapShutdownRequest;
void map_shutdown_units(const MapShutdownRequest *request);
void debug_shutdown(DbRef player, void *data, char *buffer);
void debug_setvrt(DbRef player, void *data, char *buffer);
void debug_setwbv(DbRef player, void *data, char *buffer);
