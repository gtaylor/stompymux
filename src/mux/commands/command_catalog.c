/* command_catalog.c - Handler-free command catalog ownership. */

#include "mux/commands/command_catalog.h"
#include "mux/commands/command.h"
#include "mux/commands/macro.h"

#include <stdlib.h>
#include <string.h>

#include "mux/commands/command_internal.h"
#include "mux/server/platform.h"
#include "mux/server/server_registries.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"

struct SwitchClone {
  const NameTable *source;
  NameTable *clone;
};

static size_t switch_table_count(const NameTable *table) {
  size_t count = 0;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunsafe-buffer-usage"
  while (table[count].name != nullptr)
    count++;
#pragma clang diagnostic pop
  return count + 1;
}

static SwitchClone *switch_clone_at(SwitchClone *records, size_t count,
                                    size_t index) {
  return checked_storage_at(records, count, sizeof(*records), index);
}

static NameTable *registry_switches(CommandRegistry *registry,
                                    const NameTable *source) {
  if (source == nullptr)
    return nullptr;
  SwitchClone *records = registry->switch_clones;
  for (size_t i = 0; i < registry->switch_clone_count; i++) {
    SwitchClone *record =
        switch_clone_at(records, registry->switch_clone_count, i);
    if (record->source == source)
      return record->clone;
  }

  if (registry->switch_clone_count == registry->switch_clone_capacity) {
    size_t capacity = registry->switch_clone_capacity == 0
                          ? 8
                          : registry->switch_clone_capacity * 2;
    SwitchClone *grown =
        checked_storage_try_reallocate_array(records, capacity, sizeof(*grown));
    if (grown == nullptr)
      return nullptr;
    registry->switch_clones = records = grown;
    registry->switch_clone_capacity = capacity;
  }
  size_t count = switch_table_count(source);
  NameTable *clone = checked_storage_try_allocate_array(count, sizeof(*clone));
  if (clone == nullptr)
    return nullptr;
  memcpy(clone, source, count * sizeof(*clone));
  *switch_clone_at(records, registry->switch_clone_capacity,
                   registry->switch_clone_count++) =
      (SwitchClone){.source = source, .clone = clone};
  return clone;
}

size_t command_registry_builtin_count(const CommandRegistry *registry) {
  return registry == nullptr ? 0 : registry->builtin_count;
}

CMDENT *command_registry_builtin_at(CommandRegistry *registry, size_t index) {
  if (registry == nullptr || index >= registry->builtin_count)
    return nullptr;
  return checked_storage_at(registry->builtins, registry->builtin_count,
                            sizeof(*registry->builtins), index);
}

const CMDENT *command_registry_builtin_at_const(const CommandRegistry *registry,
                                                size_t index) {
  if (registry == nullptr || index >= registry->builtin_count)
    return nullptr;
  return checked_storage_at_const(registry->builtins, registry->builtin_count,
                                  sizeof(*registry->builtins), index);
}

static bool command_catalog_is_empty(const CommandRegistry *registry) {
  static CMDENT *const EMPTY_PREFIX_COMMANDS[256] = {};
  return registry->commands.tree == nullptr && registry->builtins == nullptr &&
         registry->builtin_count == 0 && registry->switch_clones == nullptr &&
         registry->switch_clone_count == 0 &&
         registry->switch_clone_capacity == 0 &&
         registry->switch_aliases == nullptr &&
         registry->switch_alias_count == 0 &&
         registry->switch_alias_capacity == 0 &&
         registry->goto_command == nullptr &&
         memcmp((const void *)registry->prefix_commands,
                (const void *)EMPTY_PREFIX_COMMANDS,
                sizeof(registry->prefix_commands)) == 0;
}

