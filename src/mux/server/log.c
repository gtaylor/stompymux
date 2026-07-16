/*
 * log.c - logging routines
 */

#include "mux/server/platform.h"

#include <sys/time.h>
#include <sys/types.h>
#include <time.h>

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/server/log.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/ansi.h"
#ifdef ARBITRARY_LOGFILES
#include "mux/server/log_cache.h"
#endif

NameTable logdata_nametab[] = {{"flags", 1, 0, LOGOPT_FLAGS},
                               {"location", 1, 0, LOGOPT_LOC},
                               {"owner", 1, 0, LOGOPT_OWNER},
                               {"timestamp", 1, 0, LOGOPT_TIMESTAMP},
                               {nullptr, 0, 0, 0}};

NameTable logoptions_nametab[] = {{"accounting", 2, 0, LOG_ACCOUNTING},
                                  {"all_commands", 2, 0, LOG_ALLCOMMANDS},
                                  {"suspect_commands", 2, 0, LOG_SUSPECTCMDS},
                                  {"bad_commands", 2, 0, LOG_BADCOMMANDS},
                                  {"buffer_alloc", 3, 0, LOG_ALLOCATE},
                                  {"bugs", 3, 0, LOG_BUGS},
                                  {"checkpoints", 2, 0, LOG_DBSAVES},
                                  {"config_changes", 2, 0, LOG_CONFIGMODS},
                                  {"create", 2, 0, LOG_PCREATES},
                                  {"logins", 1, 0, LOG_LOGIN},
                                  {"network", 1, 0, LOG_NET},
                                  {"problems", 1, 0, LOG_PROBLEMS},
                                  {"security", 2, 0, LOG_SECURITY},
                                  {"shouts", 2, 0, LOG_SHOUTS},
                                  {"startup", 2, 0, LOG_STARTUP},
                                  {"wizard", 1, 0, LOG_WIZARD},
                                  {nullptr, 0, 0, 0}};

char *strip_ansi_r(char *dest, const char *raw, size_t n) {
  const char *p = raw;
  char *q = dest;

  while (p && *p && ((size_t)(q - dest) < n)) {
    if (*p == ESC_CHAR) {
      /*
       * Start of ANSI code. Skip to end.
       */
      while (*p && !isalpha(*p))
        p++;
      if (*p)
        p++;
    } else
      *q++ = *p++;
  }
  *q = '\0';
  return dest;
}

/**
 * See if it's is OK to log something, and if so, start writing the
 * log entry.
 */
int start_log(const char *primary, const char *secondary) {
  struct tm *tp;
  time_t now;

  mudstate.logging++;
  switch (mudstate.logging) {
  case 1:
  case 2:

    /*
     * Format the timestamp
     */

    if ((mudconf.log_info & LOGOPT_TIMESTAMP) != 0) {
      time((time_t *)(&now));
      tp = localtime((time_t *)(&now));
      snprintf(mudstate.buffer, 256 /* TODO */, "%d%02d%02d.%02d%02d%02d ",
               tp->tm_year + 1900, tp->tm_mon + 1, tp->tm_mday, tp->tm_hour,
               tp->tm_min, tp->tm_sec);
    } else {
      mudstate.buffer[0] = '\0';
    }

    /*
     * Write the header to the log
     */

    if (secondary && *secondary)
      fprintf(stderr, "%s%s %3s/%-5s: ", mudstate.buffer, mudconf.mud_name,
              primary, secondary);
    else
      fprintf(stderr, "%s%s %-9s: ", mudstate.buffer, mudconf.mud_name,
              primary);
    /*
     * If a recursive call, log it and return indicating no log
     */

    if (mudstate.logging == 1)
      return 1;
    fprintf(stderr, "Recursive logging request.\r\n");
    [[fallthrough]];
  default:
    mudstate.logging--;
  }
  return 0;
}

/**
 * Finish up writing a log entry
 */
void end_log(void) {
  fprintf(stderr, "\n");
  fflush(stderr);
  mudstate.logging--;
}

/**
 * Write perror message to the log
 */
void log_perror(const char *primary, const char *secondary, const char *extra,
                const char *failing_object) {
  start_log(primary, secondary);
  if (extra && *extra) {
    log_text("(");
    log_text(extra);
    log_text(") ");
  }
  perror(failing_object);
  fflush(stderr);
  mudstate.logging--;
}

/**
 * Write text to log file.
 */
void log_text(const char *text) {
  char new[LBUF_SIZE];
  strncpy(new, text, LBUF_SIZE - 1);
  fprintf(stderr, "%s", strip_ansi_r(new, text, strlen(text)));
}

void log_simple(int key, const char *primary, const char *secondary,
                const char *message) {
  if ((key & mudconf.log_options) != 0 && start_log(primary, secondary)) {
    log_text(message);
    end_log();
  }
}

