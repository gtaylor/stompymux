/* game.c - Core game notifications, database dumps, and shutdown operations. */

#include "mux/server/platform.h"

#include <regex.h>

#include "p.glue.h"

#include "mux/commands/command.h"
#include "mux/commands/command_queue.h"
#include "mux/commands/macro.h"
#include "mux/communication/commac.h"
#include "mux/communication/comsys.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/help/help_index.h"
#include "mux/persistence/commac_persistence.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/file_cache.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/server/version.h"
#include "mux/support/alloc.h"
#include "mux/support/password.h"
#include "mux/world/match.h"
#include "persistence/btech_persistence.h"
#ifndef NEXT
#endif

#define NSUBEXP 10

extern void init_cmdtab(void);
extern void configuration_initialize(void);
extern void pcache_init(void);
extern int configuration_read(char *fn);
extern void init_functab(void);
extern void raw_notify(DbRef, const char *);
extern void do_dbck(DbRef, DbRef, int);

void fork_and_dump(int);
void dump_database(void);
void do_dump_optimize(DbRef, DbRef, int);
void pcache_sync(void);
int dump_database_internal(int);
static void init_rlimit(void);

int reserved;

extern int corrupt;

/*
 * used to allocate storage for temporary stuff, cleared before command
 * execution
 */

void do_dump(DbRef player, DbRef cause, int key) {
  notify(player, "Dumping...");

  /*
   * DUMP_OPTIMIZE takes advantage of a feature of GDBM to compress
   * unused space in the database, and will not be very useful
   * except sparingly, perhaps done every month or so.
   */

  if (key & DUMP_OPTIMIZE)
    do_dump_optimize(player, cause, key);
  else
    fork_and_dump(key);
}

void do_dump_optimize(DbRef player, DbRef cause, int key) {
  raw_notify(player, "Database is memory based.");
}

/**
 * print out stuff into error file
 */
void report(void) {
  STARTLOG(LOG_BUGS, "BUG", "INFO") {
    log_text("Command: '");
    log_text(mudstate.debug_cmd);
    log_text("'");
    ENDLOG;
  }
  if (is_good_obj(mudstate.curr_player)) {
    STARTLOG(LOG_BUGS, "BUG", "INFO") {
      log_text("Player: ");
      log_name_and_loc(mudstate.curr_player);
      if ((mudstate.curr_enactor != mudstate.curr_player) &&
          is_good_obj(mudstate.curr_enactor)) {
        log_text(" Enactor: ");
        log_name_and_loc(mudstate.curr_enactor);
      }
      ENDLOG;
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
static int attribute_match_one(DbRef thing, DbRef parent, DbRef player,
                               char type, const char *str, int check_exclude,
                               int hash_insert) {
  DbRef aowner;
  int match, attr, i;
  long aflags;
  char buff[LBUF_SIZE], *s, *as;
  char *args[10];
  Attribute *ap;

  memset(args, 0, sizeof(args));

  /*
   * See if we can do it.  Silently fail if we can't.
   */

  if (!could_doit(player, parent, A_LUSE))
    return -1;

  match = 0;
  for (attr = attribute_list_first(parent, &as); attr;
       attr = attribute_list_next(&as)) {
    ap = attribute_by_number(attr);

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
         numeric_hash_table_find(ap->number, &mudstate.parent_htab))) {
      continue;
    }
    attribute_get_string(buff, parent, attr, &aowner, &aflags);

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
      numeric_hash_table_add(ap->number, (int *)&attr, &mudstate.parent_htab);

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
      wait_que(thing, player, 0, NOTHING, 0, s, args, 10, mudstate.global_regs);
      for (i = 0; i < 10; i++) {
        if (args[i])
          free_lbuf(args[i]);
      }
    }
  }
  return (match);
}

