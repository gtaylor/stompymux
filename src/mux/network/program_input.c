/* program_input.c - Descriptor input routing for interactive program editing. */

#include "mux/network/program_input.h"

#include "mux/server/platform.h"

#include "mux/commands/command_queue.h"
#include "mux/database/attrs.h"
#include "mux/network/netcommon.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#include "mux/support/formatting.h"
#include "mux/support/red_black_tree.h"

void descriptor_program_clear(DbRef player) {
  Descriptor *descriptor;
  ProgramData *program;
  int index;

  descriptor =
      (Descriptor *)red_black_tree_find(mudstate.desctree, (void *)player);
  if (descriptor == NULL || descriptor->program_data == NULL)
    return;

  program = descriptor->program_data;
  for (index = 0; index < MAX_GLOBAL_REGS; index++) {
    free_lbuf(program->wait_regs[index]);
  }
  free(program);

  DESC_ITER_PLAYER(player, descriptor)
  descriptor->program_data = NULL;

  attribute_clear(player, A_PROGCMD);
}

void descriptor_program_handle(Descriptor *d, char *message) {
  char *cmd;
  DbRef aowner;
  long aflags;

  /*
   * Allow the player to pipe a command while in interactive mode.
   */

  if (*message == '|') {
    descriptor_command(d, message + 1);
    /* Use telnet protocol's GOAHEAD command to show prompt */
    if (d->program_data != NULL)
      descriptor_queue_string(
          d, tprintf("%s>%s \377\371", ANSI_HILITE, ANSI_NORMAL));
    return;
  }
  cmd = attribute_get(d->player, A_PROGCMD, &aowner, &aflags);
  wait_que(d->program_data->wait_cause, d->player, 0, NOTHING, 0, cmd,
           (char **)&message, 1, (char **)d->program_data->wait_regs);

  descriptor_program_clear(d->player);
  free_lbuf(cmd);
}
