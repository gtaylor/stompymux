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
};

typedef struct WorldIndexes WorldIndexes;
struct WorldIndexes {
  HashTable powers;
  HashTable flags;
  HashTable players;
};

typedef struct AccessControlStore AccessControlStore;
struct AccessControlStore {
  SiteData *access_sites;
  SiteData *suspect_sites;
  BADNAME *bad_names;
};

void command_registry_initialize(CommandRegistry *registry);
void command_registry_destroy(CommandRegistry *registry);
void world_indexes_initialize(WorldIndexes *indexes);
void world_indexes_destroy(WorldIndexes *indexes);
void access_control_store_initialize(AccessControlStore *store);
void access_control_store_destroy(AccessControlStore *store);
