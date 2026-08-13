/* server_registries.c - Cohesive command, world, and access indexes. */

#include "mux/server/server_registries.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h"
#include "mux/commands/command.h"
#include "mux/commands/command_catalog.h"
#include "mux/objects/flags.h"
#include "mux/server/server_config.h"
#include "mux/support/hash_table.h"

bool command_registry_initialize(CommandRegistry *registry) {
  assert(registry != nullptr);
  memset(registry, 0, sizeof(*registry));
  if (!command_builtin_catalog_install(registry) ||
      !command_macro_catalog_install(registry)) {
    command_registry_destroy(registry);
    return false;
  }
  return true;
}

void command_registry_destroy(CommandRegistry *registry) {
  if (registry == nullptr)
    return;
  command_catalog_release(registry);
}

void world_indexes_initialize(WorldIndexes *indexes) {
  assert(indexes != nullptr);
  memset(indexes, 0, sizeof(*indexes));
}

void world_indexes_destroy(WorldIndexes *indexes) {
  if (indexes == nullptr)
    return;
  hash_table_destroy(&indexes->powers);
  hash_table_destroy(&indexes->flags);
  if (indexes->players.tree != nullptr) {
    for (void *player = hash_table_first_entry(&indexes->players);
         player != nullptr; player = hash_table_next_entry(&indexes->players))
      free(player);
  }
  hash_table_destroy(&indexes->players);
  memset(indexes, 0, sizeof(*indexes));
}

void access_control_store_initialize(AccessControlStore *store) {
  assert(store != nullptr);
  memset(store, 0, sizeof(*store));
}

void access_control_store_destroy(AccessControlStore *store) {
  if (store == nullptr)
    return;
  SiteData *site = store->access_sites;
  while (site != nullptr) {
    SiteData *next = site->next;
    free(site);
    site = next;
  }
  site = store->suspect_sites;
  while (site != nullptr) {
    SiteData *next = site->next;
    free(site);
    site = next;
  }
  BADNAME *bad_name = store->bad_names;
  while (bad_name != nullptr) {
    BADNAME *next = bad_name->next;
    free(bad_name->name);
    free(bad_name);
    bad_name = next;
  }
  memset(store, 0, sizeof(*store));
}
