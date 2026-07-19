/* server_registries.c - Cohesive command, world, and access indexes. */

#include "mux/server/server_registries.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "mux/commands/functions.h"
#include "mux/server/server_config.h"

static int free_numeric_hash_data(void *, void *data, int, void *) {
  free(data);
  return 1;
}

void command_registry_initialize(CommandRegistry *registry) {
  assert(registry != nullptr);
  memset(registry, 0, sizeof(*registry));
}

void command_registry_destroy(CommandRegistry *registry) {
  if (registry == nullptr)
    return;
  UFUN *function = registry->user_functions;
  while (function != nullptr) {
    UFUN *next = function->next;
    free(function->name);
    free(function);
    function = next;
  }
  hash_table_destroy(&registry->commands);
  hash_table_destroy(&registry->macros);
  hash_table_destroy(&registry->functions);
  hash_table_destroy(&registry->user_function_index);
  memset(registry, 0, sizeof(*registry));
}

void world_indexes_initialize(WorldIndexes *indexes) {
  assert(indexes != nullptr);
  memset(indexes, 0, sizeof(*indexes));
}

void world_indexes_destroy(WorldIndexes *indexes) {
  if (indexes == nullptr)
    return;
  red_black_tree_walk(indexes->forward_lists.tree, WALK_POSTORDER,
                      free_numeric_hash_data, nullptr);
  hash_table_destroy(&indexes->powers);
  hash_table_destroy(&indexes->flags);
  hash_table_destroy(&indexes->attributes);
  hash_table_destroy(&indexes->players);
  numeric_hash_table_destroy(&indexes->forward_lists);
  numeric_hash_table_destroy(&indexes->parent_commands);
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
