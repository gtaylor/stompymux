/* game.c - Core game notifications, database dumps, and shutdown operations. */

#include "mux/server/platform.h"

#include <regex.h>

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/commands/command_queue.h"
#include "mux/commands/functions.h"
#include "mux/commands/macro.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/help/help_index.h"
#include "mux/network/connect_flow.h"
#include "mux/persistence/commac_persistence.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/file_cache.h"
#include "mux/server/game.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/version.h"
#include "mux/support/alloc.h"
#include "mux/support/password.h"
#include "mux/world/match.h"
#include "persistence/btech_persistence.h"
#ifndef NEXT
#endif

#define NSUBEXP 10

extern void init_cmdtab(CommandRegistry *registry);

void do_dump_optimize(EvaluationContext *, DbRef, DbRef, int);
static void init_rlimit(MuxServer *server);

/*
 * used to allocate storage for temporary stuff, cleared before command
 * execution
 */

void do_dump(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  int key = invocation->key;
  notify(evaluation, player, "Dumping...");

  /*
   * DUMP_OPTIMIZE takes advantage of a feature of GDBM to compress
   * unused space in the database, and will not be very useful
   * except sparingly, perhaps done every month or so.
   */

  if (key & DUMP_OPTIMIZE)
    do_dump_optimize(evaluation, player, invocation->cause, key);
  else
    fork_and_dump(invocation->context->runtime->server_control, key);
}

void do_dump_optimize(EvaluationContext *evaluation, DbRef player, DbRef cause,
                      int key) {
  raw_notify(evaluation, player, "Database is memory based.");
}

/**
 * print out stuff into error file
 */
void report(CommandContext *command) {
  STARTLOG(command->log, LOG_BUGS, "BUG", "INFO") {
    log_text("Command: '");
    log_text(command->debug_command);
    log_text("'");
    ENDLOG(command->log);
  }
  if (is_good_obj(command->world->database, command->player)) {
    STARTLOG(command->log, LOG_BUGS, "BUG", "INFO") {
      log_text("Player: ");
      log_name_and_loc(command->log, command->player);
      if ((command->enactor != command->player) &&
          is_good_obj(command->world->database, command->enactor)) {
        log_text(" Enactor: ");
        log_name_and_loc(command->log, command->enactor);
      }
      ENDLOG(command->log);
    }
  }
}

/*
 * Load a regular expression match and insert it into
 * registers.
 */
static int regexp_match(const char *pattern, const char *str, char *args[],
                        int nargs) {
  regex_t re;
  int got_match;
  regmatch_t pmatch[NSUBEXP];
  int i, len;

  /*
   * Load the regexp pattern. This allocates memory which must be
   * later freed. A free() of the regexp does free all structures
   * under it.
   */

  if (regcomp(&re, pattern, REG_EXTENDED) != 0) {
    /*
     * This is a matching error. We have an error message in
     * regexp_errbuf that we can ignore, since we're doing
     * command-matching.
     */
    return 0;
  }

  /*
   * Now we try to match the pattern. The relevant fields will
   * automatically be filled in by this.
   */
  got_match = (regexec(&re, str, NSUBEXP, pmatch, 0) == 0);
  if (!got_match) {
    regfree(&re);
    return 0;
  }

  /*
   * Now we fill in our args vector. Note that in regexp matching,
   * 0 is the entire string matched, and the parenthesized strings
   * go from 1 to 9. We DO PRESERVE THIS PARADIGM, for consistency
   * with other languages.
   */

  for (i = 0; i < nargs; i++) {
    args[i] = nullptr;
  }

  /* Convenient: nargs and NSUBEXP are the same.
   * We are also guaranteed that our buffer is going to be LBUF_SIZE
   * so we can copy without fear.
   */

  for (i = 0;
       (i < NSUBEXP) && (pmatch[i].rm_so != -1) && (pmatch[i].rm_eo != -1);
       i++) {
    len = pmatch[i].rm_eo - pmatch[i].rm_so;
    args[i] = alloc_lbuf("regexp_match");
    memset(args[i], 0, LBUF_SIZE);
    strncpy(args[i], str + pmatch[i].rm_so, (size_t)len);
    args[i][len] = '\0'; /* strncpy() does not null-terminate */
  }

  regfree(&re);
  return 1;
}

/**
 * Check attribute list for wild card matches and queue them.
 */
