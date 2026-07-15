/*
 * move.c -- Routines for moving about
 */

#include "mux/server/platform.h"

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/world/match.h"
#include "mux/world/move.h"

/*
 * ---------------------------------------------------------------------------
 * * process_leave_loc: Generate messages and actions resulting from leaving
 * * a place.
 */

static void process_leave_loc(DbRef thing, DbRef dest, DbRef cause, int canhear,
                              int hush) {
  DbRef loc;
  int quiet, pattr, oattr, aattr;

  loc = obj_location(thing);
  if ((loc == NOTHING) || (loc == dest))
    return;

  if (dest == HOME)
    dest = obj_home(thing);

  /*
   * Run the LEAVE attributes in the current room if we meet any of * *
   *
   * *  * * following criteria: * - The current room has wizard privs.
   * * - * * * Neither the current room nor the moving object are dark.
   * * - The * *  * moving object can hear and does not hav wizard
   * privs. * EXCEPT  * if * * we were called with the HUSH_LEAVE key.
   */

  quiet = (!(is_wizard(loc) || (!is_dark(thing) && !is_dark(loc)) ||
             (canhear && !(is_wizard(thing) && is_dark(thing))))) ||
          (hush & HUSH_LEAVE);
  oattr = quiet ? 0 : A_OLEAVE;
  aattr = quiet ? 0 : A_ALEAVE;
  pattr = A_LEAVE;
  did_it(thing, loc, pattr, NULL, oattr, NULL, aattr, (char **)NULL, 0);

  /*
   * Do OXENTER for receiving room
   */

  if ((dest != NOTHING) && !quiet)
    did_it(thing, dest, 0, NULL, A_OXENTER, NULL, 0, (char **)NULL, 0);

  /*
   * Display the 'has left' message if we meet any of the following * *
   *
   * *  * * criteria: * - Neither the current room nor the moving
   * object are  * *  * dark. * - The object can hear and is not a dark
   * wizard.
   */

  if (!quiet)
    if ((!is_dark(thing) && !is_dark(loc)) ||
        (canhear && !(is_wizard(thing) && is_dark(thing)))) {
      notify_except2(loc, thing, thing, cause,
                     tprintf("%s has left.", Name(thing)));
    }
}

/*
 * ---------------------------------------------------------------------------
 * * process_enter_loc: Generate messages and actions resulting from entering
 * * a place.
 */
static void process_enter_loc(DbRef thing, DbRef src, DbRef cause, int canhear,
                              int hush) {
  DbRef loc;
  int quiet, pattr, oattr, aattr;

  loc = obj_location(thing);
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

  quiet = (!(is_wizard(loc) || (!is_dark(thing) && !is_dark(loc)) ||
             (canhear && !(is_wizard(thing) && is_dark(thing))))) ||
          (hush & HUSH_ENTER);
  oattr = quiet ? 0 : A_OENTER;
  aattr = quiet ? 0 : A_AENTER;
  pattr = A_ENTER;
  did_it(thing, loc, pattr, NULL, oattr, NULL, aattr, (char **)NULL, 0);

  /*
   * Do OXLEAVE for sending room
   */

  if ((src != NOTHING) && !quiet)
    did_it(thing, src, 0, NULL, A_OXLEAVE, NULL, 0, (char **)NULL, 0);

  /*
   * Display the 'has arrived' message if we meet all of the following
   * * * * * criteria: * - The moving object can hear. * - The object
   * is * * not * a dark wizard.
   */

  if (!quiet && canhear && !(is_dark(thing) && is_wizard(thing))) {
    notify_except2(loc, thing, thing, cause,
                   tprintf("%s has arrived.", Name(thing)));
  }
}

/*
 * ---------------------------------------------------------------------------
 * * move_object: Physically move an object from one place to another.
 * * Does not generate any messages or actions.
 */

void move_object(DbRef thing, DbRef dest) {
  DbRef src;

  /*
   * Remove from the source location
   */

  src = obj_location(thing);
  if (src != NOTHING)
    s_contents(src, remove_first(obj_contents(src), thing));

  /*
   * Special check for HOME
   */

  if (dest == HOME)
    dest = obj_home(thing);

  /*
   * Add to destination location
   */

  if (dest != NOTHING)
    s_contents(dest, insert_first(obj_contents(dest), thing));
  else
    s_next(thing, NOTHING);
  s_location(thing, dest);

  look_in(thing, dest, LK_SHOWEXIT);
}

