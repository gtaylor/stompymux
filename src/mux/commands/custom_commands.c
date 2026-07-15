/* custom_commands.c - User-defined command lookup and dispatch helpers. */

#include "mux/commands/command.h"

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"

void do_switch(DbRef player, DbRef cause, int key, char *expr, char *args[],
               int nargs, char *cargs[], int ncargs) {
  int a, any;
  char *buff, *bp, *str;

  if (!expr || (nargs <= 0))
    return;

  if (key == SWITCH_DEFAULT) {
    if (mudconf.switch_df_all)
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
    exec(buff, &bp, 0, player, cause, EV_FCHECK | EV_EVAL | EV_TOP, &str, cargs,
         ncargs);
    *bp = '\0';
    if (wild_match(buff, expr)) {
      wait_que(player, cause, 0, NOTHING, 0, args[a + 1], cargs, ncargs,
               mudstate.global_regs);
      if (key == SWITCH_ONE) {
        free_lbuf(buff);
        return;
      }
      any = 1;
    }
  }
  free_lbuf(buff);
  if ((a < nargs) && !any && args[a])
    wait_que(player, cause, 0, NOTHING, 0, args[a], cargs, ncargs,
             mudstate.global_regs);
}

void do_addcommand(DbRef player, DbRef cause, int key, char *name,
                   char *command) {
  CMDENT *old, *cmd;
  ADDENT *add, *nextp;

  DbRef thing;
  int atr;
  char *s;

  if (!*name) {
    notify(player, "Sorry.");
    return;
  }
  if (!parse_attrib(player, command, &thing, &atr) || (atr == NOTHING)) {
    notify(player, "No such attribute.");
    return;
  }

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = tolower(*s);
  }

  old = (CMDENT *)hash_table_find(name, &mudstate.command_htab);

  if (old && (old->callseq & CS_ADDED)) {

    /* If it's already found in the hash table, and it's being
       added using the same object and attribute... */

    for (nextp = (ADDENT *)old->handler; nextp != nullptr;
         nextp = nextp->next) {
      if ((nextp->thing == thing) && (nextp->atr == atr)) {
        notify_printf(player, "%s already added.", name);
        return;
      }
    }

    /* else tack it on to the existing entry... */

    add = malloc(sizeof(ADDENT));
    add->thing = thing;
    add->atr = atr;
    add->name = (char *)strdup(name);
    add->next = (ADDENT *)old->handler;
    old->handler = (void *)add;
  } else {
    if (old) {
      /* Delete the old built-in and rename it __name */
      hash_table_delete(name, &mudstate.command_htab);
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
    cmd->handler = (void *)add;

    hash_table_add(name, (int *)cmd, &mudstate.command_htab);

    if (old) {
      /* Fix any aliases of this command. */
      hash_table_replace_all((int *)old, (int *)cmd, &mudstate.command_htab);
      hash_table_add(tprintf("__%s", name), (int *)old, &mudstate.command_htab);
    }
  }

  /* We reset the one letter commands here so you can overload them */

  set_prefix_cmds();
  notify_printf(player, "%s added.", name);
}

void do_listcommands(DbRef player, DbRef cause, int key, char *name) {
  CMDENT *old;
  ADDENT *nextp;
  int didit = 0;

  char *s, *keyname;

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = tolower(*s);
  }

  if (*name) {
    old = (CMDENT *)hash_table_find(name, &mudstate.command_htab);

    if (old && (old->callseq & CS_ADDED)) {

      /* If it's already found in the hash table, and it's being
         added using the same object and attribute... */

      for (nextp = (ADDENT *)old->handler; nextp != nullptr;
           nextp = nextp->next) {
        notify_printf(player, "%s: #%d/%s", nextp->name, nextp->thing,
                      ((Attribute *)attribute_by_number(nextp->atr))->name);
      }
    } else {
      notify_printf(player, "%s not found in command table.", name);
    }
    return;
  } else {
    for (keyname = hash_table_first_key(&mudstate.command_htab);
         keyname != nullptr;
         keyname = hash_table_next_key(&mudstate.command_htab)) {

      old = (CMDENT *)hash_table_find(keyname, &mudstate.command_htab);

      if (old && (old->callseq & CS_ADDED)) {

        for (nextp = (ADDENT *)old->handler; nextp != nullptr;
             nextp = nextp->next) {
          if (strcmp(keyname, nextp->name))
            continue;
          notify_printf(player, "%s: #%d/%s", nextp->name, nextp->thing,
                        ((Attribute *)attribute_by_number(nextp->atr))->name);
          didit = 1;
        }
      }
    }
  }
  if (!didit)
    notify(player, "No added commands found in command table.");
}

void do_delcommand(DbRef player, DbRef cause, int key, char *name,
                   char *command) {
  CMDENT *old, *cmd;
  ADDENT *prev = nullptr, *nextp;

  DbRef thing;
  int atr;
  char *s;

  if (!*name) {
    notify(player, "Sorry.");
    return;
  }

  if (*command) {
    if (!parse_attrib(player, command, &thing, &atr) || (atr == NOTHING)) {
      notify(player, "No such attribute.");
      return;
    }
  }

  /* Let's make this case insensitive... */

  for (s = name; *s; s++) {
    *s = tolower(*s);
  }

  old = (CMDENT *)hash_table_find(name, &mudstate.command_htab);

  if (old && (old->callseq & CS_ADDED)) {
    if (!*command) {
      for (prev = (ADDENT *)old->handler; prev != nullptr; prev = nextp) {
        nextp = prev->next;
        /* Delete it! */
        free(prev->name);
        free(prev);
      }
      hash_table_delete(name, &mudstate.command_htab);
      if ((cmd = (CMDENT *)hash_table_find(
               tprintf("__%s", name), &mudstate.command_htab)) != nullptr) {
        hash_table_delete(tprintf("__%s", name), &mudstate.command_htab);
        hash_table_add(name, (int *)cmd, &mudstate.command_htab);
        hash_table_replace_all((int *)old, (int *)cmd, &mudstate.command_htab);
      }
      free(old);
      set_prefix_cmds();
      notify(player, "Done.");
      return;
    } else {
      for (nextp = (ADDENT *)old->handler; nextp != nullptr;
           nextp = nextp->next) {
        if ((nextp->thing == thing) && (nextp->atr == atr)) {
          /* Delete it! */
          free(nextp->name);
          if (!prev) {
            if (!nextp->next) {
              hash_table_delete(name, &mudstate.command_htab);
              if ((cmd = (CMDENT *)hash_table_find(tprintf("__%s", name),
                                                   &mudstate.command_htab)) !=
                  nullptr) {
                hash_table_delete(tprintf("__%s", name),
                                  &mudstate.command_htab);
                hash_table_add(name, (int *)cmd, &mudstate.command_htab);
                hash_table_replace_all((int *)old, (int *)cmd,
                                       &mudstate.command_htab);
              }
              free(old);
            } else {
              old->handler = (void *)nextp->next;
              free(nextp);
            }
          } else {
            prev->next = nextp->next;
            free(nextp);
          }
          set_prefix_cmds();
          notify(player, "Done.");
          return;
        }
        prev = nextp;
      }
      notify(player, "Command not found in command table.");
    }
  } else {
    notify(player, "Command not found in command table.");
  }
}
