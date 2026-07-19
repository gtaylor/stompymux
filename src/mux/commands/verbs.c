/* verbs.c - Attribute-driven verb execution and action messaging. */

#include "mux/commands/verbs.h"

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/database/attrs.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/server_api.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/world/object_spatial.h"

void did_it(EvaluationContext *evaluation, DbRef player, DbRef thing, int what,
            const char *def, int owhat, const char *odef, int awhat,
            char *args[], int nargs) {
  char *d, *buff, *act, *charges, *bp, *str;
  DbRef loc, aowner;
  int num;
  long aflags;

  /*
   * message to player
   */

  if (what > 0) {
    d = attribute_parent_get(evaluation->world->database, thing, what, &aowner,
                             &aflags);
    if (*d) {
      buff = bp = alloc_lbuf("did_it.1");
      str = d;
      exec(evaluation, buff, &bp, 0, thing, player,
           EV_EVAL | EV_FIGNORE | EV_TOP, &str, args, nargs);
      *bp = '\0';
      notify(evaluation, player, buff);
      free_lbuf(buff);
    } else if (def) {
      notify(evaluation, player, def);
    }
    free_lbuf(d);
  } else if ((what < 0) && def) {
    notify(evaluation, player, def);
  }
  /*
   * message to neighbors
   */

  if ((owhat > 0) && has_location(evaluation->world->database, player) &&
      is_good_obj(
          evaluation->world->database,
          loc = game_object_location(evaluation->world->database, player))) {
    d = attribute_parent_get(evaluation->world->database, thing, owhat, &aowner,
                             &aflags);
    if (*d) {
      buff = bp = alloc_lbuf("did_it.2");
      str = d;
      exec(evaluation, buff, &bp, 0, thing, player,
           EV_EVAL | EV_FIGNORE | EV_TOP, &str, args, nargs);
      *bp = '\0';
      if (*buff)
        notify_except2(
            evaluation, loc, player, player, thing,
            tprintf("%s %s",
                    game_object_name(evaluation->world->database, player),
                    buff));
      free_lbuf(buff);
    } else if (odef) {
      notify_except2(
          evaluation, loc, player, player, thing,
          tprintf("%s %s",
                  game_object_name(evaluation->world->database, player), odef));
    }
    free_lbuf(d);
  } else if ((owhat < 0) && odef &&
             has_location(evaluation->world->database, player) &&
             is_good_obj(evaluation->world->database,
                         loc = game_object_location(evaluation->world->database,
                                                    player))) {
    notify_except2(
        evaluation, loc, player, player, thing,
        tprintf("%s %s", game_object_name(evaluation->world->database, player),
                odef));
  }
  /*
   * do the action attribute
   */

  if (awhat > 0) {
    if (lua_event_dispatch(evaluation->runtime->lua_owner->runtime, player,
                           thing, awhat, args, nargs))
      return;
    if (*(act = attribute_parent_get(evaluation->world->database, thing, awhat,
                                     &aowner, &aflags))) {
      charges = attribute_parent_get(evaluation->world->database, thing,
                                     A_CHARGES, &aowner, &aflags);
      if (*charges) {
        num = atoi(charges);
        if (num > 0) {
          buff = alloc_sbuf("did_it.charges");
          snprintf(buff, SBUF_SIZE, "%d", num - 1);
          attribute_add_raw(evaluation->world->database, thing, A_CHARGES,
                            buff);
          free_sbuf(buff);
        } else if (*(buff = attribute_parent_get(evaluation->world->database,
                                                 thing, A_RUNOUT, &aowner,
                                                 &aflags))) {
          free_lbuf(act);
          act = buff;
        } else {
          free_lbuf(act);
          free_lbuf(buff);
          free_lbuf(charges);
          return;
        }
      }
      free_lbuf(charges);
      wait_que(evaluation->runtime->commands, thing, player, 0, NOTHING, 0, act,
               args, nargs, evaluation->registers);
    }
    free_lbuf(act);
  }
}

/**
 * Command interface to did_it.
 */
