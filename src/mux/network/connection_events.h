/* Connection lifecycle event interface. */

#pragma once

#include "mux/network/descriptor.h"
#include "mux/objects/db.h"

typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;

void descriptor_welcome(Descriptor *descriptor);
void set_lastsite(Descriptor *descriptor, char *lastsite);
void announce_connect(DbRef player, Descriptor *descriptor);
void descriptor_announce_disconnect(DbRef player, Descriptor *descriptor,
                                    const char *reason);
int boot_off(DescriptorRegistry *descriptors, DbRef player,
             const char *message);
int boot_by_port(DescriptorRegistry *descriptors, int port, int no_god,
                 char *message);
void descriptor_reload(GameDatabase *database,
                       const ServerConfiguration *configuration,
                       DescriptorRegistry *descriptors, DbRef player);
