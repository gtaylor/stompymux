/*
 * netcommon.c
 */

/*
 * This file contains routines used by the networking code that do not
 * depend on the implementation of the networking code.  The network-specific
 * portions of the descriptor data structure are not used.
 */

#include "mux/network/network_time.h"

#include "mux/network/descriptor.h"
#include "mux/server/server_config.h" // IWYU pragma: keep

struct timeval timeval_sub(struct timeval now, struct timeval then) {
  now.tv_sec -= then.tv_sec;
  now.tv_usec -= then.tv_usec;
  if (now.tv_usec < 0) {
    now.tv_usec += 1000000;
    now.tv_sec--;
  }
  return now;
}

/*
 * ---------------------------------------------------------------------------
 * * msec_diff: return difference between two times in msec
 */

int msec_diff(struct timeval now, struct timeval then) {
  return (int)((now.tv_sec - then.tv_sec) * 1000 +
               (now.tv_usec - then.tv_usec) / 1000);
}

/*
 * ---------------------------------------------------------------------------
 * * msec_add: add milliseconds to a timeval
 */

struct timeval msec_add(struct timeval t, int x) {
  t.tv_sec += x / 1000;
  t.tv_usec += (x % 1000) * 1000;
  if (t.tv_usec >= 1000000) {
    t.tv_sec += t.tv_usec / 1000000;
    t.tv_usec = t.tv_usec % 1000000;
  }
  return t;
}

/*
 * ---------------------------------------------------------------------------
 * * update_quotas: Refill command quotas
 */

struct timeval update_quotas(const ServerConfiguration *configuration,
                             DescriptorRegistry *descriptors,
                             struct timeval last, struct timeval current) {
  int nslices;
  Descriptor *d;
  DescriptorIterator iterator = descriptor_iterator_all(descriptors);

  nslices =
      msec_diff(current, last) / (configuration->command_quota_interval > 0
                                      ? configuration->command_quota_interval
                                      : 1);

  if (nslices > 0) {
    while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
      if (d->is_dead)
        continue;
      d->quota += configuration->command_quota_increment * nslices;
      if (d->quota > configuration->command_quota_max)
        d->quota = configuration->command_quota_max;
    }
  }
  return msec_add(last, nslices * configuration->command_quota_interval);
}

/* raw_notify_raw: write a message to a player without the newline */
