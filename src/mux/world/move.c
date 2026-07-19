/*
 * move.c -- Routines for moving about
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/commands/command.h"
#include "mux/commands/command_invocation.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/world/match.h"
#include "mux/world/move.h"
#include "mux/world/world_context.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

static void process_leave_loc(EvaluationContext *evaluation, DbRef thing,
                              DbRef dest, DbRef cause, int canhear, int hush) {
  DbRef loc;
  int quiet, pattr, oattr, aattr;

  loc = game_object_location(evaluation->world->database, thing);
  if ((loc == NOTHING) || (loc == dest))
    return;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);

  /*
   * Run the LEAVE attributes in the current room if we meet any of * *
   *
   * *  * * following criteria: * - The current room has wizard privs.
   * * - * * * Neither the current room nor the moving object are dark.
   * * - The * *  * moving object can hear and does not hav wizard
   * privs. * EXCEPT  * if * * we were called with the HUSH_LEAVE key.
   */

  quiet = (!(is_wizard(evaluation->world->database, loc) ||
             (!is_dark(evaluation->world->database, thing) &&
              !is_dark(evaluation->world->database, loc)) ||
             (canhear && !(is_wizard(evaluation->world->database, thing) &&
                           is_dark(evaluation->world->database, thing))))) ||
          (hush & HUSH_LEAVE);
  oattr = quiet ? 0 : A_OLEAVE;
  aattr = quiet ? 0 : A_ALEAVE;
  pattr = A_LEAVE;
  did_it(evaluation, thing, loc, pattr, nullptr, oattr, nullptr, aattr,
         (char **)nullptr, 0);

  /*
   * Do OXENTER for receiving room
   */

  if ((dest != NOTHING) && !quiet)
    did_it(evaluation, thing, dest, 0, nullptr, A_OXENTER, nullptr, 0,
           (char **)nullptr, 0);

  /*
   * Display the 'has left' message if we meet any of the following * *
   *
   * *  * * criteria: * - Neither the current room nor the moving
   * object are  * *  * dark. * - The object can hear and is not a dark
   * wizard.
   */

  if (!quiet)
    if ((!is_dark(evaluation->world->database, thing) &&
         !is_dark(evaluation->world->database, loc)) ||
        (canhear && !(is_wizard(evaluation->world->database, thing) &&
                      is_dark(evaluation->world->database, thing)))) {
      notify_except2(
          evaluation, loc, thing, thing, cause,
          tprintf("%s has left.",
                  game_object_name(evaluation->world->database, thing)));
    }
}

/*
 * ---------------------------------------------------------------------------
 * * process_enter_loc: Generate messages and actions resulting from entering
 * * a place.
 */
static void process_enter_loc(EvaluationContext *evaluation, DbRef thing,
                              DbRef src, DbRef cause, int canhear, int hush) {
  DbRef loc;
  int quiet, pattr, oattr, aattr;

  loc = game_object_location(evaluation->world->database, thing);
  if ((loc == NOTHING) || (loc == src))
    return;

  /*
   * Run the ENTER attributes in the current room if we meet any of * *
   *
   * *  * * following criteria: * - The current room has wizard privs.
   * * - * * * Neither the current room nor the moving object are dark.
   * * - The * *  * moving object can hear and does not hav wizard
   * privs. * EXCEPT  * if * * we were called with the HUSH_ENTER key.
   */

  quiet = (!(is_wizard(evaluation->world->database, loc) ||
             (!is_dark(evaluation->world->database, thing) &&
              !is_dark(evaluation->world->database, loc)) ||
             (canhear && !(is_wizard(evaluation->world->database, thing) &&
                           is_dark(evaluation->world->database, thing))))) ||
          (hush & HUSH_ENTER);
  oattr = quiet ? 0 : A_OENTER;
  aattr = quiet ? 0 : A_AENTER;
  pattr = A_ENTER;
  did_it(evaluation, thing, loc, pattr, nullptr, oattr, nullptr, aattr,
         (char **)nullptr, 0);

  /*
   * Do OXLEAVE for sending room
   */

  if ((src != NOTHING) && !quiet)
    did_it(evaluation, thing, src, 0, nullptr, A_OXLEAVE, nullptr, 0,
           (char **)nullptr, 0);

  /*
   * Display the 'has arrived' message if we meet all of the following
   * * * * * criteria: * - The moving object can hear. * - The object
   * is * * not * a dark wizard.
   */

  if (!quiet && canhear &&
      !(is_dark(evaluation->world->database, thing) &&
        is_wizard(evaluation->world->database, thing))) {
    notify_except2(
        evaluation, loc, thing, thing, cause,
        tprintf("%s has arrived.",
                game_object_name(evaluation->world->database, thing)));
  }
}

