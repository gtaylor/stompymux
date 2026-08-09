/*
 * command.c - command parser and support routines
 */

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "btech/context.h" // IWYU pragma: keep
#include "btmux_build_config.h"
#include "mux/commands/command.h"
#include "mux/commands/command_internal.h"
#include "mux/commands/macro.h" // IWYU pragma: keep
#include "mux/server/configuration.h"
#include "mux/server/configuration_context.h" // IWYU pragma: keep
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/support/stringutil.h"

#ifdef ARBITRARY_LOGFILES
#include "mux/server/log_cache.h"
#endif

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
                         CommandRegistry *registry, DbRef player) {
  char buff[SBUF_SIZE];
  for (size_t index = 0; index < command_table_entry_count(); index++) {
    CMDENT *cmdp = command_table_entry_at(index);

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
                           DbRef player) {
  char buff[SBUF_SIZE];
  for (size_t index = 0; index < command_table_entry_count(); index++) {
    CMDENT *cmdp = command_table_entry_at(index);

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

int cf_access(int *vp, char *str, long extra, DbRef player, char *cmd,
              ConfigurationContext *context) {
  CMDENT *cmdp;
  char *ap;
  const size_t length = strlen(str);
  size_t offset = 0;
  bool set_switch;

  while (offset < length) {
    const char character = *(const char *)checked_storage_at_const(
        str, length + 1, sizeof(char), offset);

    if ((isspace)((unsigned char)character) || character == '/')
      break;
    offset++;
  }
  ap = checked_storage_at(str, length + 1, sizeof(char), offset);
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
    while (offset < length &&
           (isspace)((unsigned char)*(const char *)checked_storage_at_const(
               str, length + 1, sizeof(char), offset)))
      offset++;
  }
  ap = checked_storage_at(str, length + 1, sizeof(char), offset);

  cmdp = (CMDENT *)hash_table_find(str, &context->command_registry->commands);
  if (cmdp != nullptr) {
    if (set_switch)
      return cf_ntab_access((int *)cmdp->switches, ap, extra, player, cmd,
                            context);
    else
      return configuration_modify_bits(&(cmdp->perms), ap, extra, player, cmd,
                                       context);
  } else {
    configuration_log_not_found(context, player, cmd, "Command", str);
    return -1;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * cf_cmd_alias: Add a command alias.
 */

int cf_cmd_alias(void *vp, char *str, long extra, DbRef player, char *cmd,
                 ConfigurationContext *context) {
  char *alias, *orig, *ap;
  CMDENT *cmdp, *cmd2;
  NameTable *nt;
  int *hp;

  alias = strtok(str, " \t=,");
  orig = strtok(nullptr, " \t=,");

  if (!orig) /*
              * * we only got one argument to @alias.
              * Bad.
              */
    return -1;

  const size_t orig_length = strlen(orig);
  size_t switch_offset = 0;

  while (switch_offset < orig_length &&
         *(const char *)checked_storage_at_const(
             orig, orig_length + 1, sizeof(char), switch_offset) != '/')
    switch_offset++;
  ap = checked_storage_at(orig, orig_length + 1, sizeof(char), switch_offset);
  if (*ap == '/') {

    /*
     * Switch form of command aliasing: create an alias for a  *
     * * * * * * command + a switch
     */

    *ap = '\0';
    ap = checked_storage_at(orig, orig_length + 1, sizeof(char),
                            switch_offset + 1);

    /*
     * Look up the command
     */

    cmdp = (CMDENT *)hash_table_find(orig, (HashTable *)vp);
    if (cmdp == nullptr) {
      configuration_log_not_found(context, player, cmd, "Command", orig);
      return -1;
    }
    /*
     * Look up the switch
     */

    nt = name_table_find_entry(context->database, context->configuration,
                               player, (NameTable *)cmdp->switches, ap);
    if (!nt) {
      configuration_log_not_found(context, player, cmd, "Switch", ap);
      return -1;
    }
    /*
     * Got it, create the new command table entry
     */

    cmd2 = malloc(sizeof(CMDENT));
    cmd2->cmdname = strsave(alias);
    cmd2->switches = cmdp->switches;
    cmd2->perms = cmdp->perms | nt->perm;
    cmd2->extra = (cmdp->extra | nt->flag) & ~SW_MULTIPLE;
    if (!(nt->flag & SW_MULTIPLE))
      cmd2->extra |= SW_GOT_UNIQUE;
    cmd2->callseq = cmdp->callseq;
    cmd2->handler = cmdp->handler;
    if (hash_table_add(cmd2->cmdname, (int *)cmd2, (HashTable *)vp)) {
      /* cmd2->cmdname was allocated by strsave() above; freeing it needs
         to discard the const we otherwise want on CMDENT.cmdname. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      free((void *)cmd2->cmdname);
#pragma clang diagnostic pop
      free(cmd2);
    }
  } else {

    /*
     * A normal (non-switch) alias
     */

    hp = hash_table_find(orig, (HashTable *)vp);
    if (hp == nullptr) {
      configuration_log_not_found(context, player, cmd, "Entry", orig);
      return -1;
    }
    hash_table_add(alias, hp, (HashTable *)vp);
  }
  return 0;
}

/*
 * ---------------------------------------------------------------------------
 * * list_df_flags: List default flags at create time.
 */