static int attribute_match_one(EvaluationContext *evaluation, DbRef thing,
                               DbRef parent, DbRef player, char type,
                               const char *str, int check_exclude,
                               int hash_insert) {
  DbRef aowner;
  int match, attr, i;
  long aflags;
  char buff[LBUF_SIZE], *s, *as;
  char *args[10];
  Attribute *ap;
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  memset(args, 0, sizeof(args));

  /*
   * See if we can do it.  Silently fail if we can't.
   */

  if (!lock_test(evaluation, player, player, player, parent, LUA_LOCK_USE,
                 LUA_LOCK_OPERATION_COMMAND_MATCH, true, &lock, &lock_result))
    return -1;

  match = 0;
  for (attr = attribute_list_first(evaluation->world->database, parent, &as);
       attr; attr = attribute_list_next(&as)) {
    ap = attribute_by_number(evaluation->world->database, attr);

    /*
     * Never check NOPROG attributes.
     */

    if (!ap || (ap->flags & AF_NOPROG))
      continue;

    /*
     * If we aren't the bottom level check if we saw this attr *
     * * * * before.  Also exclude it if the attribute type is *
     * * PRIVATE.
     */

    if (check_exclude &&
        ((ap->flags & AF_PRIVATE) ||
         numeric_hash_table_find(
             ap->number, &evaluation->world->indexes->parent_commands))) {
      continue;
    }
    attribute_get_string(evaluation->world->database, buff, parent, attr,
                         &aowner, &aflags);

    /*
     * Skip if private and on a parent
     */

    if (check_exclude && (aflags & AF_PRIVATE)) {
      continue;
    }
    /*
     * If we aren't the top level remember this attr so we * * *
     * exclude * it from now on.
     */

    if (hash_insert)
      numeric_hash_table_add(ap->number, (int *)&attr,
                             &evaluation->world->indexes->parent_commands);

    /*
     * Check for the leadin character after excluding the attrib
     * * * * * This lets non-command attribs on the child block *
     * *  * commands * on the parent.
     */

    if ((buff[0] != type) || (aflags & AF_NOPROG))
      continue;

    /*
     * decode it: search for first un escaped :
     */

    for (s = buff + 1; *s && (*s != ':'); s++)
      ;
    if (!*s)
      continue;
    *s++ = 0;
    if (((aflags & AF_REGEXP) && regexp_match(buff + 1, str, args, 10)) ||
        wild(buff + 1, str, args, 10)) {
      match = 1;
      wait_que(evaluation->runtime->commands, thing, player, 0, NOTHING, 0, s,
               args, 10, evaluation->registers);
      for (i = 0; i < 10; i++) {
        if (args[i])
          free_lbuf(args[i]);
      }
    }
  }
  return (match);
}

int attribute_match(EvaluationContext *evaluation, DbRef thing, DbRef player,
                    char type, const char *str, int check_parents) {
  int match, lev, result, exclude, insert;
  DbRef parent;

  /*
   * If thing is halted, don't check anything
   */

  if (is_halted(evaluation->world->database, thing))
    return 0;

  /*
   * If not checking parents, just check the thing
   */

  match = 0;
  if (!check_parents)
    return attribute_match_one(evaluation, thing, thing, player, type, str, 0,
                               0);

  /*
   * Check parents, ignoring halted objects
   */

  exclude = 0;
  insert = 1;
  numeric_hash_table_flush(&evaluation->world->indexes->parent_commands, 0);
  ITER_PARENTS(evaluation->world->database, evaluation->world->configuration,
               thing, parent, lev) {
    if (!is_good_obj(evaluation->world->database,
                     game_object_parent(evaluation->world->database, parent)))
      insert = 0;
    result = attribute_match_one(evaluation, thing, parent, player, type, str,
                                 exclude, insert);
    if (result > 0) {
      match = 1;
    } else if (result < 0) {
      return match;
    }
    exclude = 1;
  }

  return match;
}

/**
 * Notifies the object #target of the message msg, and
 * optionally notify the contents, neighbors, and location also.
 */
int check_filter(EvaluationContext *evaluation, DbRef object, DbRef player,
                 int filter, const char *msg) {
  long aflags;
  DbRef aowner;
  char *buf, *nbuf, *cp, *dp, *str;

  buf = attribute_parent_get(evaluation->world->database, object, filter,
                             &aowner, &aflags);
  if (!*buf) {
    free_lbuf(buf);
    return (1);
  }
  nbuf = dp = alloc_lbuf("check_filter");
  str = buf;
  exec(evaluation, nbuf, &dp, 0, object, player, EV_FIGNORE | EV_EVAL | EV_TOP,
       &str, (char **)nullptr, 0);
  *dp = '\0';
  dp = nbuf;
  free_lbuf(buf);
  do {
    cp = parse_to(evaluation->world->configuration, &dp, ',', EV_STRIP);
    if (quick_wild(cp, msg)) {
      free_lbuf(nbuf);
      return (0);
    }
  } while (dp != nullptr);
  free_lbuf(nbuf);
  return (1);
}