void do_verb(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  const ServerConfiguration *configuration = evaluation->world->configuration;
  DbRef player = invocation->player;
  DbRef cause = invocation->cause;
  char *victim_str = invocation->first;
  char **args = invocation->vector;
  int nargs = invocation->vector_count;
  DbRef actor, victim, aowner;
  int what, owhat, awhat, nxargs, restriction, i;
  long aflags;
  Attribute *ap;
  const char *whatd, *owhatd;
  char *xargs[10];

  /*
   * Look for the victim
   */

  if (!victim_str || !*victim_str) {
    notify(evaluation, player, "Nothing to do.");
    return;
  }
  /*
   * Get the victim
   */

  init_match(&invocation->context->match, player, victim_str, NOTYPE);
  match_everything(&invocation->context->match, MAT_EXIT_PARENTS);
  victim = noisy_match_result(&invocation->context->match);
  if (!is_good_obj(evaluation->world->database, victim))
    return;

  /*
   * Get the actor.  Default is my cause
   */

  if ((nargs >= 1) && args[0] && *args[0]) {
    init_match(&invocation->context->match, player, args[0], NOTYPE);
    match_everything(&invocation->context->match, MAT_EXIT_PARENTS);
    actor = noisy_match_result(&invocation->context->match);
    if (!is_good_obj(evaluation->world->database, actor))
      return;
  } else {
    actor = cause;
  }

  /*
   * Check permissions. There are two possibilities
   *   1: Player controls both victim and actor. In this case, victim
   *      runs his action list.
   *   2: Player controls actor. In this case, victim does not run his
   *      action list and any attributes that player cannot read from
   *      victim are defaulted.
   */

  if (!is_controls(evaluation, player, actor)) {
    notify_quiet(evaluation, player, "Permission denied,");
    return;
  }
  restriction = !is_controls(evaluation, player, victim);

  what = -1;
  owhat = -1;
  awhat = -1;
  whatd = nullptr;
  owhatd = nullptr;
  nxargs = 0;

  /*
   * Get invoker message attribute
   */

  if (nargs >= 2) {
    ap = attribute_by_name(evaluation->world->database, args[1]);
    if (ap && (ap->number > 0))
      what = ap->number;
  }
  /*
   * Get invoker message default
   */

  if ((nargs >= 3) && args[2] && *args[2]) {
    whatd = args[2];
  }
  /*
   * Get others message attribute
   */

  if (nargs >= 4) {
    ap = attribute_by_name(evaluation->world->database, args[3]);
    if (ap && (ap->number > 0))
      owhat = ap->number;
  }
  /*
   * Get others message default
   */

  if ((nargs >= 5) && args[4] && *args[4]) {
    owhatd = args[4];
  }
  /*
   * Get action attribute
   */

  if (nargs >= 6) {
    ap = attribute_by_name(evaluation->world->database, args[5]);
    if (ap)
      awhat = ap->number;
  }
  /*
   * Get arguments
   */

  if (nargs >= 7) {
    parse_arglist(evaluation, victim, actor, args[6], '\0',
                  EV_STRIP_LS | EV_STRIP_TS, xargs, 10, (char **)nullptr, 0);
    for (nxargs = 0; (nxargs < 10) && xargs[nxargs]; nxargs++)
      ;
  }
  /*
   * If player doesn't control both, enforce visibility restrictions
   */

  if (restriction) {
    ap = nullptr;
    if (what != -1) {
      attribute_get_info(evaluation->world->database, victim, what, &aowner,
                         &aflags);
      ap = attribute_by_number(evaluation->world->database, what);
    }
    if (!ap || !read_attr(evaluation, player, victim, ap, aowner, aflags) ||
        ((ap->number == A_DESC) && !configuration->read_rem_desc &&
         !is_examinable(evaluation, player, victim) &&
         !nearby(evaluation->world->database, player, victim)))
      what = -1;

    ap = nullptr;
    if (owhat != -1) {
      attribute_get_info(evaluation->world->database, victim, owhat, &aowner,
                         &aflags);
      ap = attribute_by_number(evaluation->world->database, owhat);
    }
    if (!ap || !read_attr(evaluation, player, victim, ap, aowner, aflags) ||
        ((ap->number == A_DESC) && !configuration->read_rem_desc &&
         !is_examinable(evaluation, player, victim) &&
         !nearby(evaluation->world->database, player, victim)))
      owhat = -1;

    awhat = 0;
  }
  /*
   * Go do it
   */

  did_it(evaluation, actor, victim, what, whatd, owhat, owhatd, awhat, xargs,
         nxargs);

  /*
   * Free user args
   */

  for (i = 0; i < nxargs; i++)
    free_lbuf(xargs[i]);
}