bool command_catalog_install(CommandRegistry *registry,
                             const CommandDefinition *definitions,
                             size_t count) {
  if (registry == nullptr || definitions == nullptr ||
      !command_catalog_is_empty(registry))
    return false;
  registry->builtins =
      checked_storage_try_allocate_array(count, sizeof(CMDENT));
  if (count != 0 && registry->builtins == nullptr)
    return false;
  hash_table_initialize(&registry->commands, 250 * HASH_FACTOR);
  memset((void *)registry->prefix_commands, 0,
         sizeof(registry->prefix_commands));
  for (size_t i = 0; i < count; i++) {
    const CommandDefinition *definition =
        checked_storage_at_const(definitions, count, sizeof(*definitions), i);
    NameTable *switches = registry_switches(registry, definition->switches);
    if (definition->switches != nullptr && switches == nullptr)
      return false;
    CMDENT *entry = checked_storage_at(registry->builtins, count,
                                       sizeof(*registry->builtins), i);
    *entry = (CMDENT){.cmdname = definition->cmdname,
                      .switches = switches,
                      .perms = definition->perms,
                      .extra = definition->extra,
                      .callseq = definition->callseq,
                      .handler = definition->handler};
    if (hash_table_add(entry->cmdname, entry, &registry->commands) != 0)
      return false;
    registry->builtin_count++;
  }
  const unsigned char PREFIXES[] = {'"', ':', ';', '\\', '#'};
  for (size_t i = 0; i < sizeof(PREFIXES); i++) {
    const unsigned char *prefix = checked_storage_at_const(
        PREFIXES, sizeof(PREFIXES), sizeof(*PREFIXES), i);
    char name[2] = {(char)*prefix, '\0'};
    CMDENT **slot = (CMDENT **)checked_storage_at(
        (void *)registry->prefix_commands,
        sizeof(registry->prefix_commands) / sizeof(*registry->prefix_commands),
        sizeof(*registry->prefix_commands), *prefix);
    *slot = hash_table_find(name, &registry->commands);
  }
  registry->goto_command = hash_table_find("goto", &registry->commands);
  return registry->goto_command != nullptr;
}

bool macro_catalog_install(CommandRegistry *registry, const MACENT *definitions,
                           size_t count) {
  if (registry == nullptr || definitions == nullptr ||
      registry->macros.tree != nullptr)
    return false;
  hash_table_initialize(&registry->macros, 5 * HASH_FACTOR);
  for (size_t i = 0; i < count; i++) {
    const MACENT *definition =
        checked_storage_at_const(definitions, count, sizeof(*definitions), i);
    if (hash_table_add_const(definition->cmdname, definition,
                             &registry->macros) != 0)
      return false;
  }
  return true;
}

bool command_registry_add_alias(CommandRegistry *registry, const char *alias,
                                CMDENT *command) {
  return hash_table_add(alias, command, &registry->commands) == 0;
}

bool command_registry_add_switch_alias(CommandRegistry *registry,
                                       const char *alias, const CMDENT *command,
                                       const NameTable *selected_switch) {
  if (registry->switch_alias_count == registry->switch_alias_capacity) {
    size_t capacity = registry->switch_alias_capacity == 0
                          ? 4
                          : registry->switch_alias_capacity * 2;
    CommandSwitchAlias *grown = checked_storage_try_reallocate_array(
        registry->switch_aliases, capacity, sizeof(*grown));
    if (grown == nullptr)
      return false;
    registry->switch_aliases = grown;
    registry->switch_alias_capacity = capacity;
  }
  CMDENT *entry = checked_storage_try_allocate(sizeof(*entry));
  char *name = strsave(alias);
  if (entry == nullptr || name == nullptr) {
    free(entry);
    free(name);
    return false;
  }
  *entry = *command;
  entry->cmdname = name;
  entry->perms = command->perms | selected_switch->perm;
  entry->extra = (command->extra | selected_switch->flag) & ~SW_MULTIPLE;
  if (!(selected_switch->flag & SW_MULTIPLE))
    entry->extra |= SW_GOT_UNIQUE;
  if (hash_table_add(entry->cmdname, entry, &registry->commands) != 0) {
    free(name);
    free(entry);
    return false;
  }
  CommandSwitchAlias *slot = checked_storage_at(
      registry->switch_aliases, registry->switch_alias_capacity,
      sizeof(*registry->switch_aliases), registry->switch_alias_count++);
  *slot = (CommandSwitchAlias){.entry = entry, .name = name};
  return true;
}

void command_catalog_release(CommandRegistry *registry) {
  if (registry == nullptr)
    return;
  hash_table_destroy(&registry->commands);
  CommandSwitchAlias *aliases = registry->switch_aliases;
  for (size_t i = 0; i < registry->switch_alias_count; i++) {
    CommandSwitchAlias *alias = checked_storage_at(
        aliases, registry->switch_alias_count, sizeof(*aliases), i);
    free(alias->name);
    free(alias->entry);
  }
  free(aliases);
  hash_table_destroy(&registry->macros);
  SwitchClone *records = registry->switch_clones;
  for (size_t i = 0; i < registry->switch_clone_count; i++) {
    SwitchClone *record =
        switch_clone_at(records, registry->switch_clone_count, i);
    free(record->clone);
  }
  free(records);
  free(registry->builtins);
  memset(registry, 0, sizeof(*registry));
}
