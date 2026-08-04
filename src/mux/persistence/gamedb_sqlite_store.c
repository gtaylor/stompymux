/* gamedb_sqlite.c -- SQLite game-database persistence */

#include <limits.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h"
#include "mux/persistence/gamedb_sqlite_internal.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"

static int gamedb_finish_snapshot(sqlite3 *sqlite, sqlite3_stmt *snapshot,
                                  sqlite3_stmt *objects,
                                  sqlite3_stmt *object_state, int success) {
  if (!success)
    gamedb_exec(sqlite, "ROLLBACK;");
  sqlite3_finalize(snapshot);
  sqlite3_finalize(objects);
  sqlite3_finalize(object_state);
  return success ? 0 : -1;
}

static int gamedb_store_native_state(GameDatabase *database, sqlite3 *sqlite,
                                     DbRef object) {
  sqlite3_stmt *statement = nullptr;
  char query[256];

  const char *tables[] = {"player_state", "btech_object_state"};
  for (size_t index = 0; index < sizeof(tables) / sizeof(*tables); index++) {
    snprintf(query, sizeof(query), "INSERT INTO %s (object_dbref) VALUES (?);",
             tables[index]);
    if (gamedb_prepare(sqlite, &statement, query) < 0 ||
        gamedb_bind_int(statement, 1, object) < 0 ||
        gamedb_step(statement) < 0) {
      sqlite3_finalize(statement);
      return -1;
    }
    sqlite3_finalize(statement);
    statement = nullptr;
  }
  for (size_t index = 0; index < native_column_count; index++) {
    const NativeColumn *column = &native_columns[index];
    const char *value = attribute_get_raw(database, object, column->field);

    if (!value)
      continue;
    snprintf(query, sizeof(query), "UPDATE %s SET %s = ? WHERE %s = ?;",
             column->table, column->column, column->key_column);
    if (gamedb_prepare(sqlite, &statement, query) < 0 ||
        sqlite3_bind_text(statement, 1, value, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_bind_int(statement, 2, object) < 0 ||
        gamedb_step(statement) < 0) {
      sqlite3_finalize(statement);
      return -1;
    }
    sqlite3_finalize(statement);
    statement = nullptr;
  }
  return 0;
}

/*
 * Populate a newly created SQLite database from the live in-memory game
 * state. The transaction is committed only after every table is complete.
 */
static int gamedb_store_snapshot(PersistenceContext *context, sqlite3 *sqlite,
                                 int dump_type) {
  sqlite3_stmt *snapshot;
  sqlite3_stmt *objects;
  sqlite3_stmt *object_state;
  DbRef object;

  snapshot = nullptr;
  objects = nullptr;
  object_state = nullptr;

  if (gamedb_exec(sqlite,
                  "PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; "
                  "PRAGMA foreign_keys = ON;") < 0 ||
      gamedb_exec(sqlite, "BEGIN IMMEDIATE;") < 0 ||
      gamedb_exec(sqlite, schema_objects_sql) < 0 ||
      gamedb_exec(sqlite, schema_state_sql) < 0 ||
      gamedb_exec(sqlite, "PRAGMA application_id = "
                          "1112821080; PRAGMA user_version = 1;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

  if (gamedb_prepare(
          sqlite, &snapshot,
          "INSERT INTO snapshot "
          "(id, schema_version, storage_format, storage_version, dump_type, "
          "dump_time, db_top, min_size, record_players) "
          "VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_prepare(
          sqlite, &objects,
          "INSERT INTO objects "
          "(dbref, name, location, zone, contents, exits, link, next, type, "
          "lua_parent, "
          "has_ansi_flag, has_audible_flag, "
          "has_auditorium_flag, has_blind_flag, has_connected_flag, "
          "has_dark_flag, has_floating_flag, has_gagged_flag, has_going_flag, "
          "has_halted_flag, has_in_character_flag, has_light_flag, "
          "has_monitor_flag, has_no_command_flag, "
          "has_quiet_flag, has_safe_flag, has_suspect_flag, "
          "has_transparent_flag, has_wizard_flag, has_xcode_flag, "
          "has_zombie_flag, "
          "has_idle_power) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_prepare(sqlite, &object_state,
                     "INSERT INTO object_state "
                     "(object_dbref, namespace, key, value_type, value) "
                     "VALUES (?, ?, ?, ?, ?);") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

  if (gamedb_bind_int(snapshot, 1, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 2, GAMEDB_SOURCE_FORMAT_SQLITE) < 0 ||
      gamedb_bind_int(snapshot, 3, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 4, dump_type) < 0 ||
      gamedb_bind_int(snapshot, 5, *context->now) < 0 ||
      gamedb_bind_int(snapshot, 6, context->database->top) < 0 ||
      gamedb_bind_int(snapshot, 7, context->database->minimum_size) < 0 ||
      gamedb_bind_int(snapshot, 8, *context->record_players) < 0 ||
      gamedb_step(snapshot) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

  DO_WHOLE_DB(context->database, object) {
    if (is_going(context->database, object))
      continue;
    if (gamedb_bind_int(objects, 1, object) < 0 ||
        sqlite3_bind_text(objects, 2,
                          game_object_name(context->database, object), -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        gamedb_bind_int(objects, 3,
                        game_object_location(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 4,
                        game_object_zone(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 5,
                        game_object_contents(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 6,
                        game_object_exits(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 7,
                        game_object_link(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 8,
                        game_object_next(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 9, typeof_obj(context->database, object)) <
            0 ||
        sqlite3_bind_text(objects, 10,
                          game_object_lua_parent(context->database, object), -1,
                          SQLITE_TRANSIENT) != SQLITE_OK) {
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
    }
    for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++) {
      if (gamedb_bind_int(
              objects, 10 + (int)flag,
              game_object_has_flag(context->database, object, flag)) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
    }
    for (PowerId power = POWER_IDLE; power < POWER_COUNT; power++) {
      if (gamedb_bind_int(
              objects, 31 + (int)power,
              game_object_has_power(context->database, object, power)) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
    }
    if (gamedb_step(objects) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

    if (gamedb_store_native_state(context->database, sqlite, object) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
    for (size_t index = 0;
         index < object_state_count(context->database, object); index++) {
      ObjectStateEntryView entry;
      int bind_result = SQLITE_ERROR;

      if (!object_state_entry(context->database, object, index, &entry) ||
          gamedb_bind_int(object_state, 1, object) < 0 ||
          sqlite3_bind_text(object_state, 2, entry.name_space, -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          sqlite3_bind_text(object_state, 3, entry.key, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          gamedb_bind_int(object_state, 4, entry.value->type) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
      switch (entry.value->type) {
      case OBJECT_STATE_STRING:
        bind_result = sqlite3_bind_blob(
            object_state, 5, entry.value->as.string.data,
            (int)entry.value->as.string.length, SQLITE_TRANSIENT);
        break;
      case OBJECT_STATE_BOOLEAN:
        bind_result =
            sqlite3_bind_int(object_state, 5, entry.value->as.boolean);
        break;
      case OBJECT_STATE_INTEGER:
        bind_result = sqlite3_bind_int64(
            object_state, 5, (sqlite3_int64)entry.value->as.integer);
        break;
      case OBJECT_STATE_NUMBER:
        bind_result =
            sqlite3_bind_double(object_state, 5, entry.value->as.number);
        break;
      }
      if (bind_result != SQLITE_OK || gamedb_step(object_state) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
    }
  }

  if (gamedb_store_extensions(context, sqlite) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

  if (gamedb_exec(sqlite, "COMMIT;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
  return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 1);
}

/*
 * Build a complete temporary snapshot and atomically replace the configured
 * target file. The previous file remains untouched until the replacement is
 * fully written, closed, and synced.
 */
int gamedb_dump(PersistenceContext *context, int dump_type) {
  char target[PATH_MAX];
  char temporary[PATH_MAX];
  sqlite3 *sqlite;
  int fd;
  int length;
  int rc;

  if (gamedb_target_path(context, target, sizeof(target), dump_type) < 0) {
    gamedb_log_failure(context->log, "building path",
                       context->configuration->database.gamedb, nullptr);
    return -1;
  }
  length = snprintf(temporary, sizeof(temporary), "%s.tmp.XXXXXX", target);
  if (length < 0 || (size_t)length >= sizeof(temporary)) {
    gamedb_log_failure(context->log, "building temporary path", target,
                       nullptr);
    return -1;
  }

  fd = mkstemp(temporary);
  if (fd < 0) {
    gamedb_log_failure(context->log, "creating temporary file", target,
                       nullptr);
    return -1;
  }
  if (close(fd) < 0) {
    gamedb_log_failure(context->log, "closing temporary file", temporary,
                       nullptr);
    unlink(temporary);
    return -1;
  }

  sqlite = nullptr;
  rc = sqlite3_open_v2(temporary, &sqlite, SQLITE_OPEN_READWRITE, nullptr);
  if (rc != SQLITE_OK) {
    gamedb_log_failure(context->log, "opening temporary database", temporary,
                       sqlite);
    if (sqlite)
      sqlite3_close(sqlite);
    unlink(temporary);
    return -1;
  }

  if (gamedb_store_snapshot(context, sqlite, dump_type) < 0) {
    gamedb_log_failure(context->log, "writing snapshot", temporary, sqlite);
    sqlite3_close(sqlite);
    unlink(temporary);
    return -1;
  }
  if (sqlite3_close(sqlite) != SQLITE_OK) {
    gamedb_log_failure(context->log, "closing snapshot", temporary, sqlite);
    unlink(temporary);
    return -1;
  }
  if (gamedb_fsync_file(temporary) < 0) {
    gamedb_log_failure(context->log, "syncing snapshot", temporary, nullptr);
    unlink(temporary);
    return -1;
  }
  if (rename(temporary, target) < 0) {
    gamedb_log_failure(context->log, "replacing snapshot", target, nullptr);
    unlink(temporary);
    return -1;
  }
  if (gamedb_fsync_directory(target) < 0) {
    gamedb_log_failure(context->log, "syncing snapshot directory", target,
                       nullptr);
    return -1;
  }
  return 0;
}
