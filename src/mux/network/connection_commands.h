/** @file
 * Connected-player queries and network command dispatch.
 */
#pragma once

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/runtime_clock.h"

typedef struct GameDatabase GameDatabase;
typedef struct RuntimeClock RuntimeClock;

/** Executes make portlist. @param[in,out] descriptors Descriptors. @param[in]
 * target Target object or value. @param[out] buffer Caller-owned output
 * storage. @param[in,out] bufc Bufc. */

void make_portlist(DescriptorRegistry *descriptors, DbRef target, char *buffer,
                   char **bufc);
/** Executes fetch idle. @param[in,out] descriptors Descriptors. @param[in,out]
 * clock Clock. @param[in] target Target object or value. */

int fetch_idle(DescriptorRegistry *descriptors, RuntimeClock *clock,
               DbRef target);
/** Executes fetch connect. @param[in,out] descriptors Descriptors.
 * @param[in,out] clock Clock. @param[in] target Target object or value. */

int fetch_connect(DescriptorRegistry *descriptors, RuntimeClock *clock,
                  DbRef target);
/** Executes descriptor command. @param[in,out] descriptor Network descriptor.
 * @param[in,out] command Command text or descriptor. */

bool descriptor_command(Descriptor *descriptor, char *command);
