/** @file
 * Registry-owned command catalog construction.
 */
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
/** Executes command catalog install. @param[in,out] registry Registry to use.
 * @param[in] definitions Definitions. @param[in] count Number of elements. */

bool command_catalog_install(CommandRegistry *registry,
                             const CommandDefinition *definitions,
                             size_t count);
/** Executes macro catalog install. @param[in,out] registry Registry to use.
 * @param[in] definitions Definitions. @param[in] count Number of elements. */

bool macro_catalog_install(CommandRegistry *registry, const MACENT *definitions,
                           size_t count);
/** Executes command builtin catalog install. @param[in,out] registry Registry
 * to use. */

bool command_builtin_catalog_install(CommandRegistry *registry);
/** Executes command macro catalog install. @param[in,out] registry Registry to
 * use. */

bool command_macro_catalog_install(CommandRegistry *registry);
/** Executes command catalog release. @param[in,out] registry Registry to use.
 */

void command_catalog_release(CommandRegistry *registry);
/** Counts command registry builtin. @param[in] registry Registry to use. */

size_t command_registry_builtin_count(const CommandRegistry *registry);
/** Returns command registry builtin at. @param[in] registry Registry to use.
 * @param[in] index Zero-based index. */

CMDENT *command_registry_builtin_at(CommandRegistry *registry, size_t index);
/** Returns command registry builtin at. @param[in] registry Registry to use.
 * @param[in] index Zero-based index. */

const CMDENT *command_registry_builtin_at_const(const CommandRegistry *registry,
                                                size_t index);
/** Adds alias to command registry. @param[in,out] registry Registry to use.
 * @param[in] alias Alias. @param[in,out] command Command text or descriptor. */

bool command_registry_add_alias(CommandRegistry *registry, const char *alias,
                                CMDENT *command);
/** Adds switch alias to command registry. @param[in,out] registry Registry to
 * use. @param[in] alias Alias. @param[in] command Command text or descriptor.
 * @param[in] selected_switch Selected switch. */

bool command_registry_add_switch_alias(CommandRegistry *registry,
                                       const char *alias, const CMDENT *command,
                                       const NameTable *selected_switch);
