/* custom_commands.c - User-defined command lookup and dispatch helpers. */

#include "mux/commands/command.h"

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"

void do_switch(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  int key = invocation->key;
  char *expr = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  char **cargs = invocation->command_arguments;
  int ncargs = invocation->command_argument_count;
  int a, any;
  char *buff, *bp, *str;

  if (!expr || (nargs <= 0))
    return;

  if (key == SWITCH_DEFAULT) {
    if (invocation->context->server->configuration->switch_df_all)
      key = SWITCH_ANY;
    else
      key = SWITCH_ONE;
  }
  /*
   * now try a wild card match of buff with stuff in coms
   */

  any = 0;
  buff = bp = alloc_lbuf("do_switch");
  for (a = 0; (a < (nargs - 1)) && args[a] && args[a + 1]; a += 2) {
    bp = buff;
    str = args[a];
    exec(evaluation, buff, &bp, 0, player, cause, EV_FCHECK | EV_EVAL | EV_TOP,
         &str, cargs, ncargs);
    *bp = '\0';
    if (wild_match(buff, expr)) {
      wait_que(invocation->context->server->commands, player, cause, 0, NOTHING,
               0, args[a + 1], cargs, ncargs,
               invocation->context->evaluation.registers);
      if (key == SWITCH_ONE) {
        free_lbuf(buff);
        return;
      }
      any = 1;
    }
  }
  free_lbuf(buff);
  if ((a < nargs) && !any && args[a])
    wait_que(invocation->context->server->commands, player, cause, 0, NOTHING,
             0, args[a], cargs, ncargs,
             invocation->context->evaluation.registers);
}

void do_addcommand(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *command = invocation->second;
  CommandRegistry *registry = &invocation->context->server->command_registry;
  CMDENT *old, *cmd;
  ADDENT *add, *nextp;

  DbRef thing;
  int atr;
  char *s;

  if (!*name) {
    notify(evaluation, player, "Sorry.");
    return;
  }
  if (!parse_attrib(&invocation->context->match, player, command, &thing,
                    &atr) ||
      (atr == NOTHING)) {
    notify(evaluation, player, "No such attribute.");
    return;
  }

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = (char)tolower(*s);
  }

  old = (CMDENT *)hash_table_find(name, &registry->commands);

  if (old && (old->callseq & CS_ADDED)) {

    /* If it's already found in the hash table, and it's being
       added using the same object and attribute... */

    for (nextp = old->handler.added; nextp != nullptr; nextp = nextp->next) {
      if ((nextp->thing == thing) && (nextp->atr == atr)) {
        notify_printf(evaluation, player, "%s already added.", name);
        return;
      }
    }

    /* else tack it on to the existing entry... */

    add = malloc(sizeof(ADDENT));
    add->thing = thing;
    add->atr = atr;
    add->name = (char *)strdup(name);
    add->next = old->handler.added;
    old->handler.added = add;
  } else {
    if (old) {
      /* Delete the old built-in and rename it __name */
      hash_table_delete(name, &registry->commands);
    }

    cmd = malloc(sizeof(CMDENT));

    cmd->cmdname = (char *)strdup(name);
    cmd->switches = nullptr;
    cmd->perms = 0;
    cmd->extra = 0;
    if (old && (old->callseq & CS_LEADIN)) {
      cmd->callseq = CS_ADDED | CS_ONE_ARG | CS_LEADIN;
    } else {
      cmd->callseq = CS_ADDED | CS_ONE_ARG;
    }
    add = malloc(sizeof(ADDENT));
    add->thing = thing;
    add->atr = atr;
    add->name = (char *)strdup(name);
    add->next = nullptr;
    cmd->handler.added = add;

    hash_table_add(name, (int *)cmd, &registry->commands);

    if (old) {
      /* Fix any aliases of this command. */
      hash_table_replace_all((int *)old, (int *)cmd, &registry->commands);
      hash_table_add(tprintf("__%s", name), (int *)old, &registry->commands);
    }
  }

  /* We reset the one letter commands here so you can overload them */

  set_prefix_cmds(registry);
  notify_printf(evaluation, player, "%s added.", name);
}

