/*
 * log.c - logging routines
 */

#include <assert.h>
#include <stdarg.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

#include "mux/commands/command_handlers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/log_cache.h"
#include "mux/server/mux_server.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/name_table.h"
#include "mux/support/styled_text/markup.h"

const NameTable LOGDATA_NAMETAB[] = {{"flags", 1, 0, LOGOPT_FLAGS},
                                     {"location", 1, 0, LOGOPT_LOC},
                                     {"timestamp", 1, 0, LOGOPT_TIMESTAMP},
                                     {nullptr, 0, 0, 0}};

const NameTable LOGOPTIONS_NAMETAB[] = {
    {"accounting", 2, 0, LOG_ACCOUNTING},
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

/**
 * See if it's is OK to log something, and if so, start writing the
 * log entry.
 */
int start_log(ServerLog *log, const char *primary, const char *secondary) {
  struct tm timestamp;
  time_t now;

  log->nesting++;
  switch (log->nesting) {
  case 1:
  case 2:

    /*
     * Format the timestamp
     */

    if ((log->configuration->log_info & LOGOPT_TIMESTAMP) != 0) {
      now = time(nullptr);
      if (now == (time_t)-1)
        now = 0;
      if (localtime_r(&now, &timestamp) == nullptr) {
        log->timestamp[0] = '\0';
      } else {
        (void)snprintf(log->timestamp, sizeof(log->timestamp),
                       "%d%02d%02d.%02d%02d%02d ", timestamp.tm_year + 1900,
                       timestamp.tm_mon + 1, timestamp.tm_mday,
                       timestamp.tm_hour, timestamp.tm_min, timestamp.tm_sec);
      }
    } else {
      log->timestamp[0] = '\0';
    }

    /*
     * Write the header to the log
     */

    if (secondary && *secondary)
      (void)fprintf(stderr, "%s%s %3s/%-5s: ", log->timestamp,
                    log->configuration->mud_name, primary, secondary);
    else
      (void)fprintf(stderr, "%s%s %-9s: ", log->timestamp,
                    log->configuration->mud_name, primary);
    /*
     * If a recursive call, log it and return indicating no log
     */

    if (log->nesting == 1)
      return 1;
    (void)fprintf(stderr, "Recursive logging request.\r\n");
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
  (void)fprintf(stderr, "\n");
  (void)fflush(stderr);
  log->nesting--;
}

/**
 * Write perror message to the log
 */
void log_perror(const LogSystemError *error) {
  start_log(error->log, error->primary, error->secondary);
  if (error->extra && *error->extra) {
    log_text("(");
    log_text(error->extra);
    log_text(") ");
  }
  perror(error->failing_object);
  (void)fflush(stderr);
  error->log->nesting--;
}

/**
 * Write text to log file.
 */
void log_text(const char *text) {
  char new[LBUF_SIZE];
  styled_text_strip(nullptr, text, new, sizeof(new));
  (void)fprintf(stderr, "%s", new);
}

void log_simple(LogEntry entry, const char *message) {
  if ((entry.key & entry.log->configuration->log_options) != 0 &&
      start_log(entry.log, entry.primary, entry.secondary)) {
    log_text(message);
    end_log(entry.log);
  }
}

void log_error(LogEntry entry, const char *format, ...) {
  char buffer[LBUF_SIZE];
  char stripped_buffer[LBUF_SIZE];
  va_list ap;
  ServerLog *log = entry.log;

  if (!(entry.key & log->configuration->log_options))
    return;

  if (log->configuration->log_info & LOGOPT_TIMESTAMP) {
    time_t now;
    struct tm tm;
    now = time(nullptr);
    if (now == (time_t)-1)
      now = 0;
    localtime_r(&now, &tm);
    (void)fprintf(stderr, "%d%02d%02d.%02d%02d%02d ", tm.tm_year + 1900,
                  tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
  }

  if (entry.secondary) {
    (void)fprintf(stderr, "%s%s %3s/%-5s: ", log->timestamp,
                  log->configuration->mud_name, entry.primary, entry.secondary);
  } else {
    (void)fprintf(stderr, "%s%s %-9s: ", log->timestamp,
                  log->configuration->mud_name, entry.primary);
  }

  va_start(ap, format);
  // NOLINTNEXTLINE(clang-analyzer-security.VAList)
  (void)vsnprintf(buffer, LBUF_SIZE, format, ap);
  va_end(ap);

  styled_text_strip(log->database->styled_text_palette, buffer, stripped_buffer,
                    sizeof(stripped_buffer));
  (void)fprintf(stderr, "%s\n", stripped_buffer);
}

/*
 * Write a number to log file.
 */
void log_number(int num) { (void)fprintf(stderr, "%d", num); }

/**
 * Writes the name, db number, and flags of an object to the log.
 */
void log_name(ServerLog *log, DbRef target) {
  char *tp;
  char new[LBUF_SIZE];

  if ((log->configuration->log_info & LOGOPT_FLAGS) != 0)
    tp = unparse_object(log->database, nullptr, (DbRef)GOD, target);
  else
    tp = unparse_object_numonly(log->database, target);
  styled_text_strip(log->database->styled_text_palette, tp, new, sizeof(new));
  (void)fprintf(stderr, "%s", new);
  free_lbuf(tp);
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
}

/*
 * Returns the object type of specified object.
 */
const char *object_type_name(GameDatabase *database, DbRef thing) {
  if (!is_good_obj(database, thing)) {
    return "??OUT-OF-RANGE??";
  }
  switch (typeof_obj(database, thing)) {
  case OBJECT_TYPE_PLAYER:
    return "PLAYER";
  case OBJECT_TYPE_THING:
    return "THING";
  case OBJECT_TYPE_ROOM:
    return "ROOM";
  case OBJECT_TYPE_EXIT:
    return "EXIT";
  case OBJECT_TYPE_GARBAGE:
    return "GARBAGE";
  default:
    return "??ILLEGAL??";
  }
}

void log_type_and_name(ServerLog *log, DbRef thing) {
  char nbuf[16];

  log_text(object_type_name(log->database, thing));
  (void)snprintf(nbuf, sizeof(nbuf), " #%ld(", thing);
  log_text(nbuf);
  if (is_good_obj(log->database, thing))
    log_text(game_object_name(log->database, thing));
  log_text(")");
}

bool log_to_file(const ArbitraryLogRequest *request) {
  char pathname[210]; /* Arbitrary limit in logfile length */
  char message_buffer[4096];

  if (!request->message || !*request->message)
    return true; /* Nothing to do */

  if (!request->filename || !*request->filename ||
      strlen(request->filename) > 200)
    return false; /* invalid logfile name */

  if (strstr(request->filename, "..") != nullptr)
    return false;
  if (strstr(request->filename, "/") != nullptr)
    return false;
  (void)snprintf(pathname, 210, "logs/%s", request->filename);

  /* Hacking checks. */

  if (access(pathname, R_OK | W_OK) != 0)
    return false;

  (void)snprintf(message_buffer, 4096, "%s\n", request->message);

  if (!log_cache_write(request->evaluation->log->cache, pathname,
                       message_buffer)) {
    notify_checked(request->evaluation, request->actor, request->actor,
                   "Serious failure while trying to write to log.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return false;
  }
  return true;
}

void do_log(CommandInvocation *invocation) {
  EvaluationContext *evaluation = &invocation->context->evaluation;
  DbRef player = invocation->player;
  char *logfile = invocation->first;
  char *message = invocation->second;
  if (!message || !*message) {
    notify_checked(evaluation, player, player, "Nothing to log!",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  if (!logfile || !*logfile) {
    notify_checked(evaluation, player, player, "Invalid logfile.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  if (!log_to_file(&(ArbitraryLogRequest){.evaluation = evaluation,
                                          .actor = player,
                                          .filename = logfile,
                                          .message = message})) {
    notify_checked(evaluation, player, player, "Request failed.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }

  notify_checked(evaluation, player, player, "Message logged.",
                 MSG_ME_ALL | MSG_F_DOWN);
}
