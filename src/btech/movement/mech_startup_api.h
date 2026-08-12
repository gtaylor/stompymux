#pragma once

#include "mux/server/platform.h"

void mech_startup(DbRef player, void *data, const char *buffer);
void mech_shutdown(DbRef player, void *data, const char *buffer);
