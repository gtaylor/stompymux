/* Connected-player queries and network command dispatch. */

#pragma once

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"

typedef struct GameDatabase GameDatabase;
typedef struct RuntimeClock RuntimeClock;

void make_portlist(DescriptorRegistry *descriptors, DbRef target, char *buffer,
                   char **cursor);
int fetch_idle(DescriptorRegistry *descriptors, RuntimeClock *clock,
               DbRef target);
int fetch_connect(DescriptorRegistry *descriptors, RuntimeClock *clock,
                  DbRef target);
int descriptor_command(Descriptor *descriptor, char *command);
