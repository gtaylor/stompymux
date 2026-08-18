/** @file
 * Cohesive command, world, and access indexes.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/support/hash_table.h"

struct WorldIndexes;       // IWYU pragma: keep
struct AccessControlStore; // IWYU pragma: keep
struct CommandRegistry;    // IWYU pragma: keep
struct Cmdentry;           // IWYU pragma: keep
struct SwitchClone;        // IWYU pragma: keep

typedef struct SiteData SiteData;
typedef struct BadnameStruc BADNAME;

typedef struct CommandRegistry CommandRegistry;
typedef struct Cmdentry CMDENT;
typedef struct SwitchClone SwitchClone;
typedef struct CommandSwitchAlias CommandSwitchAlias;
struct CommandSwitchAlias {
  CMDENT *entry;
  char *name;
};

struct CommandRegistry {
  HashTable commands;
  CMDENT *prefix_commands[256];
  CMDENT *goto_command;
  HashTable macros;
  CMDENT *builtins;
  size_t builtin_count;
  SwitchClone *switch_clones;
  size_t switch_clone_count;
  size_t switch_clone_capacity;
  CommandSwitchAlias *switch_aliases;
  size_t switch_alias_count;
  size_t switch_alias_capacity;
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

/** Initializes command registry. @param[out] registry Registry to use. */

bool command_registry_initialize(CommandRegistry *registry);
/** Destroys command registry. @param[in,out] registry Registry to use. */

void command_registry_destroy(CommandRegistry *registry);
/** Initializes world indexes. @param[out] indexes Indexes. */

void world_indexes_initialize(WorldIndexes *indexes);
/** Destroys world indexes. @param[in,out] indexes Indexes. */

void world_indexes_destroy(WorldIndexes *indexes);
/** Initializes access control store. @param[out] store Store. */

void access_control_store_initialize(AccessControlStore *store);
/** Destroys access control store. @param[in,out] store Store. */

void access_control_store_destroy(AccessControlStore *store);
