#pragma once

#include "mux/server/platform.h"

void mech_pickup(DbRef player, void *data, char *buffer);
void mech_attachcables(DbRef player, void *data, char *buffer);
void mech_detachcables(DbRef player, void *data, char *buffer);
void mech_dropoff(DbRef player, void *data, const char *buffer);
