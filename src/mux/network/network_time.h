/* Network timing and descriptor quota helpers. */

#pragma once

#include <sys/time.h>

typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;

struct timeval timeval_sub(struct timeval now, struct timeval then);
int msec_diff(struct timeval now, struct timeval then);
struct timeval msec_add(struct timeval time, int milliseconds);
struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current);
