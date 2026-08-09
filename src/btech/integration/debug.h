
/* Declares debug special-object callbacks. */

#pragma once

#include "mux/server/platform.h"
#include "special_object.h"

void debug_allocfree(DbRef key, void **data,
                     BtechSpecialLifecycleOperation operation);
void debug_list(DbRef player, void *data, char *buffer);
void debug_savedb(DbRef player, void *data, char *buffer);
void debug_shutdown(DbRef player, void *data, char *buffer);
