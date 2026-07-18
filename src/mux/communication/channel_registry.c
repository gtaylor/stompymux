/* channel_registry.c - Owned channel index shared by comsys and persistence. */

#include "mux/communication/channel_registry.h"

#include <assert.h>

#include "mux/communication/commac.h"

void channel_registry_initialize(ChannelRegistry *registry) {
  assert(registry != nullptr);
  hash_table_initialize(&registry->channels, 30 * HASH_FACTOR);
  registry->count = 0;
}

void channel_registry_destroy(ChannelRegistry *registry) {
  assert(registry != nullptr);
  for (int bucket = 0; bucket < COMMAC_BUCKET_COUNT; bucket++) {
    Commac *entry = registry->commacs[bucket];
    while (entry != nullptr) {
      Commac *next = entry->next;
      destroy_commac(entry);
      entry = next;
    }
    registry->commacs[bucket] = nullptr;
  }
  hash_table_flush(&registry->channels, 0);
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
