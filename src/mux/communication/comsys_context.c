/* comsys_context.c - Channel system dependency initialization. */

#include "mux/communication/comsys_context.h"

void comsys_context_initialize(ComsysContext *context,
                               const ServerConfiguration *configuration,
                               RuntimeClock *clock,
                               ChannelRegistry *channels) {
  *context = (ComsysContext){.configuration = configuration,
                            .clock = clock,
                            .channels = channels};
}
