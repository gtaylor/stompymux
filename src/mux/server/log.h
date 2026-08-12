/* log.h - Server logging and ANSI-stripping interface. */

#pragma once

#include "btmux_build_config.h"
#include "mux/objects/db.h" // IWYU pragma: keep
#include "mux/server/platform.h"

struct ServerLog; // IWYU pragma: keep

// IWYU pragma: no_include "mux/commands/command_context.h"
// IWYU pragma: no_include "mux/communication/commac.h"
// IWYU pragma: no_include "mux/server/server_config.h"

typedef struct LogCache LogCache;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;

typedef struct ServerLog ServerLog;
struct ServerLog {
  GameDatabase *database;
  const ServerConfiguration *configuration;
  LogCache *cache;
  int nesting;
  char timestamp[256];
};

typedef struct LogEntry {
  ServerLog *log;
  int key;
  const char *primary;
  const char *secondary;
} LogEntry;

typedef struct LogSystemError {
  ServerLog *log;
  const char *primary;
  const char *secondary;
  const char *extra;
  const char *failing_object;
} LogSystemError;

#ifdef ARBITRARY_LOGFILES
typedef struct ArbitraryLogRequest {
  EvaluationContext *evaluation;
  DbRef actor;
  const char *filename;
  const char *message;
} ArbitraryLogRequest;
#endif

void server_log_initialize(ServerLog *log, GameDatabase *database,
                           const ServerConfiguration *configuration);
bool server_log_is_enabled(const ServerLog *log, int key);

#define STARTLOG(log, key, primary, secondary)                                 \
  if (server_log_is_enabled(log, key) && start_log(log, primary, secondary))
#define ENDLOG(log) end_log(log)

int start_log(ServerLog *log, const char *primary, const char *secondary);
void end_log(ServerLog *log);
void log_perror(const LogSystemError *error);
void log_error(LogEntry entry, const char *format, ...)
    __attribute__((format(printf, 2, 3)));
void log_text(const char *text);
void log_simple(LogEntry entry, const char *message);
void log_number(int number);
void log_name(ServerLog *log, DbRef target);
void log_name_and_loc(ServerLog *log, DbRef player);
const char *object_type_name(GameDatabase *database, DbRef thing);
void log_type_and_name(ServerLog *log, DbRef thing);
#ifdef ARBITRARY_LOGFILES
bool log_to_file(const ArbitraryLogRequest *request);
#endif
