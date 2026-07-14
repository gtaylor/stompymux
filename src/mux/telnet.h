#pragma once

#include <stddef.h>

#include "descriptor.h"

int telnet_initialize(DESC *d);
void telnet_destroy(DESC *d);
void telnet_receive(DESC *d, const char *buffer, size_t size);
