#include "mux/communication/speech_format.h"
#include "mux/support/alloc.h"

#include <stdio.h>

void speech_format_say(char buffer[static LBUF_SIZE], const char *speaker,
                       const char *message) {
  (void)snprintf(buffer, LBUF_SIZE, "%s says \"%s\"", speaker, message);
}

void speech_format_pose(char buffer[static LBUF_SIZE], const char *speaker,
                        const char *message, bool spaced) {
  (void)snprintf(buffer, LBUF_SIZE, spaced ? "%s %s" : "%s%s", speaker,
                 message);
}
