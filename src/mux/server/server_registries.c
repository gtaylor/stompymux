/* server_registries.c - Cohesive command, world, and access indexes. */

#include "mux/server/server_registries.h"

#include <assert.h>
#include <string.h>

void command_registry_initialize(CommandRegistry *registry) {
  assert(registry != nullptr);
  memset(registry, 0, sizeof(*registry));
}

void world_indexes_initialize(WorldIndexes *indexes) {
  assert(indexes != nullptr);
  memset(indexes, 0, sizeof(*indexes));
}

void access_control_store_initialize(AccessControlStore *store) {
  assert(store != nullptr);
  memset(store, 0, sizeof(*store));
}
