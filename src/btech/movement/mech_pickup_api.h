#pragma once

#include "mux/server/platform.h"

void mech_pickup(DbRef player, Mech *mech, char *buffer);
void mech_attachcables(DbRef player, Mech *mech, char *buffer);
void mech_detachcables(DbRef player, Mech *mech, char *buffer);
void mech_dropoff(DbRef player, Mech *mech, const char *buffer);