/*
 * ---------------------------------------------------------------------------
 * * move_object: Physically move an object from one place to another.
 * * Does not generate any messages or actions.
 */

void move_object(EvaluationContext *evaluation, DbRef thing, DbRef dest) {
  DbRef src;

  /*
   * Remove from the source location
   */

  src = game_object_location(evaluation->world->database, thing);
  if (src != NOTHING)
    game_object_set_contents(
        evaluation->world->database, src,
        remove_first(evaluation->world->database,
                     game_object_contents(evaluation->world->database, src),
                     thing));

  /*
   * Special check for HOME
   */

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);

  /*
   * Add to destination location
   */

  if (dest != NOTHING)
    game_object_set_contents(
        evaluation->world->database, dest,
        insert_first(evaluation->world->database,
                     game_object_contents(evaluation->world->database, dest),
                     thing));
  else
    game_object_set_next(evaluation->world->database, thing, NOTHING);
  game_object_set_location(evaluation->world->database, thing, dest);

  look_in(evaluation, thing, dest, LK_SHOWEXIT);
}

/*
 * ---------------------------------------------------------------------------
 * * send_dropto, process_sticky_dropto, process_dropped_dropto,
 * * process_sacrifice_dropto: Check for and process droptos.
 */

/*
 * send_dropto: Send an object through the dropto of a room
 */

static void send_dropto(EvaluationContext *evaluation, DbRef thing,
                        DbRef player) {
  if (!is_sticky(evaluation->world->database, thing))
    move_via_generic(
        evaluation, thing,
        game_object_location(
            evaluation->world->database,
            game_object_location(evaluation->world->database, thing)),
        player, 0);
  else
    move_via_generic(evaluation, thing, HOME, player, 0);
  divest_object(evaluation, thing);
}

/*
 * process_sticky_dropto: Call when an object leaves the room to see if
 * * we should empty the room
 */

static void process_sticky_dropto(EvaluationContext *evaluation, DbRef loc,
                                  DbRef player) {
  DbRef dropto, thing, next;

  /*
   * Do nothing if checking anything but a sticky room
   */

  if (!is_good_obj(evaluation->world->database, loc) ||
      !has_dropto(evaluation->world->database, loc) ||
      !is_sticky(evaluation->world->database, loc))
    return;

  /*
   * Make sure dropto loc is valid
   */

  dropto = game_object_location(evaluation->world->database, loc);
  if ((dropto == NOTHING) || (dropto == loc))
    return;

  /*
   * Make sure no players hanging out
   */

  DOLIST(evaluation->world->database, thing,
         game_object_contents(evaluation->world->database, loc)) {
    if (is_connected(evaluation->world->database,
                     game_object_owner(evaluation->world->database, thing)) &&
        is_hearer(evaluation, thing))
      return;
  }

  /*
   * Send everything through the dropto
   */

  game_object_set_contents(
      evaluation->world->database, loc,
      reverse_list(evaluation->world->database,
                   game_object_contents(evaluation->world->database, loc)));
  SAFE_DOLIST(evaluation->world->database, thing, next,
              game_object_contents(evaluation->world->database, loc)) {
    send_dropto(evaluation, thing, player);
  }
}

