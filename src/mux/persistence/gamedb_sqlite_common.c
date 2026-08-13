/* gamedb_sqlite.c -- SQLite game-database persistence */

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <linux/limits.h>
#include <sqlite3.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include "mux/objects/db.h"
#include "mux/persistence/gamedb.h"
#include "mux/persistence/gamedb_sqlite_internal.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "mux/support/utf8.h"

void gamedb_log_failure(ServerLog *log, const char *stage, const char *path,
                        sqlite3 *sqlite) {
  const char *detail;

  detail = sqlite ? sqlite3_errmsg(sqlite) : strerror(errno);
  log_error(
      (LogEntry){
          .log = log, .key = LOG_ALWAYS, .primary = "GDB", .secondary = "FAIL"},
      "SQLite %s for %s: %s", stage, path, detail);
}

const NativeColumn *gamedb_native_column_at(size_t index) {
  return checked_storage_at_const(NATIVE_COLUMNS, NATIVE_COLUMN_COUNT,
                                  sizeof(*NATIVE_COLUMNS), index);
}

/* Report a subsystem persistence failure with its registered extension name. */
static void gamedb_log_extension_failure(ServerLog *log, const char *operation,
                                         const char *name, const char *path,
                                         sqlite3 *sqlite) {
  const char *detail;

  detail = sqlite ? sqlite3_errmsg(sqlite) : "extension callback failed";
  log_error(
      (LogEntry){
          .log = log, .key = LOG_ALWAYS, .primary = "GDB", .secondary = "FAIL"},
      "SQLite persistence extension %s failed while %s %s: %s", name, operation,
      path, detail);
}

/* Restore every registered subsystem while its snapshot connection is open. */
int gamedb_load_extensions(PersistenceContext *context, sqlite3 *sqlite,
                           const char *path) {
  size_t index;

  for (index = 0; index < context->extension_count; index++) {
    PersistenceSqliteExtension *extension =
        persistence_extension_at(context, index);
    if (extension->load == nullptr)
      continue;
    if (extension->load(sqlite, context, extension->context) < 0) {
      gamedb_log_extension_failure(context->log, "loading", extension->name,
                                   path, sqlite);
      return -1;
    }
  }
  return 0;
}

/* Store every registered subsystem before committing the full snapshot. */
int gamedb_store_extensions(PersistenceContext *context, sqlite3 *sqlite) {
  size_t index;

  for (index = 0; index < context->extension_count; index++) {
    PersistenceSqliteExtension *extension =
        persistence_extension_at(context, index);
    if (extension->store(sqlite, context, extension->context) < 0) {
      gamedb_log_extension_failure(context->log, "writing", extension->name,
                                   context->configuration->database.gamedb,
                                   sqlite);
      return -1;
    }
  }
  return 0;
}

/* Execute a statement that does not return rows. */
int gamedb_exec(sqlite3 *sqlite, const char *sql) {
  char *errmsg;
  int rc;

  errmsg = nullptr;
  rc = sqlite3_exec(sqlite, sql, nullptr, nullptr, &errmsg);
  if (errmsg)
    sqlite3_free(errmsg);
  return rc == SQLITE_OK ? 0 : -1;
}

/* Execute a reusable INSERT statement and reset it for the next row. */
int gamedb_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE)
    return -1;
  if (sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

/* Bind a MUX integer to SQLite's signed 64-bit integer representation. */
int gamedb_bind_int(sqlite3_stmt *statement, int index, long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) == SQLITE_OK
             ? 0
             : -1;
}

/* Select the configured SQLite file for a normal or exceptional dump. */
int gamedb_target_path(const GamedbTargetPathRequest *request) {
  const PersistenceContext *context = request->context;
  char *target = request->target;
  size_t target_size = request->target_size;
  int dump_type = request->dump_type;
  int length;

  switch (dump_type) {
  case DUMP_CRASHED:
    length = snprintf(target, target_size, "%s.CRASH",
                      context->configuration->database.gamedb);
    break;
  case DUMP_KILLED:
    length = snprintf(target, target_size, "%s.KILLED",
                      context->configuration->database.gamedb);
    break;
  default:
    length = snprintf(target, target_size, "%s",
                      context->configuration->database.gamedb);
    break;
  }
  return length < 0 || (size_t)length >= target_size ? -1 : 0;
}

/* Flush a completed temporary database before it is renamed into place. */
int gamedb_fsync_file(const char *path) {
  int fd;
  int result;

  fd = open(path, O_RDONLY);
  if (fd < 0)
    return -1;
  result = fsync(fd);
  close(fd);
  return result;
}

/* Flush the containing directory so the completed rename is durable. */
int gamedb_fsync_directory(const char *path) {
  char directory[PATH_MAX];
  char *slash;
  int fd;
  int result;

  if (strlen(path) >= sizeof(directory))
    return -1;
  string_copy(directory, path);
  slash = strrchr(directory, '/');
  if (!slash)
    string_copy(directory, ".");
  else if (slash == directory)
    *(char *)checked_storage_at(directory, sizeof(directory), sizeof(char), 1) =
        '\0';
  else
    *slash = '\0';

  fd = open(directory, O_RDONLY | O_DIRECTORY);
  if (fd < 0)
    return -1;
  result = fsync(fd);
  close(fd);
  return result;
}

/* Compile one SQL statement for repeated binding and execution. */
int gamedb_prepare(sqlite3 *sqlite, sqlite3_stmt **statement, const char *sql) {
  return sqlite3_prepare_v2(sqlite, sql, -1, statement, nullptr) == SQLITE_OK
             ? 0
             : -1;
}

/* Read an SQLite integer only when it fits the destination int exactly. */
int gamedb_column_int(sqlite3_stmt *statement, int column, int *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < INT_MIN || number > INT_MAX)
    return -1;
  *value = (int)number;
  return 0;
}

int gamedb_column_bool(sqlite3_stmt *statement, int column, bool *value) {
  int number;

  if (gamedb_column_int(statement, column, &number) < 0 ||
      (number != 0 && number != 1))
    return -1;
  *value = number != 0;
  return 0;
}

/* Read an SQLite integer only when it fits the destination long exactly. */
int gamedb_column_long(sqlite3_stmt *statement, int column, long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < LONG_MIN || number > LONG_MAX)
    return -1;
  *value = (long)number;
  return 0;
}

/* Read a NUL-free SQLite text value that fits the target MUX buffer. */
int gamedb_column_text(sqlite3_stmt *statement, int column, const char **value,
                       int maximum_size) {
  const unsigned char *text;
  int length;

  if (sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  text = sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  if (!text || length < 0 || length >= maximum_size ||
      (int)strlen((const char *)text) != length ||
      !utf8_validate((const char *)text, (size_t)length))
    return -1;
  *value = (const char *)text;
  return 0;
}

/* Validate singleton snapshot metadata and restore global allocation state. */
