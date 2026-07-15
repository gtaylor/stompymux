/* program.c - Player program editing and execution commands. */

#include "mux/commands/command.h"

#include "mux/server/platform.h"

#include "mux/commands/functions.h"
#include "mux/database/attrs.h"
#include "mux/network/netcommon.h"
#include "mux/network/program_input.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/formatting.h"

void do_quitprog(DbRef player, DbRef cause, int key, char *name) {
  Descriptor *d;
  DbRef doer;
  int isprog = 0;

  if (*name) {
    doer = match_thing(player, name);
  } else {
    doer = player;
  }

  if (!(is_wizard(player) || is_wizard(obj_owner(player))) &&
      (player != doer)) {
    notify(player, "Permission denied.");
    return;
  }
  if (!is_player(doer) || !is_good_obj(doer)) {
    notify(player, "That is not a player.");
    return;
  }
  if (!is_connected(doer)) {
    notify(player, "That player is not connected.");
    return;
  }
  DESC_ITER_PLAYER(doer, d) {
    if (d->program_data != nullptr) {
      isprog = 1;
    }
  }

  if (!isprog) {
    notify(player, "Player is not in an @program.");
    return;
  }
  descriptor_program_clear(doer);
  notify(player, "@program cleared.");
  notify(doer, "Your @program has been terminated.");
}

void do_prog(DbRef player, DbRef cause, int key, char *name, char *command) {
  Descriptor *d;
  ProgramData *program;
  int i, atr;
  long aflags;
  DbRef doer, thing, aowner;
  Attribute *ap;
  char *attrib, *msg;

  if (!name || !*name) {
    notify(player, "No players specified.");
    return;
  }
  doer = match_thing(player, name);

  if (!(is_wizard(player) || is_wizard(obj_owner(player))) &&
      (player != doer)) {
    notify(player, "Permission denied.");
    return;
  }
  if (!is_player(doer) || !is_good_obj(doer)) {
    notify(player, "That is not a player.");
    return;
  }
  if (!is_connected(doer)) {
    notify(player, "That player is not connected.");
    return;
  }
  msg = command;
  attrib = parse_to(&msg, ':', 1);

  if (msg && *msg) {
    notify(doer, msg);
  }
  parse_attrib(player, attrib, &thing, &atr);
  if (atr != NOTHING) {
    if (!attribute_get_info(thing, atr, &aowner, &aflags)) {
      notify(player, "Attribute not present on object.");
      return;
    }
    ap = attribute_by_number(atr);
    if (is_god(player) ||
        (!is_god(thing) && see_attr(player, thing, ap, aowner, aflags) &&
         (is_wizard(player) || (aowner == obj_owner(player))))) {
      attribute_add_raw(doer, A_PROGCMD, attribute_get_raw(thing, atr));
    } else {
      notify(player, "Permission denied.");
      return;
    }
  } else {
    notify(player, "No such attribute.");
    return;
  }

  /*
   * Check to see if the cause already has an @prog input pending
   */
  DESC_ITER_PLAYER(doer, d) {
    if (d->program_data != nullptr) {
      notify(player, "Input already pending.");
      return;
    }
  }

  program = malloc(sizeof(ProgramData));
  program->wait_cause = player;
  for (i = 0; i < MAX_GLOBAL_REGS; i++) {
    program->wait_regs[i] = alloc_lbuf("prog_regs");
    StringCopy(program->wait_regs[i], mudstate.global_regs[i]);
  }

  /*
   * Now, start waiting.
   */
  DESC_ITER_PLAYER(doer, d) {
    d->program_data = program;

    /*
     * Use telnet protocol's GOAHEAD command to show prompt
     */
    descriptor_queue_string(
        d, tprintf("%s>%s \377\371", ANSI_HILITE, ANSI_NORMAL));
  }
}

/**
 * Implement the @@ (comment) command. Very cpu-intensive :-)
 */
void do_comment(DbRef player, DbRef cause, int key) {}
