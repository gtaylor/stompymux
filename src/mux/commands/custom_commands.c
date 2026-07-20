/* custom_commands.c - Command helpers. */

#include "mux/commands/command.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

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
    if (invocation->context->world->configuration->switch_df_all)
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
      wait_que(invocation->context->runtime->commands, player, cause, 0,
               NOTHING, 0, args[a + 1], cargs, ncargs,
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
    wait_que(invocation->context->runtime->commands, player, cause, 0, NOTHING,
             0, args[a], cargs, ncargs,
             invocation->context->evaluation.registers);
}
