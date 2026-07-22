/* log.h - Server logging and ANSI-stripping interface. */

#pragma once

#include <stddef.h>

#include "mux/objects/db.h"

typedef struct LogCache LogCache;
typedef struct ServerConfiguration ServerConfiguration;

typedef struct ServerLog ServerLog;
struct ServerLog {
  GameDatabase *database;
  const ServerConfiguration *configuration;
  LogCache *cache;
  int nesting;
  char timestamp[256];
};

void server_log_initialize(ServerLog *log, GameDatabase *database,
                           const ServerConfiguration *configuration);
bool server_log_is_enabled(const ServerLog *log, int key);

char *strip_ansi_r(char *destination, const char *source, size_t size);
int start_log(ServerLog *log, const char *primary, const char *secondary);
void end_log(ServerLog *log);
void log_perror(ServerLog *log, const char *primary, const char *secondary,
                const char *name, const char *error);
void log_error(ServerLog *log, int key, const char *primary,
               const char *secondary, const char *format, ...)
    __attribute__((format(printf, 5, 6)));
void log_text(const char *text);
void log_simple(ServerLog *log, int key, const char *primary,
                const char *secondary, const char *message);
void log_number(int number);
void log_name(ServerLog *log, DbRef thing);
void log_name_and_loc(ServerLog *log, DbRef thing);
const char *object_type_name(GameDatabase *database, DbRef thing);
void log_type_and_name(ServerLog *log, DbRef thing);
#ifdef ARBITRARY_LOGFILES
int log_to_file(EvaluationContext *evaluation, DbRef thing, const char *logfile,
                const char *message);
#endif