/*
 * ---------------------------------------------------------------------------
 * * send_dropto, process_sticky_dropto, process_dropped_dropto,
 * * process_sacrifice_dropto: Check for and process droptos.
 */

/*
 * send_dropto: Send an object through the dropto of a room
 */

static void send_dropto(DbRef thing, DbRef player) {
  if (!is_sticky(thing))
    move_via_generic(thing, obj_dropto(obj_location(thing)), player, 0);
  else
    move_via_generic(thing, HOME, player, 0);
  divest_object(thing);
}

/*
 * process_sticky_dropto: Call when an object leaves the room to see if
 * * we should empty the room
 */

static void process_sticky_dropto(DbRef loc, DbRef player) {
  DbRef dropto, thing, next;

  /*
   * Do nothing if checking anything but a sticky room
   */

  if (!is_good_obj(loc) || !has_dropto(loc) || !is_sticky(loc))
    return;

  /*
   * Make sure dropto loc is valid
   */

  dropto = obj_dropto(loc);
  if ((dropto == NOTHING) || (dropto == loc))
    return;

  /*
   * Make sure no players hanging out
   */

  DOLIST(thing, obj_contents(loc)) {
    if (is_connected(obj_owner(thing)) && is_hearer(thing))
      return;
  }

  /*
   * Send everything through the dropto
   */

  s_contents(loc, reverse_list(obj_contents(loc)));
  SAFE_DOLIST(thing, next, obj_contents(loc)) { send_dropto(thing, player); }
}

/*
 * process_dropped_dropto: Check what to do when someone drops an object.
 */