static char *add_prefix(EvaluationContext *evaluation, DbRef object,
                        DbRef player, int prefix, const char *msg,
                        const char *dflt) {
  long aflags;
  DbRef aowner;
  char *buf, *nbuf, *cp, *bp, *str;

  buf = attribute_parent_get(evaluation->world->database, object, prefix,
                             &aowner, &aflags);
  if (!*buf) {
    cp = buf;
    safe_str(dflt, buf, &cp);
  } else {
    nbuf = bp = alloc_lbuf("add_prefix");
    str = buf;
    exec(evaluation, nbuf, &bp, 0, object, player,
         EV_FIGNORE | EV_EVAL | EV_TOP, &str, (char **)nullptr, 0);
    *bp = '\0';
    free_lbuf(buf);
    buf = nbuf;
    cp = &buf[strlen(buf)];
  }
  if (cp != buf)
    safe_str(" ", buf, &cp);
  safe_str(msg, buf, &cp);
  *cp = '\0';
  return (buf);
}

static char *dflt_from_msg(GameDatabase *database, DbRef sender,
                           DbRef sendloc) {
  char *tp, *tbuff;

  tp = tbuff = alloc_lbuf("notify_checked.fwdlist");
  safe_str("From ", tbuff, &tp);
  if (is_good_obj(database, sendloc))
    safe_str(game_object_name(database, sendloc), tbuff, &tp);
  else
    safe_str(game_object_name(database, sender), tbuff, &tp);
  safe_chr(',', tbuff, &tp);
  *tp = '\0';
  return tbuff;
}

