/** @file
 * Connect/create login flow for not-yet-authenticated descriptors.
 */
#pragma once

#include "mux/network/descriptor.h"

typedef struct LoginThrottle LoginThrottle;

/** Creates login throttle. */

LoginThrottle *login_throttle_create(void);
/** Destroys login throttle. @param[in,out] throttle Throttle. */

void login_throttle_destroy(LoginThrottle *throttle);
/** Executes descriptor start connect flow. @param[in,out] descriptor Network
 * descriptor. */

void descriptor_start_connect_flow(Descriptor *descriptor);
