/* connect_flow.h - Connect/create login flow for not-yet-authenticated
 * descriptors. */

#pragma once

#include "mux/network/descriptor.h"

int descriptor_begin_connect_flow(Descriptor *descriptor, char *command);
