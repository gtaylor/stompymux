/* server_registries.h - Cohesive command, world, and access indexes. */

#pragma once

#include "mux/database/db.h"
#include "mux/support/hash_table.h"

typedef struct SiteData SiteData;
typedef struct badname_struc BADNAME;

typedef struct CommandRegistry CommandRegistry;
struct CommandRegistry {
  HashTable commands;
  void *prefix_commands[256];
  void *goto_command;
  HashTable macros;
  HashTable functions;
  HashTable user_function_index;
  void *user_functions;
};

typedef struct WorldIndexes WorldIndexes;
struct WorldIndexes {
  HashTable powers;
  HashTable flags;
  HashTable attributes;
  HashTable players;
  HashTable forward_lists;
  HashTable parent_commands;
};

typedef struct AccessControlStore AccessControlStore;
struct AccessControlStore {
  SiteData *access_sites;
  SiteData *suspect_sites;
  BADNAME *bad_names;
};

void command_registry_initialize(CommandRegistry *registry);
void world_indexes_initialize(WorldIndexes *indexes);
void access_control_store_initialize(AccessControlStore *store);
