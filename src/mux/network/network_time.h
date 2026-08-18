/** @file
 * Network timing and descriptor quota helpers.
 */
#pragma once

#include "mux/world/world_context.h"
#include <sys/time.h>

#include "mux/server/server_config.h"

struct timeval;

typedef struct DescriptorRegistry DescriptorRegistry;
typedef struct ServerConfiguration ServerConfiguration;

/** Executes timeval sub. @param[in] now Now. @param[in] then Then. */

struct timeval timeval_sub(struct timeval now, struct timeval then);
/** Executes msec diff. @param[in] now Now. @param[in] then Then. */

int msec_diff(struct timeval now, struct timeval then);
/** Adds msec. @param[in] time Time. @param[in] x X. */

struct timeval msec_add(struct timeval time, int x);
/** Executes update quotas. @param[in] configuration Server configuration.
 * @param[in,out] descriptors Descriptors. @param[in] last Last. @param[in]
 * current Current. */

struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current);