void notify_checked(EvaluationContext *evaluation, DbRef target, DbRef sender,
                    const char *msg, int key) {
  char *msg_ns, *mp, *tbuff, *tp, *buff, *colbuf = nullptr;
  char *args[10];
  DbRef aowner, targetloc, recip, obj;
  int i, nargs, has_neighbors, pass_listen;
  long aflags;
  int check_listens, pass_uselock, target_audible;
  FWDLIST *fp;
  LuaLockInvocation lock;
  LuaLockResult lock_result;

  /*
   * If speaker is invalid or message is empty, just exit
   */

  if (!is_good_obj(evaluation->world->database, target) || !msg || !*msg)
    return;

  /*
   * Enforce a recursion limit
   */

  evaluation->notification_nesting++;
  if (evaluation->notification_nesting >=
      evaluation->world->configuration->ntfy_nest_lim) {
    evaluation->notification_nesting--;
    return;
  }
  /*
   * If we want NOSPOOF output, generate it.  It is only needed if
   * we are sending the message to the target object
   */

  if (key & MSG_ME) {
    mp = msg_ns = alloc_lbuf("notify_checked");
    if (is_nospoof(evaluation->world->database, target) && (target != sender) &&
        (target != evaluation->command->enactor) &&
        (target != evaluation->command->player &&
         is_good_obj(evaluation->world->database, sender))) {

      /*
       * I'd really like to use tprintf here but I can't
       * because the caller may have.
       * notify(target, tprintf(...)) is quite common
       * in the code.
       */

      tbuff = alloc_sbuf("notify_checked.nospoof");
      safe_chr('[', msg_ns, &mp);
      safe_str(game_object_name(evaluation->world->database, sender), msg_ns,
               &mp);
      snprintf(tbuff, SBUF_SIZE, "(#%ld)", sender);
      safe_str(tbuff, msg_ns, &mp);

      if (sender != game_object_owner(evaluation->world->database, sender)) {
        safe_chr('{', msg_ns, &mp);
        safe_str(game_object_name(
                     evaluation->world->database,
                     game_object_owner(evaluation->world->database, sender)),
                 msg_ns, &mp);
        safe_chr('}', msg_ns, &mp);
      }
      if (sender != evaluation->command->enactor) {
        snprintf(tbuff, SBUF_SIZE, "<-(#%ld)", evaluation->command->enactor);
        safe_str(tbuff, msg_ns, &mp);
      }
      safe_str("] ", msg_ns, &mp);
      free_sbuf(tbuff);
    }
    safe_str(msg, msg_ns, &mp);
    *mp = '\0';
  } else {
    msg_ns = nullptr;
  }

  /*
   * msg contains the raw message, msg_ns contains the NOSPOOFed msg
   */

  check_listens = is_halted(evaluation->world->database, target) ? 0 : 1;
  switch (typeof_obj(evaluation->world->database, target)) {
  case TYPE_PLAYER:
    if (key & MSG_ME) {
      if (key & MSG_COLORIZE)
        colbuf = colorize(evaluation, target, msg_ns);
      raw_notify(evaluation, target, colbuf ? colbuf : msg_ns);
    }

    if (colbuf)
      free_lbuf(colbuf);
    if (!evaluation->world->configuration->player_listen)
      check_listens = 0;
    [[fallthrough]];
  case TYPE_THING:
  case TYPE_ROOM:

    /* If we're in a pipe, objects can receive raw_notify
     * if they're not a player and connected (if we didn't
     * do this, they'd be notified twice! */

    if (evaluation->is_piping &&
        (!is_player(evaluation->world->database, target) ||
         (is_player(evaluation->world->database, target) &&
          !is_connected(evaluation->world->database, target)))) {
      raw_notify(evaluation, target, msg_ns);
    }

    /*
     * Forward puppet message if it is for me
     */

    has_neighbors = has_location(evaluation->world->database, target);
    targetloc = where_is(evaluation->world->database, target);
    target_audible = is_audible(evaluation->world->database, target);

    if ((key & MSG_ME) && is_puppet(evaluation->world->database, target) &&
        (target != game_object_owner(evaluation->world->database, target)) &&
        ((key & MSG_PUP_ALWAYS) ||
         ((targetloc !=
           game_object_location(
               evaluation->world->database,
               game_object_owner(evaluation->world->database, target))) &&
          (targetloc !=
           game_object_owner(evaluation->world->database, target))))) {
      tp = tbuff = alloc_lbuf("notify_checked.puppet");
      safe_str(game_object_name(evaluation->world->database, target), tbuff,
               &tp);
      safe_str("> ", tbuff, &tp);
      if (key & MSG_COLORIZE)
        colbuf = colorize(
            evaluation, game_object_owner(evaluation->world->database, target),
            msg_ns);
      safe_str(colbuf ? colbuf : msg_ns, tbuff, &tp);
      *tp = '\0';
      raw_notify(evaluation,
                 game_object_owner(evaluation->world->database, target), tbuff);
      if (colbuf)
        free_lbuf(colbuf);
      free_lbuf(tbuff);
    }
    /*
     * Check for @Listen match if it will be useful
     */

    pass_listen = 0;
    nargs = 0;
    if (check_listens && (key & (MSG_ME | MSG_INV_L)) &&
        has_listen(evaluation->world->database, target)) {
      tp = attribute_get(evaluation->world->database, target, A_LISTEN, &aowner,
                         &aflags);
      if (*tp && wild(tp, msg, args, 10)) {
        for (nargs = 10; nargs && (!args[nargs - 1] || !(*args[nargs - 1]));
             nargs--)
          ;
        pass_listen = 1;
      }
      free_lbuf(tp);
    }
    /*
     * If we matched the @listen or are monitoring, check the * *
     * USE lock
     */

    if (sender < 0)
      sender = GOD;
    pass_uselock = 0;
    if ((key & MSG_ME) && check_listens &&
        (pass_listen || is_monitor(evaluation->world->database, target)))
      pass_uselock =
          lock_test(evaluation, sender, sender, sender, target, LUA_LOCK_USE,
                    LUA_LOCK_OPERATION_LISTEN, true, &lock, &lock_result);

    /*
     * Process AxHEAR if we pass LISTEN, USElock and it's for me
     */

    if ((key & MSG_ME) && pass_listen && pass_uselock) {
      if (sender != target)
        notify_action(evaluation, sender, target, 0, nullptr, 0, nullptr,
                      LUA_EVENT_MATCH_HEARD_OTHER, args, nargs);
      else
        notify_action(evaluation, sender, target, 0, nullptr, 0, nullptr,
                      LUA_EVENT_MATCH_HEARD_SELF, args, nargs);
      notify_action(evaluation, sender, target, 0, nullptr, 0, nullptr,
                    LUA_EVENT_MATCH_HEARD, args, nargs);
    }
    /*
     * Get rid of match arguments. We don't need them anymore
     */

    if (pass_listen) {
      for (i = 0; i < 10; i++)
        if (args[i] != nullptr)
          free_lbuf(args[i]);
    }
    /*
     * Process ^-listens if for me, MONITOR, and we pass USElock
     */
    /*
     * \todo Eventually come up with a cleaner method for making sure
     * the sender isn't the same as the target.
     */
    if ((key & MSG_ME) &&
        (sender != target || is_wizard(evaluation->world->database, target)) &&
        pass_uselock && is_monitor(evaluation->world->database, target)) {
      (void)attribute_match(evaluation, target, sender, AMATCH_LISTEN, msg, 0);
    }
    /*
     * Deliver message to forwardlist members
     */

    if ((key & MSG_FWDLIST) &&
        is_audible(evaluation->world->database, target) &&
        check_filter(evaluation, target, sender, A_FILTER, msg)) {
      tbuff = dflt_from_msg(evaluation->world->database, sender, target);
      buff = add_prefix(evaluation, target, sender, A_PREFIX, msg, tbuff);
      free_lbuf(tbuff);

      fp = fwdlist_get(evaluation->world->database, target);
      if (fp) {
        for (i = 0; i < fp->count; i++) {
          recip = fp->data[i];
          if (!is_good_obj(evaluation->world->database, recip) ||
              (recip == target))
            continue;
          notify_checked(evaluation, recip, sender, buff,
                         (MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE));
        }
      }
      free_lbuf(buff);
    }
    /*
     * Deliver message through audible exits
     */

    if (key & MSG_INV_EXITS) {
      DOLIST(evaluation->world->database, obj,
             game_object_exits(evaluation->world->database, target)) {
        recip = game_object_location(evaluation->world->database, obj);
        if (is_audible(evaluation->world->database, obj) &&
            ((recip != target) &&
             check_filter(evaluation, obj, sender, A_FILTER, msg))) {
          buff = add_prefix(evaluation, obj, target, A_PREFIX, msg,
                            "From a distance,");
          notify_checked(evaluation, recip, sender, buff,
                         MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE);
          free_lbuf(buff);
        }
      }
    }
    /*
     * Deliver message through neighboring audible exits
     */

    if (has_neighbors && ((key & MSG_NBR_EXITS) ||
                          ((key & MSG_NBR_EXITS_A) && target_audible))) {

      /*
       * If from inside, we have to add the prefix string *
       *
       * *  * * of * the container.
       */

      if (key & MSG_S_INSIDE) {
        tbuff = dflt_from_msg(evaluation->world->database, sender, target);
        buff = add_prefix(evaluation, target, sender, A_PREFIX, msg, tbuff);
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix(evaluation, ) allocation.
         */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }

      DOLIST(evaluation->world->database, obj,
             game_object_exits(
                 evaluation->world->database,
                 game_object_location(evaluation->world->database, target))) {
        recip = game_object_location(evaluation->world->database, obj);
        if (is_good_obj(evaluation->world->database, recip) &&
            is_audible(evaluation->world->database, obj) &&
            (recip != targetloc) && (recip != target) &&
            check_filter(evaluation, obj, sender, A_FILTER, msg)) {
          tbuff = add_prefix(evaluation, obj, target, A_PREFIX, buff,
                             "From a distance,");
          notify_checked(evaluation, recip, sender, tbuff,
                         MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE);
          free_lbuf(tbuff);
        }
      }
      if (key & MSG_S_INSIDE) {
        free_lbuf(buff);
      }
    }
    /*
     * Deliver message to contents
     */

    if (((key & MSG_INV) || ((key & MSG_INV_L) && pass_listen)) &&
        (check_filter(evaluation, target, sender, A_INFILTER, msg))) {

      /*
       * Don't prefix the message if we were given the * *
       * * * MSG_NOPREFIX key.
       */

      if (key & MSG_S_OUTSIDE) {
        buff = add_prefix(evaluation, target, sender, A_INPREFIX, msg, "");
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix(evaluation, ) allocation.
         */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      DOLIST(evaluation->world->database, obj,
             game_object_contents(evaluation->world->database, target)) {
        if (obj != target) {
          notify_checked(evaluation, obj, sender, buff,
                         MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE);
        }
      }
      if (key & MSG_S_OUTSIDE)
        free_lbuf(buff);
    }
    /*
     * Deliver message to neighbors
     */

    if (has_neighbors &&
        ((key & MSG_NBR) ||
         ((key & MSG_NBR_A) && target_audible &&
          check_filter(evaluation, target, sender, A_FILTER, msg)))) {
      if (key & MSG_S_INSIDE) {
        tbuff = dflt_from_msg(evaluation->world->database, sender, target);
        buff = add_prefix(evaluation, target, sender, A_PREFIX, msg, "");
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix(evaluation, ) allocation.
         */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      DOLIST(evaluation->world->database, obj,
             game_object_contents(evaluation->world->database, targetloc)) {
        if ((obj != target) && (obj != targetloc)) {
          notify_checked(evaluation, obj, sender, buff,
                         MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE |
                             (key & MSG_COLORIZE));
        }
      }
      if (key & MSG_S_INSIDE) {
        free_lbuf(buff);
      }
    }
    /*
     * Deliver message to container
     */

    if (has_neighbors &&
        ((key & MSG_LOC) ||
         ((key & MSG_LOC_A) && target_audible &&
          check_filter(evaluation, target, sender, A_FILTER, msg)))) {
      if (key & MSG_S_INSIDE) {
        tbuff = dflt_from_msg(evaluation->world->database, sender, target);
        buff = add_prefix(evaluation, target, sender, A_PREFIX, msg, tbuff);
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix(evaluation, ) allocation.
         */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      notify_checked(evaluation, targetloc, sender, buff,
                     MSG_ME | MSG_F_UP | MSG_S_INSIDE);
      if (key & MSG_S_INSIDE) {
        free_lbuf(buff);
      }
    }
    break;
  default:
    break;
  }
  if (msg_ns)
    free_lbuf(msg_ns);
  evaluation->notification_nesting--;
}

void notify_except(EvaluationContext *evaluation, DbRef loc, DbRef player,
                   DbRef exception, const char *msg) {
  DbRef first;

  if (loc != exception)
    notify_checked(evaluation, loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A));
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, loc)) {
    if (first != exception)
      notify_checked(evaluation, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
  }
}

void notify_except2(EvaluationContext *evaluation, DbRef loc, DbRef player,
                    DbRef exc1, DbRef exc2, const char *msg) {
  DbRef first;

  if ((loc != exc1) && (loc != exc2))
    notify_checked(evaluation, loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A));
  DOLIST(evaluation->world->database, first,
         game_object_contents(evaluation->world->database, loc)) {
    if (first != exc1 && first != exc2) {
      notify_checked(evaluation, first, player, msg,
                     (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
    }
  }
}

void do_shutdown(CommandInvocation *invocation) {
  server_shutdown(invocation->context->runtime->server_control,
                  invocation->player, invocation->key, invocation->first);
}

void server_shutdown(ServerControl *control, DbRef player, int key,
                     const char *message) {
  ResetSpecialObjects(control->btech);
  if (player != NOTHING) {
    raw_broadcast(
        control->descriptors, 0, "Game: Shutdown by %s",
        game_object_name(control->database,
                         game_object_owner(control->database, player)));
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown by ");
      log_name(control->log, player);
      ENDLOG(control->log);
    }
  } else {
    raw_broadcast(control->descriptors, 0, "Game: Fatal Error: %s", message);
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Fatal error: ");
      log_text(message);
      ENDLOG(control->log);
    }
  }
  if (player != NOTHING) {
    STARTLOG(control->log, LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown status: ");
      log_text(message);
      ENDLOG(control->log);
    }
  }

  /*
   * Do we perform a normal or an emergency shutdown?  Normal shutdown
   * * * * * is handled by exiting the server lifecycle event loop,
   * emergency  * * * * shutdown is done here.
   */

  if (key & SHUTDN_PANIC) {

    /*
     * Close down the network interface
     */

    server_lifecycle_close_connections(control->lifecycle, true,
                                       "Going down - Bye.\n");

    /*
     * Close the attribute text db and dump the header db
     */

    pcache_sync(control->players);
    STARTLOG(control->log, LOG_ALWAYS, "DMP", "PANIC") {
      log_text("Panic dump: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
    dump_database_internal(control, DUMP_CRASHED);

    STARTLOG(control->log, LOG_ALWAYS, "DMP", "DONE") {
      log_text("Panic dump complete: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
  } else if (key & SHUTDN_KILLED) {
    pcache_sync(control->players);
    STARTLOG(control->log, LOG_ALWAYS, "DMP", "KILLED") {
      log_text("Killed dump: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
    dump_database_internal(control, DUMP_KILLED);
    STARTLOG(control->log, LOG_ALWAYS, "DMP", "DONE") {
      log_text("Killed dump complete: ");
      log_text(control->configuration->database.gamedb);
      ENDLOG(control->log);
    }
  }
  /*
   * Set up for normal shutdown
   */

  server_lifecycle_stop(control->lifecycle);
  return;
}

int dump_database_internal(ServerControl *control, int dump_type) {
  return gamedb_dump(control->persistence, dump_type);
}

void dump_database(ServerControl *control) {
  STARTLOG(control->log, LOG_DBSAVES, "DMP", "DUMP") {
    log_text("Dumping: ");
    log_text(control->configuration->database.gamedb);
    ENDLOG(control->log);
  }
  pcache_sync(control->players);

  dump_database_internal(control, DUMP_NORMAL);
  STARTLOG(control->log, LOG_DBSAVES, "DMP", "DONE") {
    log_text("Dump complete: ");
    log_text(control->configuration->database.gamedb);
    ENDLOG(control->log);
  }
}

void fork_and_dump(ServerControl *control, int key) {
  if (*control->configuration->dump_msg)
    raw_broadcast(control->descriptors, 0, "%s",
                  control->configuration->dump_msg);

  log_error(control->log, LOG_DBSAVES, "DMP", "CHKPT", "Saving database: %s",
            control->configuration->database.gamedb);

  pcache_sync(control->players);

  if (!key || (key & DUMP_STRUCT)) {
    if (control->configuration->fork_dump) {
      /* Fork and dump.  */
      switch (fork()) {
      case -1: /* fork() failed */
        /* FIXME: Make this error message conform.  */
        log_perror(control->log, "DMP", "FAIL", nullptr, "fork()");
        return;

      case 0: /* child */
        dprintk("child database write process starting.");
        server_lifecycle_unbind_signals(control->lifecycle);
        dump_database_internal(control, DUMP_NORMAL);
        dprintk("child database write process finished.");
        /* You generally don't want to run atexit()
         * handlers and that sort of thing.  */
        _exit(0);
        break;

      default: /* parent */
        break;
      }
    } else {
      /* Just dump.  */
      dump_database_internal(control, DUMP_NORMAL);
    }
  }

  if (*control->configuration->postdump_msg)
    raw_broadcast(control->descriptors, 0, "%s",
                  control->configuration->postdump_msg);
}

static int load_game(MuxServer *server) {
  STARTLOG(&server->log, LOG_STARTUP, "INI", "LOAD") {
    log_text("Loading: ");
    log_text(server->configuration->database.gamedb);
    ENDLOG(&server->log);
  };
  if (gamedb_load(&server->persistence,
                  server->configuration->database.gamedb) < 0) {
    STARTLOG(&server->log, LOG_ALWAYS, "INI", "FATAL") {
      log_text("Error loading ");
      log_text(server->configuration->database.gamedb);
      ENDLOG(&server->log);
    }
    return -1;
  }

  /* Load the mecha stuff.. */
  if (server->configuration->have_specials)
    LoadSpecialObjects(&server->btech);

  STARTLOG(&server->log, LOG_STARTUP, "INI", "LOAD") {
    log_text("Load complete.");
    ENDLOG(&server->log);
  }
  /*
   * everything ok
   */
  return (0);
}

/**
 * match a list of things, using the no_command flag
 */
int list_check(EvaluationContext *evaluation, DbRef thing, DbRef player,
               char type, char *str, int check_parent) {
  int match, limit;

  match = 0;
  limit = evaluation->world->database->top;
  while (thing != NOTHING) {
    if ((thing != player) &&
        (!(is_no_command(evaluation->world->database, thing)))) {
      if (attribute_match(evaluation, thing, player, type, str, check_parent) >
          0)
        match = 1;
    }
    thing = game_object_next(evaluation->world->database, thing);
    if (--limit < 0)
      return match;
  }
  return match;
}

int is_hearer(EvaluationContext *evaluation, DbRef thing) {
  char *as, *buff, *s;
  DbRef aowner;
  int attr;
  long aflags;
  Attribute *ap;

  if (evaluation->is_piping && (thing == evaluation->pipe_object))
    return 1;

  if (is_connected(evaluation->world->database, thing) ||
      is_puppet(evaluation->world->database, thing))
    return 1;

  if (is_monitor(evaluation->world->database, thing))
    buff = alloc_lbuf("Hearer");
  else
    buff = nullptr;
  for (attr = attribute_list_first(evaluation->world->database, thing, &as);
       attr; attr = attribute_list_next(&as)) {
    if (attr == A_LISTEN) {
      if (buff)
        free_lbuf(buff);
      return 1;
    }
    if (buff && is_monitor(evaluation->world->database, thing)) {
      ap = attribute_by_number(evaluation->world->database, attr);
      if (!ap || (ap->flags & AF_NOPROG))
        continue;

      attribute_get_string(evaluation->world->database, buff, thing, attr,
                           &aowner, &aflags);

      /*
       * Make sure we can execute it
       */

      if ((buff[0] != AMATCH_LISTEN) || (aflags & AF_NOPROG))
        continue;

      /*
       * Make sure there's a : in it
       */

      for (s = buff + 1; *s && (*s != ':'); s++)
        ;
      if (s) {
        free_lbuf(buff);
        return 1;
      }
    }
  }
  if (buff)
    free_lbuf(buff);
  return 0;
}

void do_readcache(CommandInvocation *invocation) {
  fcache_load(&invocation->context->evaluation,
              invocation->context->runtime->files, invocation->player);
}

int main(int argc, char *argv[]) {
  MuxServer server;
  char *config_file;
  int mindb;

  if (argc > 3 || (argc > 2 && strcmp(argv[1], "-s")) ||
      (argc > 1 && !strcmp(argv[1], "--restart"))) {
    fprintf(stderr, "Usage: %s [-s] [config-file]\n", argv[0]);
    exit(1);
  }

  mindb = 0; /* Are we creating a new db? */
  /* config_file also gets assigned a genuinely mutable argv[] entry below,
     so it can't be const; CONF_FILE is only read as the default here. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
  config_file = (char *)CONF_FILE;
#pragma clang diagnostic pop
  if (argc > 1) {
    if (!strcmp(argv[1], "-s")) {
      mindb = 1;
      if (argc == 3)
        config_file = argv[2];
    } else {
      config_file = argv[1];
    }
  }
  if (!mux_server_create(&server)) {
    fprintf(stderr, "Unable to create MUX server resources.\n");
    exit(2);
  }
  time(&server.start_time);
  server.process_start_time = server.start_time;
  server.btech.process_start_time = server.process_start_time;
  server.database.top = -1;
  configuration_initialize(&server.configuration_context);
  init_rlimit(&server);
  init_cmdtab(&server.command_registry);
  init_mactab(&server.command_registry);
  init_chantab(&server.channels);
  init_flagtab(&server.world_indexes);
  init_powertab(&server.world_indexes);
  init_functab(&server);
  init_attrtab(&server.database);
  init_version(&server);

  hash_table_initialize(&server.world_indexes.players, 250 * HASH_FACTOR);
  numeric_hash_table_initialize(&server.world_indexes.forward_lists,
                                25 * HASH_FACTOR);
  numeric_hash_table_initialize(&server.world_indexes.parent_commands,
                                5 * HASH_FACTOR);
  configuration_read(&server.configuration_context, config_file);

  if (!password_initialize()) {
    fprintf(stderr, "Unable to initialize password hashing.\n");
    exit(2);
  }

  if (!*server.configuration->database.gamedb) {
    fprintf(stderr,
            "Required configuration directive game_database is missing.\n");
    exit(2);
  }

  if (btech_persistence_register(&server.persistence, &server.btech) < 0) {
    fprintf(stderr, "Unable to register BTech SQLite persistence.\n");
    exit(2);
  }

  if (commac_persistence_register(&server.persistence) < 0) {
    fprintf(stderr, "Unable to register commac SQLite persistence.\n");
    exit(2);
  }

  if (!mux_server_load_content(&server)) {
    fprintf(stderr, "Unable to load MUX server content.\n");
    exit(2);
  }
  db_free(&server.database);

  server.record_players = 0;

  if (mindb)
    db_make_minimal(&server.background_command.evaluation);
  else if (load_game(&server) < 0) {
    STARTLOG(&server.log, LOG_ALWAYS, "INI", "LOAD") {
      log_text("Couldn't load: ");
      log_text(server.configuration->database.gamedb);
      ENDLOG(&server.log);
    }
    exit(2);
  }
  server_lifecycle_prepare(server.lifecycle);

  /*
   * Do a consistency check and set up the freelist
   */

  database_check(&server.background_command.evaluation, NOTHING, 0);

  /*
   * Reset all the hash stats
   */

  hash_table_reset(&server.command_registry.commands);
  hash_table_reset(&server.command_registry.macros);
  channel_registry_reset_statistics(&server.channels);
  hash_table_reset(&server.command_registry.functions);
  hash_table_reset(&server.world_indexes.flags);
  hash_table_reset(&server.world_indexes.attributes);
  hash_table_reset(&server.world_indexes.players);
  numeric_hash_table_reset(&server.world_indexes.forward_lists);

  if (!server_lifecycle_boot(server.lifecycle, mindb)) {
    exit(2);
  }

#ifdef MCHECK
  mtrace();
#endif

  /*
   * go do it
   */

  server_lifecycle_run(server.lifecycle, server.configuration->port);

#ifdef MCHECK
  muntrace();
#endif

  server_lifecycle_close_connections(server.lifecycle, false,
                                     "Going down - Bye");
  dump_database(&server.server_control);

  mux_server_destroy(&server);
  exit(0);
}

static void init_rlimit(MuxServer *server) {
  struct rlimit *rlp;

  rlp = (struct rlimit *)alloc_lbuf("rlimit");

  if (getrlimit(RLIMIT_NOFILE, rlp)) {
    log_perror(&server->log, "RLM", "FAIL", nullptr, "getrlimit()");
    free_lbuf(rlp);
    return;
  }
  rlp->rlim_cur = rlp->rlim_max;
  if (setrlimit(RLIMIT_NOFILE, rlp))
    log_perror(&server->log, "RLM", "FAIL", nullptr, "setrlimit()");
  free_lbuf(rlp);
}
