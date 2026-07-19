/* comsys_context.h - Long-lived channel system dependencies. */

#pragma once

typedef struct ChannelRegistry ChannelRegistry;
typedef struct RuntimeClock RuntimeClock;
typedef struct ServerConfiguration ServerConfiguration;

typedef struct ComsysContext ComsysContext;
struct ComsysContext {
  /* Every member is borrowed from MuxServer. */
  const ServerConfiguration *configuration;
  RuntimeClock *clock;
  ChannelRegistry *channels;
};

void comsys_context_initialize(ComsysContext *context,
                               const ServerConfiguration *configuration,
                               RuntimeClock *clock, ChannelRegistry *channels);
