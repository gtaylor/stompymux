/*
 * command.c - command parser and support routines
 */

#include <assert.h>
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h" // IWYU pragma: keep
#include "mux/commands/command.h"
#include "mux/commands/command_catalog.h"
#include "mux/commands/command_internal.h"
#include "mux/commands/macro.h" // IWYU pragma: keep
#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h" // IWYU pragma: keep
#include "mux/server/configuration_interpreter.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"

NameTable access_nametab[] = {{"god", 2, CA_GOD, CA_GOD},
                              {"wizard", 3, CA_WIZARD, CA_WIZARD},
                              {"no_suspect", 5, CA_WIZARD, CA_NO_SUSPECT},
                              {"queue_enabled", 6, CA_PUBLIC, CA_QUEUE},
                              {"disabled", 4, CA_GOD, CA_DISABLED},
                              {"need_location", 6, CA_PUBLIC, CA_LOCATION},
                              {"need_contents", 6, CA_PUBLIC, CA_CONTENTS},
                              {"need_player", 6, CA_PUBLIC, CA_PLAYER},
                              {"dark", 4, CA_GOD, CF_DARK},
                              {nullptr, 0, 0, 0}};

void command_list_access(EvaluationContext *evaluation,
                         const ServerConfiguration *configuration,
                         const CommandRegistry *registry, DbRef player) {
  char buff[SBUF_SIZE];
  for (size_t index = 0; index < command_registry_builtin_count(registry);
       index++) {
    const CMDENT *cmdp = command_registry_builtin_at_const(registry, index);

    if (check_access(evaluation->world->database, configuration, player,
                     cmdp->perms)) {
      if (!(cmdp->perms & CF_DARK)) {
        (void)snprintf(buff, SBUF_SIZE, "%s:", cmdp->cmdname);
        name_table_list_set(evaluation, configuration, player, access_nametab,
                            cmdp->perms, buff, 1);
      }
    }
  }
}

/*
 * ---------------------------------------------------------------------------
 * * command_list_switches: List switches for commands.
 */

void command_list_switches(EvaluationContext *evaluation,
                           const ServerConfiguration *configuration,
                           const CommandRegistry *registry, DbRef player) {
  char buff[SBUF_SIZE];
  for (size_t index = 0; index < command_registry_builtin_count(registry);
       index++) {
    const CMDENT *cmdp = command_registry_builtin_at_const(registry, index);

    if (cmdp->switches) {
      if (check_access(evaluation->world->database, configuration, player,
                       cmdp->perms)) {
        if (!(cmdp->perms & CF_DARK)) {
          (void)snprintf(buff, SBUF_SIZE, "%s:", cmdp->cmdname);
          name_table_display(evaluation, configuration, player, cmdp->switches,
                             buff, 0);
        }
      }
    }
  }
}
/*
 * ---------------------------------------------------------------------------
 * * cf_access: Change command or switch permissions.
 */

int cf_access(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  CMDENT *cmdp;
  char *ap;
  const size_t LENGTH = strlen(str);
  size_t offset = 0;
  bool set_switch;

  while (offset < LENGTH) {
    const char CHARACTER = *(const char *)checked_storage_at_const(
        str, LENGTH + 1, sizeof(char), offset);

    if ((isspace)((unsigned char)CHARACTER) || CHARACTER == '/')
      break;
    offset++;
  }
  ap = checked_storage_at(str, LENGTH + 1, sizeof(char), offset);
  if (*ap == '/') {
    set_switch = true;
    *ap = '\0';
    offset++;
  } else {
    set_switch = false;
    if (*ap) {
      *ap = '\0';
      offset++;
    }
    while (offset < LENGTH &&
           (isspace)((unsigned char)*(const char *)checked_storage_at_const(
               str, LENGTH + 1, sizeof(char), offset)))
      offset++;
  }
  ap = checked_storage_at(str, LENGTH + 1, sizeof(char), offset);

  cmdp = (CMDENT *)hash_table_find(str, &context->command_registry->commands);
  if (cmdp != nullptr) {
    if (set_switch) {
      ConfigurationCall access_call = *call;
      access_call.value = cmdp->switches;
      access_call.text = ap;
      return cf_ntab_access(&access_call);
    }
    ConfigurationCall access_call = *call;
    access_call.value = &cmdp->perms;
    access_call.text = ap;
    return configuration_modify_bits(&access_call);
  }
  configuration_log_not_found(context, call->player, call->command, "Command",
                              str);
  return -1;
}

/*
 * ---------------------------------------------------------------------------
 * * cf_cmd_alias: Add a command alias.
 */

int cf_cmd_alias(const ConfigurationCall *call) {
  char *str = call->text;
  ConfigurationContext *context = call->context;
  CommandRegistry *registry = context->command_registry;
  char *alias;
  char *orig;
  char *ap;
  CMDENT *cmdp;
  NameTable *nt;

  assert(call->value == &registry->commands);

  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");

  if (!orig) /*
              * * we only got one argument to @alias.
              * Bad.
              */
    return -1;

  const size_t ORIG_LENGTH = strlen(orig);
  size_t switch_offset = 0;

  while (switch_offset < ORIG_LENGTH &&
         *(const char *)checked_storage_at_const(
             orig, ORIG_LENGTH + 1, sizeof(char), switch_offset) != '/')
    switch_offset++;
  ap = checked_storage_at(orig, ORIG_LENGTH + 1, sizeof(char), switch_offset);
  if (*ap == '/') {

    /*
     * Switch form of command aliasing: create an alias for a  *
     * * * * * * command + a switch
     */

    *ap = '\0';
    ap = checked_storage_at(orig, ORIG_LENGTH + 1, sizeof(char),
                            switch_offset + 1);

    /*
     * Look up the command
     */

    cmdp = hash_table_find(orig, &registry->commands);
    if (cmdp == nullptr) {
      configuration_log_not_found(context, call->player, call->command,
                                  "Command", orig);
      return -1;
    }
    /*
     * Look up the switch
     */

    nt = name_table_find_entry(context->database, context->configuration,
                               call->player, cmdp->switches, ap);
    if (!nt) {
      configuration_log_not_found(context, call->player, call->command,
                                  "Switch", ap);
      return -1;
    }
    /*
     * Got it, create the new command table entry
     */

    if (!command_registry_add_switch_alias(registry, alias, cmdp, nt)) {
      configuration_log_syntax(context, call->player, call->command,
                               "Unable to add command alias: ", alias);
      return -1;
    }
  } else {

    /*
     * A normal (non-switch) alias
     */

    CMDENT *source = hash_table_find(orig, &registry->commands);
    if (source == nullptr) {
      configuration_log_not_found(context, call->player, call->command, "Entry",
                                  orig);
      return -1;
    }
    if (!command_registry_add_alias(registry, alias, source)) {
      configuration_log_syntax(context, call->player, call->command,
                               "Unable to add command alias: ", alias);
      return -1;
    }
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * list_df_flags: List default flags at create time.
 */
