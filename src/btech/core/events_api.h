#pragma once

#include "mux/server/platform.h"

/* Administrative command that reports currently scheduled BTech events. */
void debug_EventTypes(DbRef player, void *data, char *buffer);