/*
 * process_dropped_dropto: Check what to do when someone drops an object.
 */

static void process_dropped_dropto(EvaluationContext *evaluation, DbRef thing,
                                   DbRef player) {
  DbRef loc;

  /*
   * If STICKY, send home
   */

  if (is_sticky(evaluation->world->database, thing)) {
    move_via_generic(evaluation, thing, HOME, player, 0);
    divest_object(evaluation, thing);
    return;
  }
  /*
   * Process the dropto if location is a room and is not STICKY
   */

  loc = game_object_location(evaluation->world->database, thing);
  if (has_dropto(evaluation->world->database, loc) &&
      (game_object_location(evaluation->world->database, loc) != NOTHING) &&
      !is_sticky(evaluation->world->database, loc))
    send_dropto(evaluation, thing, player);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_generic: Generic move routine, generates standard messages and
 * * actions.
 */

void move_via_generic(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                      DbRef cause, int hush) {
  DbRef src;
  int canhear;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  src = game_object_location(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);
  process_leave_loc(evaluation, thing, dest, cause, canhear, hush);
  move_object(evaluation, thing, dest);
  did_it(evaluation, thing, thing, A_MOVE, nullptr, A_OMOVE, nullptr, A_AMOVE,
         (char **)nullptr, 0);
  process_enter_loc(evaluation, thing, src, cause, canhear, hush);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_exit: Exit move routine, generic + exit messages + dropto check.
 */

void move_via_exit(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                   DbRef cause, DbRef exit, int hush) {
  DbRef src;
  int canhear, darkwiz, quiet, pattr, oattr, aattr;

  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  src = game_object_location(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);

  /*
   * Dark wizards don't trigger OSUCC/ASUCC
   */

  darkwiz = (is_wizard(evaluation->world->database, thing) &&
             is_dark(evaluation->world->database, thing));
  quiet = darkwiz || (hush & HUSH_EXIT);

  oattr = quiet ? 0 : A_OSUCC;
  aattr = quiet ? 0 : A_ASUCC;
  pattr = A_SUCC;
  did_it(evaluation, thing, exit, pattr, nullptr, oattr, nullptr, aattr,
         (char **)nullptr, 0);
  process_leave_loc(evaluation, thing, dest, cause, canhear, hush);
  move_object(evaluation, thing, dest);

  /*
   * Dark wizards don't trigger ODROP/ADROP
   */

  oattr = quiet ? 0 : A_ODROP;
  aattr = quiet ? 0 : A_ADROP;
  pattr = A_DROP;
  did_it(evaluation, thing, exit, pattr, nullptr, oattr, nullptr, aattr,
         (char **)nullptr, 0);

  did_it(evaluation, thing, thing, A_MOVE, nullptr, A_OMOVE, nullptr, A_AMOVE,
         (char **)nullptr, 0);
  process_enter_loc(evaluation, thing, src, cause, canhear, hush);
  process_sticky_dropto(evaluation, src, thing);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_teleport: Teleport move routine, generic + teleport messages +
 * * divestiture + dropto check.
 */

int move_via_teleport(EvaluationContext *evaluation, DbRef thing, DbRef dest,
                      DbRef cause, int hush) {
  DbRef src, curr;
  int canhear, count;
  const char *failmsg;
  const ServerConfiguration *configuration = evaluation->world->configuration;

  src = game_object_location(evaluation->world->database, thing);
  if ((dest != HOME) && is_good_obj(evaluation->world->database, src)) {
    curr = src;
    for (count = configuration->ntfy_nest_lim; count > 0; count--) {
      if (!could_doit_with_context(evaluation, thing, curr, A_LTELOUT)) {
        if ((thing == cause) || (cause == NOTHING))
          failmsg = "You can't teleport out!";
        else {
          failmsg = "You can't be teleported out!";
          notify_quiet(evaluation, cause, "You can't teleport that out!");
        }
        did_it(evaluation, thing, src, A_TOFAIL, failmsg, A_OTOFAIL, nullptr,
               A_ATOFAIL, (char **)nullptr, 0);
        return 0;
      }
      if (is_room(evaluation->world->database, curr))
        break;
      curr = game_object_location(evaluation->world->database, curr);
    }
  }
  if (dest == HOME)
    dest = game_object_link(evaluation->world->database, thing);
  canhear = is_hearer(evaluation, thing);
  if (!(hush & HUSH_LEAVE))
    did_it(evaluation, thing, thing, 0, nullptr, A_OXTPORT, nullptr, 0,
           (char **)nullptr, 0);
  process_leave_loc(evaluation, thing, dest, NOTHING, canhear, hush);
  move_object(evaluation, thing, dest);
  if (!(hush & HUSH_ENTER))
    did_it(evaluation, thing, thing, A_TPORT, nullptr, A_OTPORT, nullptr,
           A_ATPORT, (char **)nullptr, 0);
  did_it(evaluation, thing, thing, A_MOVE, nullptr, A_OMOVE, nullptr, A_AMOVE,
         (char **)nullptr, 0);
  process_enter_loc(evaluation, thing, src, NOTHING, canhear, hush);
  divest_object(evaluation, thing);
  process_sticky_dropto(evaluation, src, thing);
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * move_exit: Try to move a player through an exit.
 */

void move_exit(EvaluationContext *evaluation, DbRef player, DbRef exit,
               int divest, const char *failmsg, int hush) {
  DbRef loc;
  int oattr, aattr;

  loc = game_object_location(evaluation->world->database, exit);
  if (loc == HOME)
    loc = game_object_link(evaluation->world->database, player);
  if (is_good_obj(evaluation->world->database, loc) &&
      could_doit_with_context(evaluation, player, exit, A_LOCK)) {
    switch (typeof_obj(evaluation->world->database, loc)) {
    case TYPE_ROOM:
      move_via_exit(evaluation, player, loc, NOTHING, exit, hush);
      if (divest)
        divest_object(evaluation, player);
      break;
    case TYPE_PLAYER:
    case TYPE_THING:
      if (is_going(evaluation->world->database, loc)) {
        notify(evaluation, player, "You can't go that way.");
        return;
      }
      move_via_exit(evaluation, player, loc, NOTHING, exit, hush);
      divest_object(evaluation, player);
      break;
    case TYPE_EXIT:
      notify(evaluation, player, "You can't go that way.");
      return;
    default:
      break;
    }
  } else {
    if ((is_wizard(evaluation->world->database, player) &&
         is_dark(evaluation->world->database, player)) ||
        (hush & HUSH_EXIT)) {
      oattr = 0;
      aattr = 0;
    } else {
      oattr = A_OFAIL;
      aattr = A_AFAIL;
    }
    did_it(evaluation, player, exit, A_FAIL, failmsg, oattr, nullptr, aattr,
           (char **)nullptr, 0);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_move: Move from one place to another via exits or 'home'.
 */

void move_command(EvaluationContext *evaluation, DbRef player, DbRef cause,
                  int key, char *direction) {
  DbRef exit, loc;
  int i, quiet;
  const ServerConfiguration *configuration = evaluation->world->configuration;
  MatchContext *match = &evaluation->command->match;

  if (!string_compare(configuration, direction, "home")) { /*
                                                            * go home w/o stuff
                                                            */
    if ((is_fixed(evaluation->world->database, player) ||
         is_fixed(evaluation->world->database,
                  game_object_owner(evaluation->world->database, player))) &&
        !(is_wizard(evaluation->world->database, player))) {
      notify(evaluation, player, configuration->fixed_home_msg);
      return;
    }

    if ((loc = game_object_location(evaluation->world->database, player)) !=
            NOTHING &&
        !is_dark(evaluation->world->database, player) &&
        !is_dark(evaluation->world->database, loc)) {

      /*
       * tell all
       */
      char buffer[MBUF_SIZE];
      memset(buffer, 0, MBUF_SIZE);
      snprintf(buffer, MBUF_SIZE - 1, "%s goes home.",
               game_object_name(evaluation->world->database, player));
      notify_except(evaluation, loc, player, player, buffer);
    }
    /*
     * give the player the messages
     */

    for (i = 0; i < 3; i++)
      notify(evaluation, player, "There's no place like home...");
    move_via_generic(evaluation, player, HOME, NOTHING, 0);
    divest_object(evaluation, player);
    process_sticky_dropto(evaluation, loc, player);
    return;
  }
  /*
   * find the exit
   */

  init_match_check_keys(match, player, direction, TYPE_EXIT);
  match_exit(match);
  exit = match_result(match);
  switch (exit) {
  case NOTHING: /*
                 * try to force the object
                 */
    notify(evaluation, player, "You can't go that way.");
    break;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know which way you mean!");
    break;
  default:
    quiet = 0;
    if ((key & MOVE_QUIET) && is_controls(evaluation, player, exit))
      quiet = HUSH_EXIT;
    move_exit(evaluation, player, exit, 0, "You can't go that way.", quiet);
  }
}

void do_move(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  move_command(evaluation, invocation->player, invocation->cause,
               invocation->key, invocation->first);
}

/*
 * ---------------------------------------------------------------------------
 * * do_get: Get an object.
 */

void do_get(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing, playerloc, thingloc;
  const char *failmsg;
  int oattr, aattr, quiet;

  playerloc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, playerloc))
    return;

  /*
   * You can only pick up things in rooms and ENTER_OK objects/players
   */

  if (!is_room(evaluation->world->database, playerloc) &&
      !is_enter_ok(evaluation->world->database, playerloc) &&
      !is_controls(evaluation, player, playerloc)) {
    notify(evaluation, player, "Permission denied.");
    return;
  }
  /*
   * Look for the thing locally
   */

  MatchContext *match = &invocation->context->match;
  init_match_check_keys(match, player, what, TYPE_THING);
  match_neighbor(match);
  match_exit(match);
  if (is_long_fingers(evaluation->world->database, player))
    match_absolute(match); /*
                            * long fingers
                            */
  thing = match_result(match);

  /*
   * Look for the thing in other people's inventories
   */

  if (!is_good_obj(evaluation->world->database, thing))
    thing =
        match_status(evaluation, player,
                     match_possessed(match, player, player, what, thing, 1));
  if (!is_good_obj(evaluation->world->database, thing))
    return;

  /*
   * If we found it, get it
   */

  quiet = 0;
  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_location(evaluation->world->database, thing);
    if (thingloc == player) {
      notify(evaluation, player, "You already have that!");
      break;
    }
    if ((key & GET_QUIET) && is_controls(evaluation, player, thing))
      quiet = 1;

    if (thing == player) {
      notify(evaluation, player, "You cannot get yourself!");
    } else if (could_doit_with_context(evaluation, player, thing, A_LOCK)) {
      if (thingloc !=
          game_object_location(evaluation->world->database, player)) {
        notify_printf(evaluation, thingloc, "%s was taken from you.",
                      game_object_name(evaluation->world->database, thing));
      }
      move_via_generic(evaluation, thing, player, player, 0);
      notify(evaluation, thing, "Taken.");
      oattr = quiet ? 0 : A_OSUCC;
      aattr = quiet ? 0 : A_ASUCC;
      did_it(evaluation, player, thing, A_SUCC, "Taken.", oattr, nullptr, aattr,
             (char **)nullptr, 0);
    } else {
      oattr = quiet ? 0 : A_OFAIL;
      aattr = quiet ? 0 : A_AFAIL;
      if (thingloc != game_object_location(evaluation->world->database, player))
        failmsg = "You can't take that from there.";
      else
        failmsg = "You can't pick that up.";
      did_it(evaluation, player, thing, A_FAIL, failmsg, oattr, nullptr, aattr,
             (char **)nullptr, 0);
    }
    break;
  case TYPE_EXIT:
    /*
     * You can't take what you already have
     */

    thingloc = game_object_exits(evaluation->world->database, thing);
    if (thingloc == player) {
      notify(evaluation, player, "You already have that!");
      break;
    }
    /*
     * You must control either the exit or the location
     */

    playerloc = game_object_location(evaluation->world->database, player);
    if (!is_controls(evaluation, player, thing) &&
        !is_controls(evaluation, player, playerloc)) {
      notify(evaluation, player, "Permission denied.");
      break;
    }
    /*
     * Do it
     */

    game_object_set_exits(
        evaluation->world->database, thingloc,
        remove_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, thingloc),
                     thing));
    game_object_set_exits(
        evaluation->world->database, player,
        insert_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, player),
                     thing));
    game_object_set_exits(evaluation->world->database, thing, player);
    if (!is_quiet(evaluation->world->database, player))
      notify(evaluation, player, "Exit taken.");
    break;
  default:
    notify(evaluation, player, "You can't take that!");
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_drop: Drop an object.
 */

void do_drop(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *name = invocation->first;
  DbRef loc, exitloc, thing;
  char *buf, *bp;
  int quiet, oattr, aattr;

  loc = game_object_location(evaluation->world->database, player);
  if (!is_good_obj(evaluation->world->database, loc))
    return;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, name, TYPE_THING);
  match_possession(match);
  match_carried_exit(match);

  switch (thing = match_result(match)) {
  case NOTHING:
    notify(evaluation, player, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify(evaluation, player, "I don't know which you mean!");
    return;
  default:
    break;
  }

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_THING:
  case TYPE_PLAYER:

    /*
     * You have to be carrying it
     */

    if (((game_object_location(evaluation->world->database, thing) != player) &&
         !is_wizard(evaluation->world->database, player)) ||
        (!could_doit_with_context(evaluation, player, thing, A_LDROP))) {
      did_it(evaluation, player, thing, A_DFAIL, "You can't drop that.",
             A_ODFAIL, nullptr, A_ADFAIL, (char **)nullptr, 0);
      return;
    }
    /*
     * Move it
     */

    move_via_generic(evaluation, thing,
                     game_object_location(evaluation->world->database, player),
                     player, 0);
    notify(evaluation, thing, "Dropped.");
    quiet = 0;
    if ((key & DROP_QUIET) && is_controls(evaluation, player, thing))
      quiet = 1;
    bp = buf = alloc_lbuf("do_drop.did_it");
    safe_tprintf_str(buf, &bp, "dropped %s.",
                     game_object_name(evaluation->world->database, thing));
    oattr = quiet ? 0 : A_ODROP;
    aattr = quiet ? 0 : A_ADROP;
    did_it(evaluation, player, thing, A_DROP, "Dropped.", oattr, buf, aattr,
           (char **)nullptr, 0);
    free_lbuf(buf);

    /*
     * Process droptos
     */

    process_dropped_dropto(evaluation, thing, player);

    break;
  case TYPE_EXIT:

    /*
     * You have to be carrying it
     */

    if ((game_object_exits(evaluation->world->database, thing) != player) &&
        !is_wizard(evaluation->world->database, player)) {
      notify(evaluation, player, "You can't drop that.");
      return;
    }
    if (!is_controls(evaluation, player, loc)) {
      notify(evaluation, player, "Permission denied.");
      return;
    }
    /*
     * Do it
     */

    exitloc = game_object_exits(evaluation->world->database, thing);
    game_object_set_exits(
        evaluation->world->database, exitloc,
        remove_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, exitloc),
                     thing));
    game_object_set_exits(
        evaluation->world->database, loc,
        insert_first(evaluation->world->database,
                     game_object_exits(evaluation->world->database, loc),
                     thing));
    game_object_set_exits(evaluation->world->database, thing, loc);

    if (!is_quiet(evaluation->world->database, player))
      notify(evaluation, player, "Exit dropped.");
    break;
  default:
    notify(evaluation, player, "You can't drop that.");
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_enter, do_leave: The enter and leave commands.
 */

