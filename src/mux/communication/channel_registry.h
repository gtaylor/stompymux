/* channel_registry.h - Owned channel index shared by comsys and persistence. */

#pragma once

#include "mux/support/hash_table.h"

struct ChannelRegistry; // IWYU pragma: keep

typedef struct ChannelRegistry ChannelRegistry;
typedef struct Commac Commac;
constexpr int COMMAC_BUCKET_COUNT = 500;

struct ChannelRegistry {
  HashTable channels;
  Commac *commacs[COMMAC_BUCKET_COUNT];
  int count;
};

void channel_registry_initialize(ChannelRegistry *registry);
void channel_registry_destroy(ChannelRegistry *registry);
void channel_registry_reset_statistics(ChannelRegistry *registry);
void *channel_registry_find(ChannelRegistry *registry, const char *name);
Commac *channel_registry_bucket_at(const ChannelRegistry *registry,
                                   size_t bucket);
void channel_registry_bucket_set(ChannelRegistry *registry, size_t bucket,
                                 Commac *entry);
