/** @file
 * Telnet protocol lifecycle and receive interface.
 */
#pragma once

#include <stddef.h>

#include "mux/network/telnet_environment.h"

/** Initializes descriptor telnet. @param[out] d D. */

bool descriptor_telnet_initialize(Descriptor *d);
/** Destroys descriptor telnet. @param[in,out] d D. */

void descriptor_telnet_destroy(Descriptor *d);
/** Executes descriptor telnet receive. @param[in,out] d D. @param[in] buffer
 * Caller-owned output storage. @param[in] size Storage size in bytes. */

void descriptor_telnet_receive(Descriptor *d, const char *buffer, size_t size);
/** Sets echo on descriptor telnet. @param[in,out] d D. @param[in] echo Echo. */

void descriptor_telnet_set_echo(Descriptor *d, int echo);
