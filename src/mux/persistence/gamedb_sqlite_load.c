/* gamedb_sqlite.c -- SQLite game-database persistence */

#include <limits.h>
#include <linux/limits.h>
#include <sqlite3.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/objects/attrs.h"
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
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"
#include "mux/world/player.h"

static int gamedb_load_metadata(PersistenceContext *context, sqlite3 *sqlite,
                                int *db_top) {
  sqlite3_stmt *statement;
  int min_size;
  int record_players;
  int result = -1;
  int schema_version;

  statement = nullptr;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT schema_version, db_top, min_size, "
                     "record_players FROM snapshot WHERE id = 1;") == 0 &&
      sqlite3_step(statement) == SQLITE_ROW &&
      gamedb_column_int(statement, 0, &schema_version) == 0 &&
      gamedb_column_int(statement, 1, db_top) == 0 &&
      gamedb_column_int(statement, 2, &min_size) == 0 &&
      gamedb_column_int(statement, 3, &record_players) == 0 &&
      sqlite3_step(statement) == SQLITE_DONE &&
      schema_version == GAMEDB_SCHEMA_VERSION && *db_top > 0 && min_size >= 0 &&
      record_players >= 0) {
    context->database->minimum_size = min_size;
    *context->record_players = record_players;
    result = 0;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore object headers. */
static int gamedb_load_objects(PersistenceContext *context, sqlite3 *sqlite,
                               int db_top) {
  sqlite3_stmt *statement;
  const char *lua_parent;
  const char *name;
  DbRef object;
  DbRef contents;
  DbRef exits;
  int type;
  bool object_flags[OBJECT_FLAG_COUNT];
  DbRef link;
  DbRef location;
  DbRef next;
  bool powers[POWER_COUNT];
  int result;
  int step;
  DbRef zone;

  statement = nullptr;
  const char *query =
      "SELECT dbref, name, location, zone, contents, exits, link, next, "
      "type, lua_parent, has_ansi_flag, has_audible_flag, "
      "has_auditorium_flag, has_blind_flag, has_connected_flag, "
      "has_dark_flag, "
      "has_floating_flag, has_gagged_flag, has_going_flag, "
      "has_halted_flag, "
      "has_in_character_flag, has_light_flag, has_monitor_flag, "
      "has_no_command_flag, has_safe_flag, "
      "has_suspect_flag, has_transparent_flag, has_wizard_flag, "
      "has_xcode_flag, has_zombie_flag, has_idle_power "
      "FROM objects "
      "ORDER BY dbref;";
  if (gamedb_prepare(sqlite, &statement, query) < 0) {
    sqlite3_finalize(statement);
    return -1;
  }

  result = 0;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (gamedb_column_long(statement, 0, &object) < 0 || object < 0 ||
        object >= db_top ||
        gamedb_column_text(statement, 1, &name, MBUF_SIZE) < 0 ||
        gamedb_column_long(statement, 2, &location) < 0 ||
        gamedb_column_long(statement, 3, &zone) < 0 ||
        gamedb_column_long(statement, 4, &contents) < 0 ||
        gamedb_column_long(statement, 5, &exits) < 0 ||
        gamedb_column_long(statement, 6, &link) < 0 ||
        gamedb_column_long(statement, 7, &next) < 0 ||
        gamedb_column_int(statement, 8, &type) < 0 ||
        gamedb_column_text(statement, 9, &lua_parent, PATH_MAX) < 0 ||
        (type != OBJECT_TYPE_ROOM && type != OBJECT_TYPE_THING &&
         type != OBJECT_TYPE_EXIT && type != OBJECT_TYPE_PLAYER &&
         type != OBJECT_TYPE_GARBAGE) ||
        !utf8_validate(name, strlen(name)) ||
        (type == OBJECT_TYPE_PLAYER &&
         !ok_player_name(context->configuration, name))) {
      result = -1;
    } else {
      for (ObjectFlag flag = OBJECT_FLAG_ANSI;
           result == 0 && flag < OBJECT_FLAG_COUNT; flag++)
        if (gamedb_column_bool(
                statement, 9 + (int)flag,
                checked_storage_at(object_flags, OBJECT_FLAG_COUNT,
                                   sizeof(*object_flags), (size_t)flag)) < 0)
          result = -1;
      for (PowerId power = POWER_IDLE; result == 0 && power < POWER_COUNT;
           power++)
        if (gamedb_column_bool(statement, 29 + (int)power,
                               checked_storage_at(powers, POWER_COUNT,
                                                  sizeof(*powers),
                                                  (size_t)power)) < 0)
          result = -1;
      if (result != 0)
        continue;
      /* object_name_set()'s parameter isn't const-correct; name is only
         read (copied) here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      object_name_set(context->database, object, (char *)name);
#pragma clang diagnostic pop
      game_object_set_location(context->database, object, location);
      game_object_set_zone(context->database, object, zone);
      game_object_set_contents(context->database, object, contents);
      game_object_set_exits(context->database, object, exits);
      game_object_set_link(context->database, object, link);
      game_object_set_next(context->database, object, next);
      game_object_set_type(context->database, object, (ObjectType)type);
      if (!game_object_lua_parent_set(context->database, object, lua_parent))
        result = -1;
      game_object_clear_flags(context->database, object);
      for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++)
        game_object_set_flag(context->database, object, flag,
                             *(const bool *)checked_storage_at_const(
                                 object_flags, OBJECT_FLAG_COUNT,
                                 sizeof(*object_flags), (size_t)flag));
      for (PowerId power = POWER_IDLE; power < POWER_COUNT; power++)
        game_object_set_power(
            context->database, object, power,
            *(const bool *)checked_storage_at_const(
                powers, POWER_COUNT, sizeof(*powers), (size_t)power));
      if (typeof_obj(context->database, object) == OBJECT_TYPE_PLAYER)
        c_connected(context->database, object);
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

static int gamedb_load_native_state(PersistenceContext *context,
                                    sqlite3 *sqlite) {
  char query[256];

  for (size_t index = 0; index < native_column_count; index++) {
    const NativeColumn *column = gamedb_native_column_at(index);
    sqlite3_stmt *statement = nullptr;
    int step;

    (void)snprintf(query, sizeof(query),
                   "SELECT %s, CAST(%s AS TEXT) FROM %s WHERE %s IS NOT NULL;",
                   column->key_column, column->column, column->table,
                   column->column);
    if (gamedb_prepare(sqlite, &statement, query) < 0)
      return -1;
    while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
      const char *value;
      DbRef object;

      if (gamedb_column_long(statement, 0, &object) < 0 ||
          !is_good_obj(context->database, object) ||
          gamedb_column_text(statement, 1, &value, LBUF_SIZE) < 0 ||
          (column->field == A_ALIAS &&
           !ok_player_name(context->configuration, value))) {
        sqlite3_finalize(statement);
        return -1;
      }
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      attribute_add_raw(context->database, object, column->field,
                        (char *)value);
#pragma clang diagnostic pop
    }
    sqlite3_finalize(statement);
    if (step != SQLITE_DONE)
      return -1;
  }
  return 0;
}

static int gamedb_load_player_accounts(PersistenceContext *context,
                                       sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  bool *seen = calloc((size_t)context->database->top, sizeof(*seen));
  DbRef object;
  int step;

  if (!seen || gamedb_prepare(
                   sqlite, &statement,
                   "SELECT object_dbref, password_hash, last_login, last_site, "
                   "successful_login_count, failed_login_count, "
                   "unreported_failed_login_count FROM player_state "
                   "ORDER BY object_dbref;") < 0) {
    free(seen);
    return -1;
  }
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const char *password_hash = nullptr;
    const char *last_site = nullptr;
    DbRef player;
    long last_login = 0;
    int64_t successful = sqlite3_column_int64(statement, 4);
    int64_t failed = sqlite3_column_int64(statement, 5);
    int64_t unreported = sqlite3_column_int64(statement, 6);

    if (gamedb_column_long(statement, 0, &player) < 0 ||
        !is_good_obj(context->database, player) ||
        typeof_obj(context->database, player) != OBJECT_TYPE_PLAYER ||
        *(const bool *)checked_storage_at_const(
            seen, (size_t)context->database->top, sizeof(*seen),
            (size_t)player) ||
        (sqlite3_column_type(statement, 1) != SQLITE_NULL &&
         gamedb_column_text(statement, 1, &password_hash, LBUF_SIZE) < 0) ||
        (sqlite3_column_type(statement, 2) != SQLITE_NULL &&
         (sqlite3_column_type(statement, 2) != SQLITE_INTEGER ||
          gamedb_column_long(statement, 2, &last_login) < 0)) ||
        (sqlite3_column_type(statement, 3) != SQLITE_NULL &&
         gamedb_column_text(statement, 3, &last_site, LBUF_SIZE) < 0) ||
        sqlite3_column_type(statement, 4) != SQLITE_INTEGER ||
        sqlite3_column_type(statement, 5) != SQLITE_INTEGER ||
        sqlite3_column_type(statement, 6) != SQLITE_INTEGER ||
        !player_account_login_counts_set(context->database, player, successful,
                                         failed, unreported) ||
        (password_hash && !player_account_password_hash_set(
                              context->database, player, password_hash)) ||
        (last_site &&
         !player_account_last_site_set(context->database, player, last_site)) ||
        (sqlite3_column_type(statement, 2) != SQLITE_NULL &&
         !player_account_last_login_set(context->database, player,
                                        (time_t)last_login))) {
      sqlite3_finalize(statement);
      free(seen);
      return -1;
    }
    *(bool *)checked_storage_at(seen, (size_t)context->database->top,
                                sizeof(*seen), (size_t)player) = true;
  }
  sqlite3_finalize(statement);
  if (step != SQLITE_DONE) {
    free(seen);
    return -1;
  }
  DO_WHOLE_DB(context->database, object) {
    if (typeof_obj(context->database, object) == OBJECT_TYPE_PLAYER &&
        !is_going(context->database, object) &&
        !*(const bool *)checked_storage_at_const(
            seen, (size_t)context->database->top, sizeof(*seen),
            (size_t)object)) {
      free(seen);
      return -1;
    }
  }
  free(seen);

  statement = nullptr;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT player_dbref, outcome, position, occurred_at, "
                     "host FROM player_login_history "
                     "ORDER BY player_dbref, outcome, position;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const char *host;
    DbRef player;
    long occurred_at;
    int outcome;
    int position;

    if (gamedb_column_long(statement, 0, &player) < 0 ||
        !is_good_obj(context->database, player) ||
        typeof_obj(context->database, player) != OBJECT_TYPE_PLAYER ||
        gamedb_column_int(statement, 1, &outcome) < 0 ||
        (outcome != PLAYER_LOGIN_SUCCESS && outcome != PLAYER_LOGIN_FAILURE) ||
        gamedb_column_int(statement, 2, &position) < 0 || position < 0 ||
        gamedb_column_long(statement, 3, &occurred_at) < 0 ||
        gamedb_column_text(statement, 4, &host, LBUF_SIZE) < 0 ||
        !player_account_login_history_set(
            context->database, player, (PlayerLoginOutcome)outcome,
            (size_t)position, (time_t)occurred_at, host)) {
      sqlite3_finalize(statement);
      return -1;
    }
  }
  sqlite3_finalize(statement);
  if (step != SQLITE_DONE)
    return -1;

  statement = nullptr;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT player_dbref, position, recipient_dbref FROM "
                     "player_last_page_recipients "
                     "ORDER BY player_dbref, position;") < 0)
    return -1;
  DbRef current_player = NOTHING;
  DbRef *recipients = nullptr;
  size_t recipient_count = 0;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    DbRef player;
    DbRef recipient;
    int position;

    if (gamedb_column_long(statement, 0, &player) < 0 ||
        !is_good_obj(context->database, player) ||
        typeof_obj(context->database, player) != OBJECT_TYPE_PLAYER ||
        gamedb_column_int(statement, 1, &position) < 0 || position < 0 ||
        gamedb_column_long(statement, 2, &recipient) < 0) {
      free(recipients);
      sqlite3_finalize(statement);
      return -1;
    }
    if (player != current_player) {
      if (current_player != NOTHING &&
          !player_account_last_page_set(context->database, current_player,
                                        recipients, recipient_count)) {
        free(recipients);
        sqlite3_finalize(statement);
        return -1;
      }
      free(recipients);
      recipients = nullptr;
      recipient_count = 0;
      current_player = player;
    }
    if ((size_t)position != recipient_count) {
      free(recipients);
      sqlite3_finalize(statement);
      return -1;
    }
    DbRef *grown = realloc(recipients, (recipient_count + 1) * sizeof(*grown));
    if (!grown) {
      free(recipients);
      sqlite3_finalize(statement);
      return -1;
    }
    recipients = grown;
    recipient_count++;
    *(DbRef *)checked_storage_at(recipients, recipient_count,
                                 sizeof(*recipients), recipient_count - 1) =
        recipient;
  }
  sqlite3_finalize(statement);
  if (step != SQLITE_DONE ||
      (current_player != NOTHING &&
       !player_account_last_page_set(context->database, current_player,
                                     recipients, recipient_count))) {
    free(recipients);
    return -1;
  }
  free(recipients);
  return 0;
}

static int gamedb_load_economy_parts(PersistenceContext *context,
                                     sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  int step;

  if (gamedb_prepare(sqlite, &statement,
                     "SELECT object_dbref, part_id, brand_id, quantity "
                     "FROM btech_economy_parts "
                     "ORDER BY object_dbref, part_id, brand_id;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    DbRef object;
    int part_id, brand_id, quantity;

    if (gamedb_column_long(statement, 0, &object) < 0 ||
        !is_good_obj(context->database, object) ||
        is_going(context->database, object) ||
        gamedb_column_int(statement, 1, &part_id) < 0 ||
        gamedb_column_int(statement, 2, &brand_id) < 0 ||
        gamedb_column_int(statement, 3, &quantity) < 0 || quantity <= 0 ||
        !economy_parts_set_quantity(context->database, object, part_id,
                                    brand_id, quantity)) {
      sqlite3_finalize(statement);
      return -1;
    }
  }
  sqlite3_finalize(statement);
  return step == SQLITE_DONE ? 0 : -1;
}

static int gamedb_load_character_state(PersistenceContext *context,
                                       sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  int step;

  if (gamedb_prepare(sqlite, &statement,
                     "SELECT player_dbref, bruise, lethal, build, reflexes, "
                     "intuition, learn, charisma FROM btech_character_state "
                     "ORDER BY player_dbref;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    DbRef player;
    int fields[7];
    if (gamedb_column_long(statement, 0, &player) < 0 ||
        !is_good_obj(context->database, player) ||
        typeof_obj(context->database, player) != OBJECT_TYPE_PLAYER ||
        is_going(context->database, player))
      goto invalid;
    for (int column = 0; column < 7; column++) {
      int *field =
          checked_storage_at(fields, 7, sizeof(*fields), (size_t)column);
      if (gamedb_column_int(statement, column + 1, field) < 0 || *field < 0 ||
          *field > UINT8_MAX)
        goto invalid;
    }
    CharacterFixedState fixed = {
        .bruise = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 0),
        .lethal = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 1),
        .build = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 2),
        .reflexes = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 3),
        .intuition = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 4),
        .learn = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 5),
        .charisma = (unsigned char)*(const int *)checked_storage_at_const(
            fields, 7, sizeof(*fields), 6),
    };
    if (!character_state_fixed_set(context->database, player, &fixed))
      goto invalid;
  }
  sqlite3_finalize(statement);
  if (step != SQLITE_DONE)
    return -1;

  statement = nullptr;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT player_dbref, value_name, value, xp, last_used "
                     "FROM btech_character_values "
                     "ORDER BY player_dbref, value_name;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const char *name;
    DbRef player;
    int value, xp;
    long last_used;
    if (gamedb_column_long(statement, 0, &player) < 0 ||
        !character_state_exists(context->database, player) ||
        gamedb_column_text(statement, 1, &name, 256) < 0 ||
        gamedb_column_int(statement, 2, &value) < 0 ||
        gamedb_column_int(statement, 3, &xp) < 0 ||
        gamedb_column_long(statement, 4, &last_used) < 0 ||
        !character_state_value_set(context->database, player, name, value, xp,
                                   (time_t)last_used))
      goto invalid;
  }
  sqlite3_finalize(statement);
  return step == SQLITE_DONE ? 0 : -1;

invalid:
  sqlite3_finalize(statement);
  return -1;
}

static int gamedb_load_object_state(PersistenceContext *context,
                                    sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  int step;

  if (gamedb_prepare(sqlite, &statement,
                     "SELECT object_dbref, namespace, key, value_type, value "
                     "FROM object_state "
                     "ORDER BY object_dbref, namespace, key;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const char *name_space;
    const char *key;
    ObjectStateValue value;
    DbRef object;
    int type = sqlite3_column_int(statement, 3);
    char error[256];

    if (gamedb_column_long(statement, 0, &object) < 0 ||
        !is_good_obj(context->database, object) ||
        gamedb_column_text(statement, 1, &name_space, 128) < 0 ||
        gamedb_column_text(statement, 2, &key, 256) < 0 ||
        type < OBJECT_STATE_STRING || type > OBJECT_STATE_NUMBER) {
      sqlite3_finalize(statement);
      return -1;
    }
    memset(&value, 0, sizeof(value));
    value.type = (ObjectStateType)type;
    switch (value.type) {
    case OBJECT_STATE_STRING:
      value.as.string.data = sqlite3_column_blob(statement, 4);
      value.as.string.length = (size_t)sqlite3_column_bytes(statement, 4);
      if (sqlite3_column_type(statement, 4) != SQLITE_BLOB)
        goto invalid_state;
      break;
    case OBJECT_STATE_BOOLEAN:
      if (sqlite3_column_type(statement, 4) != SQLITE_INTEGER ||
          (sqlite3_column_int(statement, 4) != 0 &&
           sqlite3_column_int(statement, 4) != 1))
        goto invalid_state;
      value.as.boolean = sqlite3_column_int(statement, 4) != 0;
      break;
    case OBJECT_STATE_INTEGER:
      if (sqlite3_column_type(statement, 4) != SQLITE_INTEGER)
        goto invalid_state;
      value.as.integer = sqlite3_column_int64(statement, 4);
      break;
    case OBJECT_STATE_NUMBER:
      if (sqlite3_column_type(statement, 4) != SQLITE_FLOAT)
        goto invalid_state;
      value.as.number = sqlite3_column_double(statement, 4);
      break;
    }
    if (!object_state_set(context->database, object, name_space, key, &value,
                          error, sizeof(error)))
      goto invalid_state;
    continue;

  invalid_state:
    sqlite3_finalize(statement);
    return -1;
  }
  sqlite3_finalize(statement);
  return step == SQLITE_DONE ? 0 : -1;
}

/* Open, validate, and load one SQLite snapshot into the global database. */
int gamedb_load(PersistenceContext *context, const char *path) {
  sqlite3 *sqlite;
  int db_top;
  int result;

  sqlite = nullptr;
  result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, nullptr) !=
      SQLITE_OK) {
    gamedb_log_failure(context->log, "opening game database", path, sqlite);
  } else if (gamedb_load_metadata(context, sqlite, &db_top) < 0) {
    gamedb_log_failure(context->log, "validating snapshot metadata", path,
                       sqlite);
  } else {
    db_free(context->database);
    db_grow(context->database, db_top);
    if (gamedb_load_objects(context, sqlite, db_top) < 0 ||
        gamedb_load_player_accounts(context, sqlite) < 0 ||
        gamedb_load_native_state(context, sqlite) < 0 ||
        gamedb_load_character_state(context, sqlite) < 0 ||
        gamedb_load_economy_parts(context, sqlite) < 0 ||
        gamedb_load_object_state(context, sqlite) < 0) {
      gamedb_log_failure(context->log, "loading snapshot data", path, sqlite);
    } else if (gamedb_load_extensions(context, sqlite, path) < 0) {
      /* The extension has already emitted a subsystem-specific error. */
    } else {
      load_player_names(context->world);
      result = 0;
    }
  }

  if (sqlite)
    sqlite3_close(sqlite);
  return result;
}

/*
 * Complete a snapshot transaction and release every prepared statement. A
 * failed write rolls the transaction back before returning an error.
 */
