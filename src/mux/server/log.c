/*
 * log.c - logging routines
 */

#include "mux/server/platform.h"

#include <assert.h>
#include <sys/time.h>
#include <sys/types.h>
#include <time.h>

#include "mux/commands/command.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
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

void server_log_initialize(ServerLog *log, GameDatabase *database,
                           const ServerConfiguration *configuration) {
  assert(log != nullptr);
  memset(log, 0, sizeof(*log));
  log->database = database;
  log->configuration = configuration;
}

bool server_log_is_enabled(const ServerLog *log, int key) {
  return (key & log->configuration->log_options) != 0;
}

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
int start_log(ServerLog *log, const char *primary, const char *secondary) {
  struct tm *tp;
  time_t now;

  log->nesting++;
  switch (log->nesting) {
  case 1:
  case 2:

    /*
     * Format the timestamp
     */

    if ((log->configuration->log_info & LOGOPT_TIMESTAMP) != 0) {
      time((time_t *)(&now));
      tp = localtime((time_t *)(&now));
      snprintf(log->timestamp, sizeof(log->timestamp),
               "%d%02d%02d.%02d%02d%02d ", tp->tm_year + 1900, tp->tm_mon + 1,
               tp->tm_mday, tp->tm_hour, tp->tm_min, tp->tm_sec);
    } else {
      log->timestamp[0] = '\0';
    }

    /*
     * Write the header to the log
     */

    if (secondary && *secondary)
      fprintf(stderr, "%s%s %3s/%-5s: ", log->timestamp,
              log->configuration->mud_name, primary, secondary);
    else
      fprintf(stderr, "%s%s %-9s: ", log->timestamp,
              log->configuration->mud_name, primary);
    /*
     * If a recursive call, log it and return indicating no log
     */

    if (log->nesting == 1)
      return 1;
    fprintf(stderr, "Recursive logging request.\r\n");
    [[fallthrough]];
  default:
    log->nesting--;
  }
  return 0;
}

/**
 * Finish up writing a log entry
 */
void end_log(ServerLog *log) {
  fprintf(stderr, "\n");
  fflush(stderr);
  log->nesting--;
}

/**
 * Write perror message to the log
 */
void log_perror(ServerLog *log, const char *primary, const char *secondary,
                const char *extra, const char *failing_object) {
  start_log(log, primary, secondary);
  if (extra && *extra) {
    log_text("(");
    log_text(extra);
    log_text(") ");
  }
  perror(failing_object);
  fflush(stderr);
  log->nesting--;
}

/**
 * Write text to log file.
 */
void log_text(const char *text) {
  char new[LBUF_SIZE];
  strncpy(new, text, LBUF_SIZE - 1);
  fprintf(stderr, "%s", strip_ansi_r(new, text, strlen(text)));
}

void log_simple(ServerLog *log, int key, const char *primary,
                const char *secondary, const char *message) {
  if ((key & log->configuration->log_options) != 0 &&
      start_log(log, primary, secondary)) {
    log_text(message);
    end_log(log);
  }
}

void log_error(ServerLog *log, int key, const char *primary,
               const char *secondary, const char *format, ...) {
  char buffer[LBUF_SIZE];
  char stripped_buffer[LBUF_SIZE];
  va_list ap;

  if (!(key & log->configuration->log_options))
    return;

  if (log->configuration->log_info & LOGOPT_TIMESTAMP) {
    time_t now;
    struct tm tm;
    time(&now);
    localtime_r(&now, &tm);
    fprintf(stderr, "%d%02d%02d.%02d%02d%02d ", tm.tm_year + 1900,
            tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
  }

  if (secondary) {
    fprintf(stderr, "%s%s %3s/%-5s: ", log->timestamp,
            log->configuration->mud_name, primary, secondary);
  } else {
    fprintf(stderr, "%s%s %-9s: ", log->timestamp, log->configuration->mud_name,
            primary);
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
void log_name(ServerLog *log, DbRef target) {
  char *tp;
  char new[LBUF_SIZE];

  if ((log->configuration->log_info & LOGOPT_FLAGS) != 0)
    tp = unparse_object(log->database, nullptr, (DbRef)GOD, target, 0);
  else
    tp = unparse_object_numonly(log->database, target);
  strncpy(new, tp, LBUF_SIZE - 1);
  fprintf(stderr, "%s", strip_ansi_r(new, tp, strlen(tp)));
  free_lbuf(tp);
  if (((log->configuration->log_info & LOGOPT_OWNER) != 0) &&
      (target != game_object_owner(log->database, target))) {
    if ((log->configuration->log_info & LOGOPT_FLAGS) != 0)
      tp = unparse_object(log->database, nullptr, (DbRef)GOD,
                          game_object_owner(log->database, target), 0);
    else
      tp = unparse_object_numonly(log->database,
                                  game_object_owner(log->database, target));
    strncpy(new, tp, LBUF_SIZE - 1);
    fprintf(stderr, "[%s]", strip_ansi_r(new, tp, strlen(tp)));
    free_lbuf(tp);
  }
  return;
}

/**
 * Log both the name and location of an object
 */
void log_name_and_loc(ServerLog *log, DbRef player) {
  log_name(log, player);
  if ((log->configuration->log_info & LOGOPT_LOC) &&
      has_location(log->database, player)) {
    log_text(" in ");
    log_name(log, game_object_location(log->database, player));
  }
  return;
}

/*
 * Returns the object type of specified object.
 */
const char *object_type_name(GameDatabase *database, DbRef thing) {
  if (!is_good_obj(database, thing)) {
    return "??OUT-OF-RANGE??";
  }
  switch (typeof_obj(database, thing)) {
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

void log_type_and_name(ServerLog *log, DbRef thing) {
  char nbuf[16];

  log_text(object_type_name(log->database, thing));
  snprintf(nbuf, sizeof(nbuf), " #%ld(", thing);
  log_text(nbuf);
  if (is_good_obj(log->database, thing))
    log_text(game_object_name(log->database, thing));
  log_text(")");
  return;
}

#ifdef ARBITRARY_LOGFILES
int log_to_file(EvaluationContext *evaluation, DbRef thing, const char *logfile,
                const char *message) {
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

  if (!log_cache_write(evaluation->server->log.cache, pathname,
                       message_buffer)) {
    notify(evaluation, thing, "Serious failure while trying to write to log.");
    return 0;
  }
  return 1;
}

void do_log(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *logfile = invocation->first;
  char *message = invocation->second;
  if (!message || !*message) {
    notify(evaluation, player, "Nothing to log!");
    return;
  }

  if (!logfile || !*logfile) {
    notify(evaluation, player, "Invalid logfile.");
    return;
  }

  if (!log_to_file(evaluation, player, logfile, message)) {
    notify(evaluation, player, "Request failed.");
    return;
  }

  notify(evaluation, player, "Message logged.");
}
#endif
