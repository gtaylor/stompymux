/** @file
 * Long-lived channel system dependencies.
 */
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

/** Initializes comsys context. @param[out] context Operation context.
 * @param[in] configuration Server configuration. @param[in] clock Clock.
 * @param[in] channels Channels. */

void comsys_context_initialize(ComsysContext *context,
                               const ServerConfiguration *configuration,
                               RuntimeClock *clock, ChannelRegistry *channels);
