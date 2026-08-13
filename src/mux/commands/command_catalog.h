/* command_catalog.h - Registry-owned command catalog construction. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/commands/command.h"
#include "mux/commands/macro.h"

typedef struct CommandDefinition CommandDefinition;
struct CommandDefinition {
  const char *cmdname;
  const NameTable *switches;
  int perms;
  int extra;
  int callseq;
  CmdHandler handler;
};

/* Installers require their corresponding registry fields to be zeroed. Macro
 * definitions are borrowed immutable storage and must outlive the registry. */
bool command_catalog_install(CommandRegistry *registry,
                             const CommandDefinition *definitions,
                             size_t count);
bool macro_catalog_install(CommandRegistry *registry, const MACENT *definitions,
                           size_t count);
bool command_builtin_catalog_install(CommandRegistry *registry);
bool command_macro_catalog_install(CommandRegistry *registry);
void command_catalog_release(CommandRegistry *registry);
size_t command_registry_builtin_count(const CommandRegistry *registry);
CMDENT *command_registry_builtin_at(CommandRegistry *registry, size_t index);
const CMDENT *command_registry_builtin_at_const(const CommandRegistry *registry,
                                                size_t index);
bool command_registry_add_alias(CommandRegistry *registry, const char *alias,
                                CMDENT *command);
bool command_registry_add_switch_alias(CommandRegistry *registry,
                                       const char *alias, const CMDENT *command,
                                       const NameTable *selected_switch);