void do_enter_internal(EvaluationContext *evaluation, DbRef player, DbRef thing,
                       int quiet) {
  DbRef loc = game_object_location(evaluation->world->database, player);
  int oattr, aattr;

  if (!is_enter_ok(evaluation->world->database, thing) &&
      !is_controls(evaluation, player, thing)) {
    oattr = quiet ? 0 : A_OEFAIL;
    aattr = quiet ? 0 : A_AEFAIL;
    did_it(evaluation, player, thing, A_EFAIL, "Permission denied.", oattr,
           nullptr, aattr, (char **)nullptr, 0);
  } else if (player == thing) {
    notify(evaluation, player, "You can't enter yourself!");
  } else if (could_doit_with_context(evaluation, player, thing, A_LENTER) &&
             could_doit_with_context(evaluation, player, loc, A_LLEAVE)) {
    oattr = quiet ? HUSH_ENTER : 0;
    move_via_generic(evaluation, player, thing, NOTHING, oattr);
    divest_object(evaluation, player);
    process_sticky_dropto(evaluation, loc, player);
  } else {
    oattr = quiet ? 0 : A_OEFAIL;
    aattr = quiet ? 0 : A_AEFAIL;
    did_it(evaluation, player, thing, A_EFAIL, "You can't enter that.", oattr,
           nullptr, aattr, (char **)nullptr, 0);
  }
}

