/** @file
 * Connection lifecycle event interface.
 */
#pragma once

#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;

/** Executes descriptor welcome. @param[in,out] descriptor Network descriptor.
 */

void descriptor_welcome(Descriptor *descriptor);
/** Sets lastsite. @param[in,out] descriptor Network descriptor. @param[in,out]
 * lastsite Lastsite. */

void set_lastsite(Descriptor *descriptor, char *lastsite);
/** Executes announce connect. @param[in] player Player object. @param[in,out]
 * descriptor Network descriptor. */

void announce_connect(DbRef player, Descriptor *descriptor);
/** Executes descriptor announce disconnect. @param[in] player Player object.
 * @param[in,out] descriptor Network descriptor. @param[in] reason Reason. */

void descriptor_announce_disconnect(DbRef player, Descriptor *descriptor,
                                    const char *reason);
/** Executes boot off. @param[in,out] descriptors Descriptors. @param[in] player
 * Player object. @param[in] message Message. */

int boot_off(DescriptorRegistry *descriptors, DbRef player,
             const char *message);
/** Executes boot by port. @param[in,out] descriptors Descriptors. @param[in]
 * port Port. @param[in] no_god No god. @param[in,out] message Message. */

int boot_by_port(DescriptorRegistry *descriptors, int port, int no_god,
                 char *message);
