/* Private interfaces shared by SQLite game-database persistence modules. */

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "mux/persistence/gamedb.h"

extern const int GAMEDB_SCHEMA_VERSION;
extern const int GAMEDB_SOURCE_FORMAT_SQLITE;
extern const char SCHEMA_OBJECTS_SQL[];
extern const char SCHEMA_STATE_SQL[];
void gamedb_log_failure(ServerLog *log, const char *stage, const char *path,
                        sqlite3 *sqlite);
int gamedb_load_extensions(PersistenceContext *context, sqlite3 *sqlite,
                           const char *path);
int gamedb_store_extensions(PersistenceContext *context, sqlite3 *sqlite);
int gamedb_exec(sqlite3 *sqlite, const char *sql);
int gamedb_step(sqlite3_stmt *statement);
int gamedb_bind_int(sqlite3_stmt *statement, int index, long value);
typedef struct GamedbTargetPathRequest {
  const PersistenceContext *context;
  char *target;
  size_t target_size;
  int dump_type;
} GamedbTargetPathRequest;

int gamedb_target_path(const GamedbTargetPathRequest *request);
int gamedb_fsync_file(const char *path);
int gamedb_fsync_directory(const char *path);
int gamedb_prepare(sqlite3 *sqlite, sqlite3_stmt **statement, const char *sql);
int gamedb_column_int(sqlite3_stmt *statement, int column, int *value);
int gamedb_column_bool(sqlite3_stmt *statement, int column, bool *value);
int gamedb_column_long(sqlite3_stmt *statement, int column, long *value);
int gamedb_column_text(sqlite3_stmt *statement, int column, const char **value,
                       int maximum_size);
