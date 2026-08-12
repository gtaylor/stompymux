/* Network timing and descriptor quota helpers. */

#pragma once

#include "mux/world/world_context.h"
#include <sys/time.h>

#include "mux/server/server_config.h"

struct timeval;

typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;

struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int x);
struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current);
