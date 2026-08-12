#pragma once

#include "mux/server/platform.h"

/* Administrative command that reports currently scheduled BTech events. */
void debug_event_types(DbRef player, void *data, const char *buffer);
