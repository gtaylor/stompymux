/* telnet_handler.h - Telnet protocol lifecycle and receive interface. */

#pragma once

#include <stddef.h>

#include "mux/network/telnet_environment.h"

bool descriptor_telnet_initialize(Descriptor *d);
void descriptor_telnet_destroy(Descriptor *d);
void descriptor_telnet_receive(Descriptor *d, const char *buffer, size_t size);
void descriptor_telnet_set_echo(Descriptor *d, int echo);
