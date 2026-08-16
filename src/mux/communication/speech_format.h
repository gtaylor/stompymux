#pragma once

#include "mux/support/alloc.h"

#include <stdbool.h>

void speech_format_say(char buffer[static LBUF_SIZE], const char *speaker,
                       const char *message);
void speech_format_pose(char buffer[static LBUF_SIZE], const char *speaker,
                        const char *message, bool spaced);