void log_error(int key, const char *primary, const char *secondary,
               const char *format, ...) {
  char buffer[LBUF_SIZE];
  char stripped_buffer[LBUF_SIZE];
  va_list ap;

  if (!(key & mudconf.log_options))
    return;

  if (mudconf.log_info & LOGOPT_TIMESTAMP) {
    time_t now;
    struct tm tm;
    time(&now);
    localtime_r(&now, &tm);
    fprintf(stderr, "%d%02d%02d.%02d%02d%02d ", tm.tm_year + 1900,
            tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
  }

  if (secondary) {
    fprintf(stderr, "%s%s %3s/%-5s: ", mudstate.buffer, mudconf.mud_name,
            primary, secondary);
  } else {
    fprintf(stderr, "%s%s %-9s: ", mudstate.buffer, mudconf.mud_name, primary);
  }

  va_start(ap, format);
  vsnprintf(buffer, LBUF_SIZE, format, ap);
  va_end(ap);

  strip_ansi_r(stripped_buffer, buffer, LBUF_SIZE);
  fprintf(stderr, "%s\n", stripped_buffer);
}

/*
 * Write a number to log file.
 */
void log_number(int num) { fprintf(stderr, "%d", num); }

/**
 * Writes the name, db number, and flags of an object to the log.
 * If the object does not own itself, append the name, db number, and flags
 * of the owner.
 */
void log_name(DbRef target) {
  char *tp;
  char new[LBUF_SIZE];

  if ((mudconf.log_info & LOGOPT_FLAGS) != 0)
    tp = unparse_object((DbRef)GOD, target, 0);
  else
    tp = unparse_object_numonly(target);
  strncpy(new, tp, LBUF_SIZE - 1);
  fprintf(stderr, "%s", strip_ansi_r(new, tp, strlen(tp)));
  free_lbuf(tp);
  if (((mudconf.log_info & LOGOPT_OWNER) != 0) &&
      (target != obj_owner(target))) {
    if ((mudconf.log_info & LOGOPT_FLAGS) != 0)
      tp = unparse_object((DbRef)GOD, obj_owner(target), 0);
    else
      tp = unparse_object_numonly(obj_owner(target));
    strncpy(new, tp, LBUF_SIZE - 1);
    fprintf(stderr, "[%s]", strip_ansi_r(new, tp, strlen(tp)));
    free_lbuf(tp);
  }
  return;
}

/**
 * Log both the name and location of an object
 */
void log_name_and_loc(DbRef player) {
  log_name(player);
  if ((mudconf.log_info & LOGOPT_LOC) && has_location(player)) {
    log_text(" in ");
    log_name(obj_location(player));
  }
  return;
}

/*
 * Returns the object type of specified object.
 */
const char *OBJTYP(DbRef thing) {
  if (!is_good_obj(thing)) {
    return "??OUT-OF-RANGE??";
  }
  switch (typeof_obj(thing)) {
  case TYPE_PLAYER:
    return "PLAYER";
  case TYPE_THING:
    return "THING";
  case TYPE_ROOM:
    return "ROOM";
  case TYPE_EXIT:
    return "EXIT";
  case TYPE_GARBAGE:
    return "GARBAGE";
  default:
    return "??ILLEGAL??";
  }
}

void log_type_and_name(DbRef thing) {
  char nbuf[16];

  log_text(OBJTYP(thing));
  snprintf(nbuf, sizeof(nbuf), " #%ld(", thing);
  log_text(nbuf);
  if (is_good_obj(thing))
    log_text(Name(thing));
  log_text(")");
  return;
}

#ifdef ARBITRARY_LOGFILES
int log_to_file(DbRef thing, const char *logfile, const char *message) {
  char pathname[210]; /* Arbitrary limit in logfile length */
  char message_buffer[4096];

  if (!message || !*message)
    return 1; /* Nothing to do */

  if (!logfile || !*logfile || strlen(logfile) > 200)
    return 0; /* invalid logfile name */

  if (strstr(logfile, "..") != nullptr)
    return 0;
  if (strstr(logfile, "/") != nullptr)
    return 0;
  snprintf(pathname, 210, "logs/%s", logfile);

  /* Hacking checks. */

  if (access(pathname, R_OK | W_OK) != 0)
    return 0;

  snprintf(message_buffer, 4096, "%s\n", message);

  if (!logcache_writelog(pathname, message_buffer)) {
    notify(thing, "Serious failure while trying to write to log.");
    return 0;
  }
  return 1;
}

void do_log(DbRef player, DbRef cause, int key, char *logfile, char *message) {
  if (!message || !*message) {
    notify(player, "Nothing to log!");
    return;
  }

  if (!logfile || !*logfile) {
    notify(player, "Invalid logfile.");
    return;
  }

  if (!log_to_file(player, logfile, message)) {
    notify(player, "Request failed.");
    return;
  }

  notify(player, "Message logged.");
}
#endif