int attribute_match(DbRef thing, DbRef player, char type, const char *str,
                    int check_parents) {
  int match, lev, result, exclude, insert;
  DbRef parent;

  /*
   * If thing is halted, don't check anything
   */

  if (is_halted(thing))
    return 0;

  /*
   * If not checking parents, just check the thing
   */

  match = 0;
  if (!check_parents)
    return attribute_match_one(thing, thing, player, type, str, 0, 0);

  /*
   * Check parents, ignoring halted objects
   */

  exclude = 0;
  insert = 1;
  numeric_hash_table_flush(&mudstate.parent_htab, 0);
  ITER_PARENTS(thing, parent, lev) {
    if (!is_good_obj(obj_parent(parent)))
      insert = 0;
    result =
        attribute_match_one(thing, parent, player, type, str, exclude, insert);
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
int check_filter(DbRef object, DbRef player, int filter, const char *msg) {
  long aflags;
  DbRef aowner;
  char *buf, *nbuf, *cp, *dp, *str;

  buf = attribute_parent_get(object, filter, &aowner, &aflags);
  if (!*buf) {
    free_lbuf(buf);
    return (1);
  }
  nbuf = dp = alloc_lbuf("check_filter");
  str = buf;
  exec(nbuf, &dp, 0, object, player, EV_FIGNORE | EV_EVAL | EV_TOP, &str,
       (char **)nullptr, 0);
  *dp = '\0';
  dp = nbuf;
  free_lbuf(buf);
  do {
    cp = parse_to(&dp, ',', EV_STRIP);
    if (quick_wild(cp, msg)) {
      free_lbuf(nbuf);
      return (0);
    }
  } while (dp != nullptr);
  free_lbuf(nbuf);
  return (1);
}

static char *add_prefix(DbRef object, DbRef player, int prefix, const char *msg,
                        const char *dflt) {
  long aflags;
  DbRef aowner;
  char *buf, *nbuf, *cp, *bp, *str;

  buf = attribute_parent_get(object, prefix, &aowner, &aflags);
  if (!*buf) {
    cp = buf;
    safe_str(dflt, buf, &cp);
  } else {
    nbuf = bp = alloc_lbuf("add_prefix");
    str = buf;
    exec(nbuf, &bp, 0, object, player, EV_FIGNORE | EV_EVAL | EV_TOP, &str,
         (char **)nullptr, 0);
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

static char *dflt_from_msg(DbRef sender, DbRef sendloc) {
  char *tp, *tbuff;

  tp = tbuff = alloc_lbuf("notify_checked.fwdlist");
  safe_str("From ", tbuff, &tp);
  if (is_good_obj(sendloc))
    safe_str(Name(sendloc), tbuff, &tp);
  else
    safe_str(Name(sender), tbuff, &tp);
  safe_chr(',', tbuff, &tp);
  *tp = '\0';
  return tbuff;
}

char *colorize(DbRef player, char *from);

void notify_checked(DbRef target, DbRef sender, const char *msg, int key) {
  char *msg_ns, *mp, *tbuff, *tp, *buff, *colbuf = nullptr;
  char *args[10];
  DbRef aowner, targetloc, recip, obj;
  int i, nargs, has_neighbors, pass_listen;
  long aflags;
  int check_listens, pass_uselock, target_audible;
  FWDLIST *fp;

  /*
   * If speaker is invalid or message is empty, just exit
   */

  if (!is_good_obj(target) || !msg || !*msg)
    return;

  /*
   * Enforce a recursion limit
   */

  mudstate.ntfy_nest_lev++;
  if (mudstate.ntfy_nest_lev >= mudconf.ntfy_nest_lim) {
    mudstate.ntfy_nest_lev--;
    return;
  }
  /*
   * If we want NOSPOOF output, generate it.  It is only needed if
   * we are sending the message to the target object
   */

  if (key & MSG_ME) {
    mp = msg_ns = alloc_lbuf("notify_checked");
    if (is_nospoof(target) && (target != sender) &&
        (target != mudstate.curr_enactor) &&
        (target != mudstate.curr_player && is_good_obj(sender))) {

      /*
       * I'd really like to use tprintf here but I can't
       * because the caller may have.
       * notify(target, tprintf(...)) is quite common
       * in the code.
       */

      tbuff = alloc_sbuf("notify_checked.nospoof");
      safe_chr('[', msg_ns, &mp);
      safe_str(Name(sender), msg_ns, &mp);
      snprintf(tbuff, SBUF_SIZE, "(#%ld)", sender);
      safe_str(tbuff, msg_ns, &mp);

      if (sender != obj_owner(sender)) {
        safe_chr('{', msg_ns, &mp);
        safe_str(Name(obj_owner(sender)), msg_ns, &mp);
        safe_chr('}', msg_ns, &mp);
      }
      if (sender != mudstate.curr_enactor) {
        snprintf(tbuff, SBUF_SIZE, "<-(#%ld)", mudstate.curr_enactor);
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

  check_listens = is_halted(target) ? 0 : 1;
  switch (typeof_obj(target)) {
  case TYPE_PLAYER:
    if (key & MSG_ME) {
      if (key & MSG_COLORIZE)
        colbuf = colorize(target, msg_ns);
      raw_notify(target, colbuf ? colbuf : msg_ns);
    }

    if (colbuf)
      free_lbuf(colbuf);
    if (!mudconf.player_listen)
      check_listens = 0;
    [[fallthrough]];
  case TYPE_THING:
  case TYPE_ROOM:

    /* If we're in a pipe, objects can receive raw_notify
     * if they're not a player and connected (if we didn't
     * do this, they'd be notified twice! */

    if (mudstate.inpipe &&
        (!is_player(target) || (is_player(target) && !is_connected(target)))) {
      raw_notify(target, msg_ns);
    }

    /*
     * Forward puppet message if it is for me
     */

    has_neighbors = has_location(target);
    targetloc = where_is(target);
    target_audible = is_audible(target);

    if ((key & MSG_ME) && is_puppet(target) && (target != obj_owner(target)) &&
        ((key & MSG_PUP_ALWAYS) ||
         ((targetloc != obj_location(obj_owner(target))) &&
          (targetloc != obj_owner(target))))) {
      tp = tbuff = alloc_lbuf("notify_checked.puppet");
      safe_str(Name(target), tbuff, &tp);
      safe_str("> ", tbuff, &tp);
      if (key & MSG_COLORIZE)
        colbuf = colorize(obj_owner(target), msg_ns);
      safe_str(colbuf ? colbuf : msg_ns, tbuff, &tp);
      *tp = '\0';
      raw_notify(obj_owner(target), tbuff);
      if (colbuf)
        free_lbuf(colbuf);
      free_lbuf(tbuff);
    }
    /*
     * Check for @Listen match if it will be useful
     */

    pass_listen = 0;
    nargs = 0;
    if (check_listens && (key & (MSG_ME | MSG_INV_L)) && has_listen(target)) {
      tp = attribute_get(target, A_LISTEN, &aowner, &aflags);
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
    if ((key & MSG_ME) && check_listens && (pass_listen || is_monitor(target)))
      pass_uselock = could_doit(sender, target, A_LUSE);

    /*
     * Process AxHEAR if we pass LISTEN, USElock and it's for me
     */

    if ((key & MSG_ME) && pass_listen && pass_uselock) {
      if (sender != target)
        did_it(sender, target, 0, nullptr, 0, nullptr, A_AHEAR, args, nargs);
      else
        did_it(sender, target, 0, nullptr, 0, nullptr, A_AMHEAR, args, nargs);
      did_it(sender, target, 0, nullptr, 0, nullptr, A_AAHEAR, args, nargs);
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
    if ((key & MSG_ME) && (sender != target || is_wizard(target)) &&
        pass_uselock && is_monitor(target)) {
      (void)attribute_match(target, sender, AMATCH_LISTEN, msg, 0);
    }
    /*
     * Deliver message to forwardlist members
     */

    if ((key & MSG_FWDLIST) && is_audible(target) &&
        check_filter(target, sender, A_FILTER, msg)) {
      tbuff = dflt_from_msg(sender, target);
      buff = add_prefix(target, sender, A_PREFIX, msg, tbuff);
      free_lbuf(tbuff);

      fp = fwdlist_get(target);
      if (fp) {
        for (i = 0; i < fp->count; i++) {
          recip = fp->data[i];
          if (!is_good_obj(recip) || (recip == target))
            continue;
          notify_checked(recip, sender, buff,
                         (MSG_ME | MSG_F_UP | MSG_F_CONTENTS | MSG_S_INSIDE));
        }
      }
      free_lbuf(buff);
    }
    /*
     * Deliver message through audible exits
     */

    if (key & MSG_INV_EXITS) {
      DOLIST(obj, obj_exits(target)) {
        recip = obj_location(obj);
        if (is_audible(obj) &&
            ((recip != target) && check_filter(obj, sender, A_FILTER, msg))) {
          buff = add_prefix(obj, target, A_PREFIX, msg, "From a distance,");
          notify_checked(recip, sender, buff,
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
        tbuff = dflt_from_msg(sender, target);
        buff = add_prefix(target, sender, A_PREFIX, msg, tbuff);
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix() allocation. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }

      DOLIST(obj, obj_exits(obj_location(target))) {
        recip = obj_location(obj);
        if (is_good_obj(recip) && is_audible(obj) && (recip != targetloc) &&
            (recip != target) && check_filter(obj, sender, A_FILTER, msg)) {
          tbuff = add_prefix(obj, target, A_PREFIX, buff, "From a distance,");
          notify_checked(recip, sender, tbuff,
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
        (check_filter(target, sender, A_INFILTER, msg))) {

      /*
       * Don't prefix the message if we were given the * *
       * * * MSG_NOPREFIX key.
       */

      if (key & MSG_S_OUTSIDE) {
        buff = add_prefix(target, sender, A_INPREFIX, msg, "");
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix() allocation. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      DOLIST(obj, obj_contents(target)) {
        if (obj != target) {
          notify_checked(obj, sender, buff,
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
        ((key & MSG_NBR) || ((key & MSG_NBR_A) && target_audible &&
                             check_filter(target, sender, A_FILTER, msg)))) {
      if (key & MSG_S_INSIDE) {
        tbuff = dflt_from_msg(sender, target);
        buff = add_prefix(target, sender, A_PREFIX, msg, "");
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix() allocation. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      DOLIST(obj, obj_contents(targetloc)) {
        if ((obj != target) && (obj != targetloc)) {
          notify_checked(obj, sender, buff,
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
        ((key & MSG_LOC) || ((key & MSG_LOC_A) && target_audible &&
                             check_filter(target, sender, A_FILTER, msg)))) {
      if (key & MSG_S_INSIDE) {
        tbuff = dflt_from_msg(sender, target);
        buff = add_prefix(target, sender, A_PREFIX, msg, tbuff);
        free_lbuf(tbuff);
      } else {
        /* buff aliases the read-only msg here; only freed in the
         * branch above where it holds an add_prefix() allocation. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
        buff = (char *)msg;
#pragma clang diagnostic pop
      }
      notify_checked(targetloc, sender, buff, MSG_ME | MSG_F_UP | MSG_S_INSIDE);
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
  mudstate.ntfy_nest_lev--;
}

void notify_except(DbRef loc, DbRef player, DbRef exception, const char *msg) {
  DbRef first;

  if (loc != exception)
    notify_checked(loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A));
  DOLIST(first, obj_contents(loc)) {
    if (first != exception)
      notify_checked(first, player, msg, (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
  }
}

void notify_except2(DbRef loc, DbRef player, DbRef exc1, DbRef exc2,
                    const char *msg) {
  DbRef first;

  if ((loc != exc1) && (loc != exc2))
    notify_checked(loc, player, msg,
                   (MSG_ME_ALL | MSG_F_UP | MSG_S_INSIDE | MSG_NBR_EXITS_A));
  DOLIST(first, obj_contents(loc)) {
    if (first != exc1 && first != exc2) {
      notify_checked(first, player, msg, (MSG_ME | MSG_F_DOWN | MSG_S_OUTSIDE));
    }
  }
}

void do_shutdown(DbRef player, DbRef cause, int key, char *message) {
  ResetSpecialObjects();
  if (player != NOTHING) {
    raw_broadcast(0, "Game: Shutdown by %s", Name(obj_owner(player)));
    STARTLOG(LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown by ");
      log_name(player);
      ENDLOG;
    }
  } else {
    raw_broadcast(0, "Game: Fatal Error: %s", message);
    STARTLOG(LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Fatal error: ");
      log_text(message);
      ENDLOG;
    }
  }
  if (player != NOTHING) {
    STARTLOG(LOG_ALWAYS, "WIZ", "SHTDN") {
      log_text("Shutdown status: ");
      log_text(message);
      ENDLOG;
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

    emergency_shutdown();

    /*
     * Close the attribute text db and dump the header db
     */

    pcache_sync();
    STARTLOG(LOG_ALWAYS, "DMP", "PANIC") {
      log_text("Panic dump: ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
    dump_database_internal(DUMP_CRASHED);

    STARTLOG(LOG_ALWAYS, "DMP", "DONE") {
      log_text("Panic dump complete: ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
  } else if (key & SHUTDN_KILLED) {
    pcache_sync();
    STARTLOG(LOG_ALWAYS, "DMP", "KILLED") {
      log_text("Killed dump: ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
    dump_database_internal(DUMP_KILLED);
    STARTLOG(LOG_ALWAYS, "DMP", "DONE") {
      log_text("Killed dump complete: ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
  }
  /*
   * Set up for normal shutdown
   */

  mudstate.shutdown_flag = 1;
  server_lifecycle_stop();
  return;
}

int dump_database_internal(int dump_type) { return gamedb_dump(dump_type); }

void dump_database(void) {
  mudstate.dumping = 1;
  STARTLOG(LOG_DBSAVES, "DMP", "DUMP") {
    log_text("Dumping: ");
    log_text(mudconf.database.gamedb);
    ENDLOG;
  }
  pcache_sync();

  dump_database_internal(DUMP_NORMAL);
  STARTLOG(LOG_DBSAVES, "DMP", "DONE") {
    log_text("Dump complete: ");
    log_text(mudconf.database.gamedb);
    ENDLOG;
  }
  mudstate.dumping = 0;
}

void fork_and_dump(int key) {
  if (*mudconf.dump_msg)
    raw_broadcast(0, "%s", mudconf.dump_msg);

  mudstate.dumping = 1;
  log_error(LOG_DBSAVES, "DMP", "CHKPT", "Saving database: %s",
            mudconf.database.gamedb);

  pcache_sync();

  if (!key || (key & DUMP_STRUCT)) {
    if (mudconf.fork_dump) {
      /* Fork and dump.  */
      switch (fork()) {
      case -1: /* fork() failed */
        /* FIXME: Make this error message conform.  */
        log_perror("DMP", "FAIL", nullptr, "fork()");
        mudstate.dumping = 0;
        return;

      case 0: /* child */
        dprintk("child database write process starting.");
        unbind_signals();
        dump_database_internal(DUMP_NORMAL);
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
      dump_database_internal(DUMP_NORMAL);
    }
  }

  mudstate.dumping = 0;

  if (*mudconf.postdump_msg)
    raw_broadcast(0, "%s", mudconf.postdump_msg);
}

static int load_game(void) {
  STARTLOG(LOG_STARTUP, "INI", "LOAD") {
    log_text("Loading: ");
    log_text(mudconf.database.gamedb);
    ENDLOG;
  };
  if (gamedb_load(mudconf.database.gamedb) < 0) {
    STARTLOG(LOG_ALWAYS, "INI", "FATAL") {
      log_text("Error loading ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
    return -1;
  }

  /* Load the mecha stuff.. */
  if (mudconf.have_specials)
    LoadSpecialObjects();

  STARTLOG(LOG_STARTUP, "INI", "LOAD") {
    log_text("Load complete.");
    ENDLOG;
  }
  /*
   * everything ok
   */
  return (0);
}

/**
 * match a list of things, using the no_command flag
 */
int list_check(DbRef thing, DbRef player, char type, char *str,
               int check_parent) {
  int match, limit;

  match = 0;
  limit = mudstate.db_top;
  while (thing != NOTHING) {
    if ((thing != player) && (!(is_no_command(thing)))) {
      if (attribute_match(thing, player, type, str, check_parent) > 0)
        match = 1;
    }
    thing = obj_next(thing);
    if (--limit < 0)
      return match;
  }
  return match;
}

int is_hearer(DbRef thing) {
  char *as, *buff, *s;
  DbRef aowner;
  int attr;
  long aflags;
  Attribute *ap;

  if (mudstate.inpipe && (thing == mudstate.poutobj))
    return 1;

  if (is_connected(thing) || is_puppet(thing))
    return 1;

  if (is_monitor(thing))
    buff = alloc_lbuf("Hearer");
  else
    buff = nullptr;
  for (attr = attribute_list_first(thing, &as); attr;
       attr = attribute_list_next(&as)) {
    if (attr == A_LISTEN) {
      if (buff)
        free_lbuf(buff);
      return 1;
    }
    if (buff && is_monitor(thing)) {
      ap = attribute_by_number(attr);
      if (!ap || (ap->flags & AF_NOPROG))
        continue;

      attribute_get_string(buff, thing, attr, &aowner, &aflags);

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

void do_readcache(DbRef player, DbRef cause, int key) { fcache_load(player); }

int main(int argc, char *argv[]) {
  char *config_file;
  int index;
  int mindb;

  if (argc > 3 || (argc > 2 && strcmp(argv[1], "-s")) ||
      (argc > 1 && !strcmp(argv[1], "--restart"))) {
    fprintf(stderr, "Usage: %s [-s] [config-file]\n", argv[0]);
    exit(1);
  }

  if (!server_lifecycle_initialize()) {
    fprintf(stderr, "Unable to create libevent event base.\n");
    exit(2);
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
  corrupt = 0; /* Database isn't corrupted. */
  memset(&mudstate, 0, sizeof(mudstate));
  time(&mudstate.start_time);
  time(&mudstate.process_start_time);
  mudstate.db_top = -1;
  tcache_init();
  pcache_init();
  configuration_initialize();
  init_rlimit();
  init_cmdtab();
  init_mactab();
  init_chantab();
  init_flagtab();
  init_powertab();
  init_functab();
  init_attrtab();
  init_version();

  hash_table_initialize(&mudstate.player_htab, 250 * HASH_FACTOR);
  numeric_hash_table_initialize(&mudstate.fwdlist_htab, 25 * HASH_FACTOR);
  numeric_hash_table_initialize(&mudstate.parent_htab, 5 * HASH_FACTOR);
  mudstate.desctree = red_black_tree_init(descriptor_compare, nullptr);
  vattr_init();

  configuration_read(config_file);

  if (!password_initialize()) {
    fprintf(stderr, "Unable to initialize password hashing.\n");
    exit(2);
  }

  if (!*mudconf.database.gamedb) {
    fprintf(stderr,
            "Required configuration directive game_database is missing.\n");
    exit(2);
  }

  if (btech_persistence_register() < 0) {
    fprintf(stderr, "Unable to register BTech SQLite persistence.\n");
    exit(2);
  }

  if (commac_persistence_register() < 0) {
    fprintf(stderr, "Unable to register commac SQLite persistence.\n");
    exit(2);
  }

  fcache_init();
  help_index_init();
  db_free();

  mudstate.record_players = 0;

  if (mindb)
    db_make_minimal();
  else if (load_game() < 0) {
    STARTLOG(LOG_ALWAYS, "INI", "LOAD") {
      log_text("Couldn't load: ");
      log_text(mudconf.database.gamedb);
      ENDLOG;
    }
    exit(2);
  }
  server_lifecycle_prepare();

  /*
   * Do a consistency check and set up the freelist
   */

  do_dbck(NOTHING, NOTHING, 0);

  /*
   * Reset all the hash stats
   */

  hash_table_reset(&mudstate.command_htab);
  hash_table_reset(&mudstate.macro_htab);
  hash_table_reset(&mudstate.channel_htab);
  hash_table_reset(&mudstate.func_htab);
  hash_table_reset(&mudstate.flags_htab);
  hash_table_reset(&mudstate.attr_name_htab);
  hash_table_reset(&mudstate.player_htab);
  numeric_hash_table_reset(&mudstate.fwdlist_htab);

  for (index = 0; index < MAX_GLOBAL_REGS; index++) {
    mudstate.global_regs[index] = alloc_lbuf("main.global_reg");
    memset(mudstate.global_regs[index], 0, LBUF_SIZE);
  }

  if (!server_lifecycle_boot(mindb)) {
    exit(2);
  }

#ifdef MCHECK
  mtrace();
#endif

  /*
   * go do it
   */

  server_lifecycle_run(mudconf.port);

#ifdef MCHECK
  muntrace();
#endif

  close_sockets(0, "Going down - Bye");
  dump_database();

  server_lifecycle_shutdown();
  exit(0);
}

static void init_rlimit(void) {
  struct rlimit *rlp;

  rlp = (struct rlimit *)alloc_lbuf("rlimit");

  if (getrlimit(RLIMIT_NOFILE, rlp)) {
    log_perror("RLM", "FAIL", nullptr, "getrlimit()");
    free_lbuf(rlp);
    return;
  }
  rlp->rlim_cur = rlp->rlim_max;
  if (setrlimit(RLIMIT_NOFILE, rlp))
    log_perror("RLM", "FAIL", nullptr, "setrlimit()");
  free_lbuf(rlp);
}