static void process_dropped_dropto(DbRef thing, DbRef player) {
  DbRef loc;

  /*
   * If STICKY, send home
   */

  if (is_sticky(thing)) {
    move_via_generic(thing, HOME, player, 0);
    divest_object(thing);
    return;
  }
  /*
   * Process the dropto if location is a room and is not STICKY
   */

  loc = obj_location(thing);
  if (has_dropto(loc) && (obj_dropto(loc) != NOTHING) && !is_sticky(loc))
    send_dropto(thing, player);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_generic: Generic move routine, generates standard messages and
 * * actions.
 */

void move_via_generic(DbRef thing, DbRef dest, DbRef cause, int hush) {
  DbRef src;
  int canhear;

  if (dest == HOME)
    dest = obj_home(thing);
  src = obj_location(thing);
  canhear = is_hearer(thing);
  process_leave_loc(thing, dest, cause, canhear, hush);
  move_object(thing, dest);
  did_it(thing, thing, A_MOVE, NULL, A_OMOVE, NULL, A_AMOVE, (char **)NULL, 0);
  process_enter_loc(thing, src, cause, canhear, hush);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_exit: Exit move routine, generic + exit messages + dropto check.
 */

void move_via_exit(DbRef thing, DbRef dest, DbRef cause, DbRef exit, int hush) {
  DbRef src;
  int canhear, darkwiz, quiet, pattr, oattr, aattr;

  if (dest == HOME)
    dest = obj_home(thing);
  src = obj_location(thing);
  canhear = is_hearer(thing);

  /*
   * Dark wizards don't trigger OSUCC/ASUCC
   */

  darkwiz = (is_wizard(thing) && is_dark(thing));
  quiet = darkwiz || (hush & HUSH_EXIT);

  oattr = quiet ? 0 : A_OSUCC;
  aattr = quiet ? 0 : A_ASUCC;
  pattr = A_SUCC;
  did_it(thing, exit, pattr, NULL, oattr, NULL, aattr, (char **)NULL, 0);
  process_leave_loc(thing, dest, cause, canhear, hush);
  move_object(thing, dest);

  /*
   * Dark wizards don't trigger ODROP/ADROP
   */

  oattr = quiet ? 0 : A_ODROP;
  aattr = quiet ? 0 : A_ADROP;
  pattr = A_DROP;
  did_it(thing, exit, pattr, NULL, oattr, NULL, aattr, (char **)NULL, 0);

  did_it(thing, thing, A_MOVE, NULL, A_OMOVE, NULL, A_AMOVE, (char **)NULL, 0);
  process_enter_loc(thing, src, cause, canhear, hush);
  process_sticky_dropto(src, thing);
}

/*
 * ---------------------------------------------------------------------------
 * * move_via_teleport: Teleport move routine, generic + teleport messages +
 * * divestiture + dropto check.
 */

int move_via_teleport(DbRef thing, DbRef dest, DbRef cause, int hush) {
  DbRef src, curr;
  int canhear, count;
  char *failmsg;

  src = obj_location(thing);
  if ((dest != HOME) && is_good_obj(src)) {
    curr = src;
    for (count = mudconf.ntfy_nest_lim; count > 0; count--) {
      if (!could_doit(thing, curr, A_LTELOUT)) {
        if ((thing == cause) || (cause == NOTHING))
          failmsg = (char *)"You can't teleport out!";
        else {
          failmsg = (char *)"You can't be teleported out!";
          notify_quiet(cause, "You can't teleport that out!");
        }
        did_it(thing, src, A_TOFAIL, failmsg, A_OTOFAIL, NULL, A_ATOFAIL,
               (char **)NULL, 0);
        return 0;
      }
      if (is_room(curr))
        break;
      curr = obj_location(curr);
    }
  }
  if (dest == HOME)
    dest = obj_home(thing);
  canhear = is_hearer(thing);
  if (!(hush & HUSH_LEAVE))
    did_it(thing, thing, 0, NULL, A_OXTPORT, NULL, 0, (char **)NULL, 0);
  process_leave_loc(thing, dest, NOTHING, canhear, hush);
  move_object(thing, dest);
  if (!(hush & HUSH_ENTER))
    did_it(thing, thing, A_TPORT, NULL, A_OTPORT, NULL, A_ATPORT, (char **)NULL,
           0);
  did_it(thing, thing, A_MOVE, NULL, A_OMOVE, NULL, A_AMOVE, (char **)NULL, 0);
  process_enter_loc(thing, src, NOTHING, canhear, hush);
  divest_object(thing);
  process_sticky_dropto(src, thing);
  return 1;
}

/*
 * ---------------------------------------------------------------------------
 * * move_exit: Try to move a player through an exit.
 */

void move_exit(DbRef player, DbRef exit, int divest, const char *failmsg,
               int hush) {
  DbRef loc;
  int oattr, aattr;

  loc = obj_location(exit);
  if (loc == HOME)
    loc = obj_home(player);
  if (is_good_obj(loc) && could_doit(player, exit, A_LOCK)) {
    switch (typeof_obj(loc)) {
    case TYPE_ROOM:
      move_via_exit(player, loc, NOTHING, exit, hush);
      if (divest)
        divest_object(player);
      break;
    case TYPE_PLAYER:
    case TYPE_THING:
      if (is_going(loc)) {
        notify(player, "You can't go that way.");
        return;
      }
      move_via_exit(player, loc, NOTHING, exit, hush);
      divest_object(player);
      break;
    case TYPE_EXIT:
      notify(player, "You can't go that way.");
      return;
    }
  } else {
    if ((is_wizard(player) && is_dark(player)) || (hush & HUSH_EXIT)) {
      oattr = 0;
      aattr = 0;
    } else {
      oattr = A_OFAIL;
      aattr = A_AFAIL;
    }
    did_it(player, exit, A_FAIL, failmsg, oattr, NULL, aattr, (char **)NULL, 0);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_move: Move from one place to another via exits or 'home'.
 */

void do_move(DbRef player, DbRef cause, int key, char *direction) {
  DbRef exit, loc;
  int i, quiet;

  if (!string_compare(direction, "home")) { /*
                                             * go home w/o stuff
                                             */
    if ((is_fixed(player) || is_fixed(obj_owner(player))) &&
        !(is_wizard(player))) {
      notify(player, mudconf.fixed_home_msg);
      return;
    }

    if ((loc = obj_location(player)) != NOTHING && !is_dark(player) &&
        !is_dark(loc)) {

      /*
       * tell all
       */
      char buffer[MBUF_SIZE];
      memset(buffer, 0, MBUF_SIZE);
      snprintf(buffer, MBUF_SIZE - 1, "%s goes home.", Name(player));
      notify_except(loc, player, player, buffer);
    }
    /*
     * give the player the messages
     */

    for (i = 0; i < 3; i++)
      notify(player, "There's no place like home...");
    move_via_generic(player, HOME, NOTHING, 0);
    divest_object(player);
    process_sticky_dropto(loc, player);
    return;
  }
  /*
   * find the exit
   */

  init_match_check_keys(player, direction, TYPE_EXIT);
  match_exit();
  exit = match_result();
  switch (exit) {
  case NOTHING: /*
                 * try to force the object
                 */
    notify(player, "You can't go that way.");
    break;
  case AMBIGUOUS:
    notify(player, "I don't know which way you mean!");
    break;
  default:
    quiet = 0;
    if ((key & MOVE_QUIET) && is_controls(player, exit))
      quiet = HUSH_EXIT;
    move_exit(player, exit, 0, "You can't go that way.", quiet);
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_get: Get an object.
 */

void do_get(DbRef player, DbRef cause, int key, char *what) {
  DbRef thing, playerloc, thingloc;
  char *failmsg;
  int oattr, aattr, quiet;

  playerloc = obj_location(player);
  if (!is_good_obj(playerloc))
    return;

  /*
   * You can only pick up things in rooms and ENTER_OK objects/players
   */

  if (!is_room(playerloc) && !is_enter_ok(playerloc) &&
      !is_controls(player, playerloc)) {
    notify(player, "Permission denied.");
    return;
  }
  /*
   * Look for the thing locally
   */

  init_match_check_keys(player, what, TYPE_THING);
  match_neighbor();
  match_exit();
  if (is_long_fingers(player))
    match_absolute(); /*
                       * long fingers
                       */
  thing = match_result();

  /*
   * Look for the thing in other people's inventories
   */

  if (!is_good_obj(thing))
    thing =
        match_status(player, match_possessed(player, player, what, thing, 1));
  if (!is_good_obj(thing))
    return;

  /*
   * If we found it, get it
   */

  quiet = 0;
  switch (typeof_obj(thing)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    /*
     * You can't take what you already have
     */

    thingloc = obj_location(thing);
    if (thingloc == player) {
      notify(player, "You already have that!");
      break;
    }
    if ((key & GET_QUIET) && is_controls(player, thing))
      quiet = 1;

    if (thing == player) {
      notify(player, "You cannot get yourself!");
    } else if (could_doit(player, thing, A_LOCK)) {
      if (thingloc != obj_location(player)) {
        notify_printf(thingloc, "%s was taken from you.", Name(thing));
      }
      move_via_generic(thing, player, player, 0);
      notify(thing, "Taken.");
      oattr = quiet ? 0 : A_OSUCC;
      aattr = quiet ? 0 : A_ASUCC;
      did_it(player, thing, A_SUCC, "Taken.", oattr, NULL, aattr, (char **)NULL,
             0);
    } else {
      oattr = quiet ? 0 : A_OFAIL;
      aattr = quiet ? 0 : A_AFAIL;
      if (thingloc != obj_location(player))
        failmsg = (char *)"You can't take that from there.";
      else
        failmsg = (char *)"You can't pick that up.";
      did_it(player, thing, A_FAIL, failmsg, oattr, NULL, aattr, (char **)NULL,
             0);
    }
    break;
  case TYPE_EXIT:
    /*
     * You can't take what you already have
     */

    thingloc = obj_exits(thing);
    if (thingloc == player) {
      notify(player, "You already have that!");
      break;
    }
    /*
     * You must control either the exit or the location
     */

    playerloc = obj_location(player);
    if (!is_controls(player, thing) && !is_controls(player, playerloc)) {
      notify(player, "Permission denied.");
      break;
    }
    /*
     * Do it
     */

    s_exits(thingloc, remove_first(obj_exits(thingloc), thing));
    s_exits(player, insert_first(obj_exits(player), thing));
    s_exits(thing, player);
    if (!is_quiet(player))
      notify(player, "Exit taken.");
    break;
  default:
    notify(player, "You can't take that!");
    break;
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_drop: Drop an object.
 */

void do_drop(DbRef player, DbRef cause, int key, char *name) {
  DbRef loc, exitloc, thing;
  char *buf, *bp;
  int quiet, oattr, aattr;

  loc = obj_location(player);
  if (!is_good_obj(loc))
    return;

  init_match(player, name, TYPE_THING);
  match_possession();
  match_carried_exit();

  switch (thing = match_result()) {
  case NOTHING:
    notify(player, "You don't have that!");
    return;
  case AMBIGUOUS:
    notify(player, "I don't know which you mean!");
    return;
  }

  switch (typeof_obj(thing)) {
  case TYPE_THING:
  case TYPE_PLAYER:

    /*
     * You have to be carrying it
     */

    if (((obj_location(thing) != player) && !is_wizard(player)) ||
        (!could_doit(player, thing, A_LDROP))) {
      did_it(player, thing, A_DFAIL, "You can't drop that.", A_ODFAIL, NULL,
             A_ADFAIL, (char **)NULL, 0);
      return;
    }
    /*
     * Move it
     */

    move_via_generic(thing, obj_location(player), player, 0);
    notify(thing, "Dropped.");
    quiet = 0;
    if ((key & DROP_QUIET) && is_controls(player, thing))
      quiet = 1;
    bp = buf = alloc_lbuf("do_drop.did_it");
    safe_tprintf_str(buf, &bp, "dropped %s.", Name(thing));
    oattr = quiet ? 0 : A_ODROP;
    aattr = quiet ? 0 : A_ADROP;
    did_it(player, thing, A_DROP, "Dropped.", oattr, buf, aattr, (char **)NULL,
           0);
    free_lbuf(buf);

    /*
     * Process droptos
     */

    process_dropped_dropto(thing, player);

    break;
  case TYPE_EXIT:

    /*
     * You have to be carrying it
     */

    if ((obj_exits(thing) != player) && !is_wizard(player)) {
      notify(player, "You can't drop that.");
      return;
    }
    if (!is_controls(player, loc)) {
      notify(player, "Permission denied.");
      return;
    }
    /*
     * Do it
     */

    exitloc = obj_exits(thing);
    s_exits(exitloc, remove_first(obj_exits(exitloc), thing));
    s_exits(loc, insert_first(obj_exits(loc), thing));
    s_exits(thing, loc);

    if (!is_quiet(player))
      notify(player, "Exit dropped.");
    break;
  default:
    notify(player, "You can't drop that.");
  }
}

/*
 * ---------------------------------------------------------------------------
 * * do_enter, do_leave: The enter and leave commands.
 */

void do_enter_internal(DbRef player, DbRef thing, int quiet) {
  DbRef loc = obj_location(player);
  int oattr, aattr;

  if (!is_enter_ok(thing) && !is_controls(player, thing)) {
    oattr = quiet ? 0 : A_OEFAIL;
    aattr = quiet ? 0 : A_AEFAIL;
    did_it(player, thing, A_EFAIL, "Permission denied.", oattr, NULL, aattr,
           (char **)NULL, 0);
  } else if (player == thing) {
    notify(player, "You can't enter yourself!");
#ifdef ENTER_REQUIRES_LEAVESUCC
  } else if (could_doit(player, thing, A_LENTER) &&
             could_doit(player, loc, A_LLEAVE))
#else
  } else if (could_doit(player, thing, A_LENTER))
#endif
  {
    oattr = quiet ? HUSH_ENTER : 0;
    move_via_generic(player, thing, NOTHING, oattr);
    divest_object(player);
    process_sticky_dropto(loc, player);
  } else {
    oattr = quiet ? 0 : A_OEFAIL;
    aattr = quiet ? 0 : A_AEFAIL;
    did_it(player, thing, A_EFAIL, "You can't enter that.", oattr, NULL, aattr,
           (char **)NULL, 0);
  }
}

void do_enter(DbRef player, DbRef cause, int key, char *what) {
  DbRef thing;
  int quiet;

  init_match(player, what, TYPE_THING);
  match_neighbor();
  if (is_long_fingers(player))
    match_absolute(); /*
                       * the wizard has long fingers
                       */

  if ((thing = noisy_match_result()) == NOTHING)
    return;

  switch (typeof_obj(thing)) {
  case TYPE_PLAYER:
  case TYPE_THING:
    quiet = 0;
    if ((key & MOVE_QUIET) && is_controls(player, thing))
      quiet = 1;
    do_enter_internal(player, thing, quiet);
    break;
  default:
    notify(player, "Permission denied.");
  }
  return;
}

void do_leave(DbRef player, DbRef cause, int key) {
  DbRef loc;
  int quiet, oattr, aattr;

  loc = obj_location(player);

  if (!is_good_obj(loc) || is_room(loc) || is_going(loc)) {
    notify(player, "You can't leave.");
    return;
  }
  quiet = 0;
  if ((key & MOVE_QUIET) && is_controls(player, loc))
    quiet = HUSH_LEAVE;
#ifdef LEAVE_REQUIRES_ENTERSUCC
  if (could_doit(player, loc, A_LLEAVE) &&
      could_doit(player, obj_location(loc), A_LENTER)) {
#else
  if (could_doit(player, loc, A_LLEAVE)) {
#endif
    move_via_generic(player, obj_location(loc), NOTHING, quiet);
  } else {
    oattr = quiet ? 0 : A_OLFAIL;
    aattr = quiet ? 0 : A_ALFAIL;
    did_it(player, loc, A_LFAIL, "You can't leave.", oattr, NULL, aattr,
           (char **)NULL, 0);
  }
}
