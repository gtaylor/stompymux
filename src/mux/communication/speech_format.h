/** @file
 * Public MUX communication interface for speech format.
 */
#pragma once

#include "mux/support/alloc.h"

#include <stdbool.h>

/** Executes speech format say. @param[out] buffer Caller-owned output storage.
 * @param[in] speaker Speaker. @param[in] message Message. */

void speech_format_say(char buffer[static LBUF_SIZE], const char *speaker,
                       const char *message);
/** Executes speech format pose. @param[out] buffer Caller-owned output storage.
 * @param[in] speaker Speaker. @param[in] message Message. @param[in] spaced
 * Spaced. */

void speech_format_pose(char buffer[static LBUF_SIZE], const char *speaker,
                        const char *message, bool spaced);
