/** @file
 * Server logging and ANSI-stripping interface.
 */
#pragma once

#include "mux/server/platform.h"
#include "mux/support/name_table.h"

struct ServerLog; // IWYU pragma: keep

// IWYU pragma: no_include "mux/commands/command_context.h"
// IWYU pragma: no_include "mux/communication/commac.h"
// IWYU pragma: no_include "mux/server/server_config.h"

typedef struct LogCache LogCache;
typedef struct EvaluationContext EvaluationContext;
typedef struct GameDatabase GameDatabase;
typedef struct ServerConfiguration ServerConfiguration;
extern const NameTable LOGDATA_NAMETAB[];
extern const NameTable LOGOPTIONS_NAMETAB[];

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

typedef struct ArbitraryLogRequest {
  EvaluationContext *evaluation;
  DbRef actor;
  const char *filename;
  const char *message;
} ArbitraryLogRequest;

/** Initializes server log. @param[out] log Server log. @param[in] database Game
 * database. @param[in] configuration Server configuration. */

void server_log_initialize(ServerLog *log, GameDatabase *database,
                           const ServerConfiguration *configuration);
/** Executes server log is enabled. @param[in] log Server log. @param[in] key
 * Lookup key or command flags. */

bool server_log_is_enabled(const ServerLog *log, int key);

#define STARTLOG(log, key, primary, secondary)                                 \
  if (server_log_is_enabled(log, key) && start_log(log, primary, secondary))
#define ENDLOG(log) end_log(log)

/** Executes start log. @param[in,out] log Server log. @param[in] primary
 * Primary. @param[in] secondary Secondary. */

bool start_log(ServerLog *log, const char *primary, const char *secondary);
/** Executes end log. @param[in,out] log Server log. */

void end_log(ServerLog *log);
/** Executes log perror. @param[in] error Storage receiving an error
 * description. */

void log_perror(const LogSystemError *error);
/** Executes log error. @param[in] entry Entry. @param[in] format Format. */

void log_error(LogEntry entry, const char *format, ...)
    __attribute__((format(printf, 2, 3)));
/** Executes log text. @param[in] text Text to process. */

void log_text(const char *text);
/** Executes log simple. @param[in] entry Entry. @param[in] message Message. */

void log_simple(LogEntry entry, const char *message);
/** Executes log number. @param[in] number Number. */

void log_number(int number);
/** Executes log name. @param[in] log Server log. @param[in] target Target
 * object or value. */

void log_name(ServerLog *log, DbRef target);
/** Executes log name and loc. @param[in,out] log Server log. @param[in] player
 * Player object. */

void log_name_and_loc(ServerLog *log, DbRef player);
/** Executes object type name. @param[in] database Game database. @param[in]
 * thing Thing. */

const char *object_type_name(GameDatabase *database, DbRef thing);
/** Executes log type and name. @param[in] log Server log. @param[in] thing
 * Thing. */

void log_type_and_name(ServerLog *log, DbRef thing);
/** Executes log to file. @param[in] request Request. */

bool log_to_file(const ArbitraryLogRequest *request);
