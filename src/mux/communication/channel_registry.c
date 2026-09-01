/* channel_registry.c - Owned channel index shared by comsys and persistence. */

#include "mux/communication/channel_registry.h"

#include <stdint.h>

#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include <stddef.h>

Commac *channel_registry_bucket_at(const ChannelRegistry *registry,
                                   size_t bucket) {
  return *(Commac *const *)checked_storage_at_const(
      (const void *)registry->commacs, COMMAC_BUCKET_COUNT,
      sizeof(*registry->commacs), bucket);
}

void channel_registry_bucket_set(ChannelRegistry *registry, size_t bucket,
                                 Commac *entry) {
  *(Commac **)checked_storage_at((void *)registry->commacs, COMMAC_BUCKET_COUNT,
                                 sizeof(*registry->commacs), bucket) = entry;
}

#include <assert.h>

#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/server/platform.h"

void channel_registry_initialize(ChannelRegistry *registry) {
  assert(registry != nullptr);
  hash_table_initialize(&registry->channels, 30 * HASH_FACTOR);
  registry->count = 0;
  registry->next_generation = 1;
}

void channel_registry_destroy(ChannelRegistry *registry) {
  assert(registry != nullptr);
  for (int bucket = 0; bucket < COMMAC_BUCKET_COUNT; bucket++) {
    Commac *entry = channel_registry_bucket_at(registry, (size_t)bucket);
    while (entry != nullptr) {
      Commac *next = entry->next;
      destroy_commac(entry);
      entry = next;
    }
    channel_registry_bucket_set(registry, (size_t)bucket, nullptr);
  }
  if (registry->channels.tree != nullptr) {
    struct Channel *channel = hash_table_first_entry(&registry->channels);
    while (channel != nullptr) {
      channel_destroy(channel);
      channel = hash_table_next_entry(&registry->channels);
    }
  }
  hash_table_destroy(&registry->channels);
  registry->count = 0;
}

void channel_registry_reset_statistics(ChannelRegistry *registry) {
  assert(registry != nullptr);
  hash_table_reset(&registry->channels);
}

void *channel_registry_find(ChannelRegistry *registry, const char *name) {
  assert(registry != nullptr);
  return hash_table_find(name, &registry->channels);
}

uint64_t channel_registry_claim_generation(ChannelRegistry *registry) {
  assert(registry != nullptr);
  uint64_t generation = registry->next_generation++;

  if (registry->next_generation == 0)
    registry->next_generation = 1;
  return generation;
}
