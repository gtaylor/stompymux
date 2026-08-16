#pragma once

#include "mux/server/platform.h"

void mech_startup(DbRef player, Mech *mech, const char *buffer);
void mech_shutdown(DbRef player, Mech *mech, const char *buffer);
