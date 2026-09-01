/** @file
 * Owned channel index shared by comsys and persistence.
 */
#pragma once

#include <stdint.h>

#include "mux/support/hash_table.h"

struct ChannelRegistry; // IWYU pragma: keep

typedef struct ChannelRegistry ChannelRegistry;
typedef struct Commac Commac;
constexpr int COMMAC_BUCKET_COUNT = 500;

struct ChannelRegistry {
  HashTable channels;
  Commac *commacs[COMMAC_BUCKET_COUNT];
  int count;
  uint64_t next_generation;
};

/** Initializes channel registry. @param[out] registry Registry to use. */

void channel_registry_initialize(ChannelRegistry *registry);
/** Destroys channel registry. @param[in,out] registry Registry to use. */

void channel_registry_destroy(ChannelRegistry *registry);
/** Executes channel registry reset statistics. @param[in,out] registry Registry
 * to use. */

void channel_registry_reset_statistics(ChannelRegistry *registry);
/** Finds channel registry find. @param[in] registry Registry to use. @param[in]
 * name Name to use. */

void *channel_registry_find(ChannelRegistry *registry, const char *name);
/** Claims a nonzero identity generation for a newly registered channel.
 * @param[in,out] registry Registry to use.
 * @return The claimed generation. */

uint64_t channel_registry_claim_generation(ChannelRegistry *registry);
/** Returns channel registry bucket at. @param[in] registry Registry to use.
 * @param[in] bucket Bucket. */

Commac *channel_registry_bucket_at(const ChannelRegistry *registry,
                                   size_t bucket);
/** Sets channel registry bucket. @param[in,out] registry Registry to use.
 * @param[in] bucket Bucket. @param[in,out] entry Entry. */

void channel_registry_bucket_set(ChannelRegistry *registry, size_t bucket,
                                 Commac *entry);
