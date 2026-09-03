/* gamedb_sqlite.c -- SQLite game-database persistence */

#include <limits.h>
#include <linux/limits.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

#include "mux/objects/character_state.h"
#include "mux/objects/db.h"
#include "mux/objects/economy_parts.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/objects/player_account.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h"
#include "mux/persistence/gamedb_sqlite_internal.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"

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

  const char *tables[] = {"btech_object_state"};
  const size_t TABLE_COUNT = sizeof(tables) / sizeof(*tables);
  for (size_t index = 0; index < TABLE_COUNT; index++) {
    const char *table = *(const char *const *)checked_storage_at_const(
        (const void *)tables, TABLE_COUNT, sizeof(*tables), index);
    (void)snprintf(query, sizeof(query),
                   "INSERT INTO %s (object_dbref) VALUES (?);", table);
    if (gamedb_prepare(sqlite, &statement, query) < 0 ||
        gamedb_bind_int(statement, 1, object) < 0 ||
        gamedb_step(statement) < 0) {
      sqlite3_finalize(statement);
      return -1;
    }
    sqlite3_finalize(statement);
    statement = nullptr;
  }
  for (size_t index = 0; index < NATIVE_COLUMN_COUNT; index++) {
    const NativeColumn *column = gamedb_native_column_at(index);
    const char *value = attribute_get_raw(database, object, column->field);

    if (!value)
      continue;
    (void)snprintf(query, sizeof(query), "UPDATE %s SET %s = ? WHERE %s = ?;",
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

static int gamedb_store_player_account(GameDatabase *database, sqlite3 *sqlite,
                                       DbRef player) {
  sqlite3_stmt *state = nullptr;
  sqlite3_stmt *history = nullptr;
  sqlite3_stmt *last_page = nullptr;
  const char *password_hash = player_account_password_hash(database, player);
  const char *alias = player_account_alias(database, player);
  const char *last_site = player_account_last_site(database, player);
  PlayerLastLoginResult last_login = player_account_last_login(
      (PlayerAccountRef){.database = database, .player = player});
  int result = -1;

  if (gamedb_prepare(
          sqlite, &state,
          "INSERT INTO player_state "
          "(object_dbref, password_hash, alias, last_login, last_site, "
          "successful_login_count, failed_login_count, "
          "unreported_failed_login_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?);") <
          0 ||
      gamedb_bind_int(state, 1, player) < 0 ||
      (*password_hash
           ? sqlite3_bind_text(state, 2, password_hash, -1, SQLITE_TRANSIENT)
           : sqlite3_bind_null(state, 2)) != SQLITE_OK ||
      (*alias ? sqlite3_bind_text(state, 3, alias, -1, SQLITE_TRANSIENT)
              : sqlite3_bind_null(state, 3)) != SQLITE_OK ||
      (last_login.found ? sqlite3_bind_int64(state, 4, last_login.occurred_at)
                        : sqlite3_bind_null(state, 4)) != SQLITE_OK ||
      (*last_site ? sqlite3_bind_text(state, 5, last_site, -1, SQLITE_TRANSIENT)
                  : sqlite3_bind_null(state, 5)) != SQLITE_OK ||
      sqlite3_bind_int64(
          state, 6, player_account_successful_login_count(database, player)) !=
          SQLITE_OK ||
      sqlite3_bind_int64(state, 7,
                         player_account_failed_login_count(database, player)) !=
          SQLITE_OK ||
      sqlite3_bind_int64(state, 8,
                         player_account_unreported_failed_login_count(
                             database, player)) != SQLITE_OK ||
      gamedb_step(state) < 0)
    goto done;

  if (gamedb_prepare(sqlite, &history,
                     "INSERT INTO player_login_history "
                     "(player_dbref, outcome, position, occurred_at, host) "
                     "VALUES (?, ?, ?, ?, ?);") < 0)
    goto done;
  for (PlayerLoginOutcome outcome = PLAYER_LOGIN_SUCCESS;
       outcome <= PLAYER_LOGIN_FAILURE; outcome++) {
    size_t count =
        player_account_login_history_count((PlayerLoginHistoryRequest){
            .account = {.database = database, .player = player},
            .outcome = outcome});
    for (size_t position = 0; position < count; position++) {
      PlayerLoginHistoryResult record =
          player_account_login_history(&(PlayerLoginHistoryRequest){
              .account = {.database = database, .player = player},
              .outcome = outcome,
              .position = position});
      if (!record.found || gamedb_bind_int(history, 1, player) < 0 ||
          gamedb_bind_int(history, 2, outcome) < 0 ||
          gamedb_bind_int(history, 3, (long)position) < 0 ||
          sqlite3_bind_int64(history, 4, record.record.occurred_at) !=
              SQLITE_OK ||
          sqlite3_bind_text(history, 5, record.record.host, -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          gamedb_step(history) < 0)
        goto done;
    }
  }

  if (gamedb_prepare(sqlite, &last_page,
                     "INSERT INTO player_last_page_recipients "
                     "(player_dbref, position, recipient_dbref) "
                     "VALUES (?, ?, ?);") < 0)
    goto done;
  for (size_t position = 0;
       position < player_account_last_page_count(database, player);
       position++) {
    PlayerPageRecipientResult recipient =
        player_account_last_page_recipient(&(PlayerPageRecipientRequest){
            .account = {.database = database, .player = player},
            .position = position});
    if (!recipient.found || gamedb_bind_int(last_page, 1, player) < 0 ||
        gamedb_bind_int(last_page, 2, (long)position) < 0 ||
        gamedb_bind_int(last_page, 3, recipient.recipient) < 0 ||
        gamedb_step(last_page) < 0)
      goto done;
  }
  result = 0;

done:
  sqlite3_finalize(state);
  sqlite3_finalize(history);
  sqlite3_finalize(last_page);
  return result;
}

static int gamedb_store_economy_parts(GameDatabase *database, sqlite3 *sqlite,
                                      DbRef object) {
  sqlite3_stmt *statement = nullptr;
  int result = -1;

  if (gamedb_prepare(sqlite, &statement,
                     "INSERT INTO btech_economy_parts "
                     "(object_dbref, part_id, brand_id, quantity) "
                     "VALUES (?, ?, ?, ?);") < 0)
    return -1;
  for (size_t index = 0; index < economy_parts_entry_count(database, object);
       index++) {
    EconomyPartsEntryResult entry_result =
        economy_parts_entry(&(EconomyPartsEntryRequest){
            .database = database, .object = object, .index = index});
    EconomyPartEntryView entry = entry_result.entry;

    if (!entry_result.found || gamedb_bind_int(statement, 1, object) < 0 ||
        gamedb_bind_int(statement, 2, entry.part_id) < 0 ||
        gamedb_bind_int(statement, 3, entry.brand_id) < 0 ||
        gamedb_bind_int(statement, 4, entry.quantity) < 0 ||
        gamedb_step(statement) < 0)
      goto done;
  }
  result = 0;

done:
  sqlite3_finalize(statement);
  return result;
}

static int gamedb_store_character_state(GameDatabase *database, sqlite3 *sqlite,
                                        DbRef player) {
  sqlite3_stmt *state = nullptr;
  sqlite3_stmt *value = nullptr;
  CharacterFixedState fixed;
  int result = -1;

  if (!character_state_exists(database, player))
    return 0;
  if (!character_state_fixed_get(database, player, &fixed) ||
      gamedb_prepare(sqlite, &state,
                     "INSERT INTO btech_character_state "
                     "(player_dbref, bruise, lethal, build, reflexes, "
                     "intuition, learn, charisma) "
                     "VALUES (?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_bind_int(state, 1, player) < 0 ||
      gamedb_bind_int(state, 2, fixed.bruise) < 0 ||
      gamedb_bind_int(state, 3, fixed.lethal) < 0 ||
      gamedb_bind_int(state, 4, fixed.build) < 0 ||
      gamedb_bind_int(state, 5, fixed.reflexes) < 0 ||
      gamedb_bind_int(state, 6, fixed.intuition) < 0 ||
      gamedb_bind_int(state, 7, fixed.learn) < 0 ||
      gamedb_bind_int(state, 8, fixed.charisma) < 0 || gamedb_step(state) < 0 ||
      gamedb_prepare(sqlite, &value,
                     "INSERT INTO btech_character_values "
                     "(player_dbref, value_name, value, xp, last_used) "
                     "VALUES (?, ?, ?, ?, ?);") < 0)
    goto done;

  for (size_t index = 0; index < character_state_value_count(database, player);
       index++) {
    CharacterStateEntryResult entry_result =
        character_state_value_entry(&(CharacterStateEntryRequest){
            .database = database, .player = player, .index = index});
    CharacterValueStateView entry = entry_result.entry;
    if (!entry_result.found || gamedb_bind_int(value, 1, player) < 0 ||
        sqlite3_bind_text(value, 2, entry.name, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_bind_int(value, 3, entry.value) < 0 ||
        gamedb_bind_int(value, 4, entry.xp) < 0 ||
        sqlite3_bind_int64(value, 5, entry.last_used) != SQLITE_OK ||
        gamedb_step(value) < 0)
      goto done;
  }
  result = 0;

done:
  sqlite3_finalize(state);
  sqlite3_finalize(value);
  return result;
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
      gamedb_exec(sqlite, SCHEMA_OBJECTS_SQL) < 0 ||
      gamedb_exec(sqlite, SCHEMA_STATE_SQL) < 0 ||
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
          "(dbref, name, location, zone, affiliation, contents, exits, link, "
          "next, type, lua_parent, description, internal_description, "
          "has_ansi_flag, has_audible_flag, "
          "has_auditorium_flag, has_blind_flag, has_connected_flag, "
          "has_dark_flag, has_floating_flag, has_gagged_flag, has_going_flag, "
          "has_halted_flag, has_in_character_flag, has_light_flag, "
          "has_monitor_flag, has_no_command_flag, "
          "has_safe_flag, has_suspect_flag, "
          "has_transparent_flag, has_wizard_flag, has_xcode_flag, "
          "has_zombie_flag, "
          "has_idle_power) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
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
                        game_object_affiliation(context->database, object)) <
            0 ||
        gamedb_bind_int(objects, 6,
                        game_object_contents(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 7,
                        game_object_exits(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 8,
                        game_object_link(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 9,
                        game_object_next(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 10, typeof_obj(context->database, object)) <
            0 ||
        sqlite3_bind_text(objects, 11,
                          game_object_lua_parent(context->database, object), -1,
                          SQLITE_TRANSIENT) != SQLITE_OK) {
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
    }
    const char *description =
        game_object_description(context->database, object);
    const char *internal_description =
        game_object_internal_description(context->database, object);
    if ((description
             ? sqlite3_bind_text(objects, 12, description, -1, SQLITE_TRANSIENT)
             : sqlite3_bind_null(objects, 12)) != SQLITE_OK ||
        (internal_description
             ? sqlite3_bind_text(objects, 13, internal_description, -1,
                                 SQLITE_TRANSIENT)
             : sqlite3_bind_null(objects, 13)) != SQLITE_OK)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
    for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++) {
      if (gamedb_bind_int(objects, 13 + (int)flag,
                          game_object_has_flag(&(ObjectFlagRequest){
                              .database = context->database,
                              .object = object,
                              .flag = flag})) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
    }
    for (PowerId power = POWER_IDLE; power < POWER_COUNT; power++) {
      if (gamedb_bind_int(objects, 33 + (int)power,
                          game_object_has_power(&(ObjectPowerRequest){
                              .database = context->database,
                              .object = object,
                              .power = power})) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state,
                                      0);
    }
    if (gamedb_step(objects) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);

    if ((typeof_obj(context->database, object) == OBJECT_TYPE_PLAYER &&
         (gamedb_store_player_account(context->database, sqlite, object) < 0 ||
          gamedb_store_character_state(context->database, sqlite, object) <
              0)) ||
        gamedb_store_native_state(context->database, sqlite, object) < 0 ||
        gamedb_store_economy_parts(context->database, sqlite, object) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, object_state, 0);
    for (size_t index = 0;
         index < object_state_count(context->database, object); index++) {
      ObjectStateEntryResult entry_result =
          object_state_entry(&(ObjectStateEntryRequest){
              .database = context->database, .object = object, .index = index});
      ObjectStateEntryView entry = entry_result.entry;
      int bind_result = SQLITE_ERROR;

      if (!entry_result.found || gamedb_bind_int(object_state, 1, object) < 0 ||
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
static int gamedb_write(PersistenceContext *context, int dump_type,
                        bool replace) {
  char target[PATH_MAX];
  char temporary[PATH_MAX];
  sqlite3 *sqlite;
  int fd;
  int length;
  int rc;

  if (gamedb_target_path(
          &(GamedbTargetPathRequest){.context = context,
                                     .target = target,
                                     .target_size = sizeof(target),
                                     .dump_type = dump_type}) < 0) {
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
  if ((replace && rename(temporary, target) < 0) ||
      (!replace && link(temporary, target) < 0)) {
    gamedb_log_failure(context->log,
                       replace ? "replacing snapshot" : "creating snapshot",
                       target, nullptr);
    unlink(temporary);
    return -1;
  }
  if (!replace)
    unlink(temporary);
  if (gamedb_fsync_directory(target) < 0) {
    gamedb_log_failure(context->log, "syncing snapshot directory", target,
                       nullptr);
    return -1;
  }
  return 0;
}

int gamedb_dump(PersistenceContext *context, int dump_type) {
  return gamedb_write(context, dump_type, true);
}

int gamedb_create(PersistenceContext *context) {
  return gamedb_write(context, DUMP_NORMAL, false);
}
