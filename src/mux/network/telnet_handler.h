/* telnet_handler.h - Telnet protocol lifecycle and receive interface. */

#pragma once

#include <stddef.h>

#include "mux/network/descriptor.h"

int descriptor_telnet_initialize(Descriptor *d);
void descriptor_telnet_destroy(Descriptor *d);
void descriptor_telnet_receive(Descriptor *d, const char *buffer, size_t size);
