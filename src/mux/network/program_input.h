/* program_input.h - Interactive program-editor descriptor input interface. */

#pragma once

#include "mux/network/descriptor.h"

void descriptor_program_handle(Descriptor *descriptor, char *message);
void descriptor_program_clear(DbRef player);
