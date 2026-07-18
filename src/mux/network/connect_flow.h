/* connect_flow.h - Connect/create login flow for not-yet-authenticated
 * descriptors. */

#pragma once

#include "mux/network/descriptor.h"

typedef struct LoginThrottle LoginThrottle;

LoginThrottle *login_throttle_create(void);
void login_throttle_destroy(LoginThrottle *throttle);
void descriptor_start_connect_flow(Descriptor *descriptor);