void do_enter(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  char *what = invocation->first;
  DbRef thing;
  int quiet;

  MatchContext *match = &invocation->context->match;
  init_match(match, player, what, TYPE_THING);
  match_neighbor(match);
  if (is_long_fingers(evaluation->world->database, player))
    match_absolute(match); /*
                            * the wizard has long fingers
                            */

  if ((thing = noisy_match_result(match)) == NOTHING)
    return;

  switch (typeof_obj(evaluation->world->database, thing)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    quiet = 0;
    if ((key & MOVE_QUIET) && is_controls(evaluation, player, thing))
      quiet = 1;
    do_enter_internal(evaluation, player, thing, quiet);
    break;
  default:
    notify(evaluation, player, "Permission denied.");
  }
  return;
}

void do_leave(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  DbRef loc;
  int quiet, oattr, aattr;

  loc = game_object_location(evaluation->world->database, player);

  if (!is_good_obj(evaluation->world->database, loc) ||
      is_room(evaluation->world->database, loc) ||
      is_going(evaluation->world->database, loc)) {
    notify(evaluation, player, "You can't leave.");
    return;
  }
  quiet = 0;
  if ((key & MOVE_QUIET) && is_controls(evaluation, player, loc))
    quiet = HUSH_LEAVE;
  if (could_doit_with_context(evaluation, player, loc, A_LLEAVE) &&
      could_doit_with_context(
          evaluation, player,
          game_object_location(evaluation->world->database, loc), A_LENTER)) {
    move_via_generic(evaluation, player,
                     game_object_location(evaluation->world->database, loc),
                     NOTHING, quiet);
  } else {
    oattr = quiet ? 0 : A_OLFAIL;
    aattr = quiet ? 0 : A_ALFAIL;
    did_it(evaluation, player, loc, A_LFAIL, "You can't leave.", oattr, nullptr,
           aattr, (char **)nullptr, 0);
  }
}