void do_listcommands(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  CommandRegistry *registry = &invocation->context->server->command_registry;
  CMDENT *old;
  ADDENT *nextp;
  int didit = 0;

  char *s, *keyname;

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = (char)tolower(*s);
  }

  if (*name) {
    old = (CMDENT *)hash_table_find(name, &registry->commands);

    if (old && (old->callseq & CS_ADDED)) {

      /* If it's already found in the hash table, and it's being
         added using the same object and attribute... */

      for (nextp = old->handler.added; nextp != nullptr; nextp = nextp->next) {
        notify_printf(evaluation, player, "%s: #%ld/%s", nextp->name,
                      nextp->thing,
                      ((Attribute *)attribute_by_number(
                           invocation->context->world->database, nextp->atr))
                          ->name);
      }
    } else {
      notify_printf(evaluation, player, "%s not found in command table.", name);
    }
    return;
  } else {
    for (keyname = hash_table_first_key(&registry->commands);
         keyname != nullptr;
         keyname = hash_table_next_key(&registry->commands)) {

      old = (CMDENT *)hash_table_find(keyname, &registry->commands);

      if (old && (old->callseq & CS_ADDED)) {

        for (nextp = old->handler.added; nextp != nullptr;
             nextp = nextp->next) {
          if (strcmp(keyname, nextp->name))
            continue;
          notify_printf(evaluation, player, "%s: #%ld/%s", nextp->name,
                        nextp->thing,
                        ((Attribute *)attribute_by_number(
                             invocation->context->world->database, nextp->atr))
                            ->name);
          didit = 1;
        }
      }
    }
  }
  if (!didit)
    notify(evaluation, player, "No added commands found in command table.");
}

void do_delcommand(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *name = invocation->first;
  char *command = invocation->second;
  CommandRegistry *registry = &invocation->context->server->command_registry;
  CMDENT *old, *cmd;
  ADDENT *prev = nullptr, *nextp;

  DbRef thing;
  int atr;
  char *s;

  if (!*name) {
    notify(evaluation, player, "Sorry.");
    return;
  }

  if (*command) {
    if (!parse_attrib(&invocation->context->match, player, command, &thing,
                      &atr) ||
        (atr == NOTHING)) {
      notify(evaluation, player, "No such attribute.");
      return;
    }
  }

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = (char)tolower(*s);
  }

  old = (CMDENT *)hash_table_find(name, &registry->commands);

  if (old && (old->callseq & CS_ADDED)) {
    if (!*command) {
      for (prev = old->handler.added; prev != nullptr; prev = nextp) {
        nextp = prev->next;
        /* Delete it! */
        free(prev->name);
        free(prev);
      }
      hash_table_delete(name, &registry->commands);
      if ((cmd = (CMDENT *)hash_table_find(tprintf("__%s", name),
                                           &registry->commands)) != nullptr) {
        hash_table_delete(tprintf("__%s", name), &registry->commands);
        hash_table_add(name, (int *)cmd, &registry->commands);
        hash_table_replace_all((int *)old, (int *)cmd, &registry->commands);
      }
      free(old);
      set_prefix_cmds(registry);
      notify(evaluation, player, "Done.");
      return;
    } else {
      for (nextp = old->handler.added; nextp != nullptr; nextp = nextp->next) {
        if ((nextp->thing == thing) && (nextp->atr == atr)) {
          /* Delete it! */
          free(nextp->name);
          if (!prev) {
            if (!nextp->next) {
              hash_table_delete(name, &registry->commands);
              if ((cmd = (CMDENT *)hash_table_find(tprintf("__%s", name),
                                                   &registry->commands)) !=
                  nullptr) {
                hash_table_delete(tprintf("__%s", name), &registry->commands);
                hash_table_add(name, (int *)cmd, &registry->commands);
                hash_table_replace_all((int *)old, (int *)cmd,
                                       &registry->commands);
              }
              free(old);
            } else {
              old->handler.added = nextp->next;
              free(nextp);
            }
          } else {
            prev->next = nextp->next;
            free(nextp);
          }
          set_prefix_cmds(registry);
          notify(evaluation, player, "Done.");
          return;
        }
        prev = nextp;
      }
      notify(evaluation, player, "Command not found in command table.");
    }
  } else {
    notify(evaluation, player, "Command not found in command table.");
  }
}
