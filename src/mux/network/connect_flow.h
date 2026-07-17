/* connect_flow.h - Connect/create login flow for not-yet-authenticated
 * descriptors. */

#pragma once

#include "mux/network/descriptor.h"

void descriptor_start_connect_flow(Descriptor *descriptor);
