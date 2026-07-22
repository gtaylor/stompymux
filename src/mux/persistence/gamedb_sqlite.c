/* gamedb_sqlite.c -- SQLite game-database persistence */

#include "mux/server/platform.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <unistd.h>

#include "mux/objects/attrs.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

// Increment whenever the schema written by this module changes.
constexpr int GAMEDB_SCHEMA_VERSION = 14;

// Identifies SQLite as the storage implementation in snapshot metadata.
constexpr int GAMEDB_SOURCE_FORMAT_SQLITE = 1;

#ifndef O_DIRECTORY
#define O_DIRECTORY 0
#endif

/*
 * Each file holds one complete game snapshot. Object attributes and dynamic
 * attribute definitions are normalized so a future incremental store can use
 * the same schema without changing the on-disk representation.
 */
static const char schema_objects_sql[] =
    "CREATE TABLE snapshot ("
    " id INTEGER PRIMARY KEY CHECK (id = 1),"
    " schema_version INTEGER NOT NULL,"
    " storage_format INTEGER NOT NULL,"
    " storage_version INTEGER NOT NULL,"
    " dump_type INTEGER NOT NULL,"
    " dump_time INTEGER NOT NULL,"
    " db_top INTEGER NOT NULL,"
    " min_size INTEGER NOT NULL,"
    " record_players INTEGER NOT NULL"
    ");"
    "CREATE TABLE objects ("
    " dbref INTEGER PRIMARY KEY,"
    " name TEXT NOT NULL,"
    " location INTEGER NOT NULL,"
    " zone INTEGER NOT NULL,"
    " contents INTEGER NOT NULL,"
    " exits INTEGER NOT NULL,"
    " link INTEGER NOT NULL,"
    " next INTEGER NOT NULL,"
    " type INTEGER NOT NULL CHECK (type IN (0, 1, 2, 3, 5)),"
    " lua_parent TEXT NOT NULL DEFAULT '',"
    " has_ansi_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_ansi_flag IN (0, 1)),"
    " has_ansimap_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_ansimap_flag IN "
    "(0, 1)),"
    " has_audible_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_audible_flag IN "
    "(0, 1)),"
    " has_auditorium_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_auditorium_flag IN (0, 1)),"
    " has_blind_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_blind_flag IN (0, "
    "1)),"
    " has_connected_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_connected_flag "
    "IN (0, 1)),"
    " has_dark_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_dark_flag IN (0, 1)),"
    " has_floating_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_floating_flag IN "
    "(0, 1)),"
    " has_gagged_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_gagged_flag IN (0, "
    "1)),"
    " has_going_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_going_flag IN (0, "
    "1)),"
    " has_halted_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_halted_flag IN (0, "
    "1)),"
    " has_in_character_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_in_character_flag IN (0, 1)),"
    " has_light_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_light_flag IN (0, "
    "1)),"
    " has_monitor_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_monitor_flag IN "
    "(0, 1)),"
    " has_no_command_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_no_command_flag IN (0, 1)),"
    " has_quiet_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_quiet_flag IN (0, "
    "1)),"
    " has_safe_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_safe_flag IN (0, 1)),"
    " has_suspect_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_suspect_flag IN "
    "(0, 1)),"
    " has_transparent_flag INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_transparent_flag IN (0, 1)),"
    " has_wizard_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_wizard_flag IN (0, "
    "1)),"
    " has_xcode_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_xcode_flag IN (0, "
    "1)),"
    " has_zombie_flag INTEGER NOT NULL DEFAULT 0 CHECK (has_zombie_flag IN (0, "
    "1)),"
    " has_idle_power INTEGER NOT NULL DEFAULT 0 CHECK (has_idle_power IN (0, "
    "1)),"
    " has_long_fingers_power INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_long_fingers_power IN (0, 1)),"
    " has_comm_all_power INTEGER NOT NULL DEFAULT 0 CHECK (has_comm_all_power "
    "IN (0, 1)),"
    " has_see_hidden_power INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_see_hidden_power IN (0, 1)),"
    " has_no_destroy_power INTEGER NOT NULL DEFAULT 0 CHECK "
    "(has_no_destroy_power IN (0, 1)),"
    " has_mech_power INTEGER NOT NULL DEFAULT 0 CHECK (has_mech_power IN (0, "
    "1)),"
    " has_security_power INTEGER NOT NULL DEFAULT 0 CHECK (has_security_power "
    "IN (0, 1)),"
    " has_mechrep_power INTEGER NOT NULL DEFAULT 0 CHECK (has_mechrep_power IN "
    "(0, 1)),"
    " has_map_power INTEGER NOT NULL DEFAULT 0 CHECK (has_map_power IN (0, 1)),"
    " has_template_power INTEGER NOT NULL DEFAULT 0 CHECK (has_template_power "
    "IN (0, 1)),"
    " has_tech_power INTEGER NOT NULL DEFAULT 0 CHECK (has_tech_power IN (0, "
    "1))"
    ");";

static const char schema_state_sql[] =
    "CREATE TABLE object_state ("
    " object_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),"
    " description TEXT, inside_description TEXT, admin_comment TEXT,"
    " enter_alias TEXT, leave_alias TEXT, destroyer INTEGER"
    ");"
    "CREATE TABLE player_state ("
    " object_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),"
    " password_hash TEXT, alias TEXT, last_login TEXT, last_name_change TEXT,"
    " login_data TEXT, last_site TEXT, last_page TEXT, timeout INTEGER,"
    " queue_limit INTEGER, privileges TEXT, timezone TEXT"
    ");"
    "CREATE TABLE btech_object_state ("
    " object_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref),"
    " mech_preferred_id TEXT, map_color TEXT, mech_skills TEXT,"
    " object_type TEXT, tactical_size TEXT, lrs_height TEXT,"
    " contact_options TEXT, mech_name TEXT, mech_type TEXT,"
    " mech_description TEXT, mw_template TEXT,"
    " faction TEXT, job TEXT, rank_number INTEGER, health TEXT,"
    " character_attributes TEXT, build_links TEXT, build_entrances TEXT,"
    " build_coordinates TEXT, advantages TEXT, pilot_dbref INTEGER,"
    " map_visibility TEXT, tech_complete_at INTEGER, economy_parts TEXT,"
    " skills TEXT, personal_combat_equipment TEXT"
    ");"
    "CREATE TABLE attributes ("
    " object_dbref INTEGER NOT NULL REFERENCES objects(dbref),"
    " name TEXT NOT NULL,"
    " value TEXT NOT NULL,"
    " PRIMARY KEY (object_dbref, name)"
    ") WITHOUT ROWID;";

typedef struct NativeColumn NativeColumn;
struct NativeColumn {
  int field;
  const char *table;
  const char *column;
};

static const NativeColumn native_columns[] = {
    {A_DESC, "object_state", "description"},
    {A_IDESC, "object_state", "inside_description"},
    {A_COMMENT, "object_state", "admin_comment"},
    {A_EALIAS, "object_state", "enter_alias"},
    {A_LALIAS, "object_state", "leave_alias"},
    {A_DESTROYER, "object_state", "destroyer"},
    {A_PASS, "player_state", "password_hash"},
    {A_ALIAS, "player_state", "alias"},
    {A_LAST, "player_state", "last_login"},
    {A_LASTNAME, "player_state", "last_name_change"},
    {A_LOGINDATA, "player_state", "login_data"},
    {A_LASTSITE, "player_state", "last_site"},
    {A_LASTPAGE, "player_state", "last_page"},
    {A_TIMEOUT, "player_state", "timeout"},
    {A_QUEUEMAX, "player_state", "queue_limit"},
    {A_PRIVS, "player_state", "privileges"},
    {A_TZ, "player_state", "timezone"},
    {A_MECHPREFID, "btech_object_state", "mech_preferred_id"},
    {A_MAPCOLOR, "btech_object_state", "map_color"},
    {A_MECHSKILLS, "btech_object_state", "mech_skills"},
    {A_XTYPE, "btech_object_state", "object_type"},
    {A_TACSIZE, "btech_object_state", "tactical_size"},
    {A_LRSHEIGHT, "btech_object_state", "lrs_height"},
    {A_CONTACTOPT, "btech_object_state", "contact_options"},
    {A_MECHNAME, "btech_object_state", "mech_name"},
    {A_MECHTYPE, "btech_object_state", "mech_type"},
    {A_MECHDESC, "btech_object_state", "mech_description"},
    {A_MWTEMPLATE, "btech_object_state", "mw_template"},
    {A_FACTION, "btech_object_state", "faction"},
    {A_JOB, "btech_object_state", "job"},
    {A_RANKNUM, "btech_object_state", "rank_number"},
    {A_HEALTH, "btech_object_state", "health"},
    {A_ATTRS, "btech_object_state", "character_attributes"},
    {A_BUILDLINKS, "btech_object_state", "build_links"},
    {A_BUILDENTRANCE, "btech_object_state", "build_entrances"},
    {A_BUILDCOORD, "btech_object_state", "build_coordinates"},
    {A_ADVS, "btech_object_state", "advantages"},
    {A_PILOTNUM, "btech_object_state", "pilot_dbref"},
    {A_MAPVIS, "btech_object_state", "map_visibility"},
    {A_TECHTIME, "btech_object_state", "tech_complete_at"},
    {A_ECONPARTS, "btech_object_state", "economy_parts"},
    {A_SKILLS, "btech_object_state", "skills"},
    {A_PCEQUIP, "btech_object_state", "personal_combat_equipment"},
};

/* Log either the SQLite error or the current operating-system error. */
static void gamedb_log_failure(ServerLog *log, const char *stage,
                               const char *path, sqlite3 *sqlite) {
  const char *detail;

  detail = sqlite ? sqlite3_errmsg(sqlite) : strerror(errno);
  log_error(log, LOG_ALWAYS, "GDB", "FAIL", "SQLite %s for %s: %s", stage, path,
            detail);
}

/* Report a subsystem persistence failure with its registered extension name. */
static void gamedb_log_extension_failure(ServerLog *log, const char *operation,
                                         const char *name, const char *path,
                                         sqlite3 *sqlite) {
  const char *detail;

  detail = sqlite ? sqlite3_errmsg(sqlite) : "extension callback failed";
  log_error(log, LOG_ALWAYS, "GDB", "FAIL",
            "SQLite persistence extension %s failed while %s %s: %s", name,
            operation, path, detail);
}

/* Restore every registered subsystem while its snapshot connection is open. */
static int gamedb_load_extensions(PersistenceContext *context, sqlite3 *sqlite,
                                  const char *path) {
  size_t index;

  for (index = 0; index < context->extension_count; index++) {
    PersistenceSqliteExtension *extension = &context->extensions[index];
    if (extension->load(sqlite, context, extension->context) < 0) {
      gamedb_log_extension_failure(context->log, "loading", extension->name,
                                   path, sqlite);
      return -1;
    }
  }
  return 0;
}

/* Store every registered subsystem before committing the full snapshot. */
static int gamedb_store_extensions(PersistenceContext *context,
                                   sqlite3 *sqlite) {
  size_t index;

  for (index = 0; index < context->extension_count; index++) {
    PersistenceSqliteExtension *extension = &context->extensions[index];
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
static int gamedb_exec(sqlite3 *sqlite, const char *sql) {
  char *errmsg;
  int rc;

  errmsg = nullptr;
  rc = sqlite3_exec(sqlite, sql, nullptr, nullptr, &errmsg);
  if (errmsg)
    sqlite3_free(errmsg);
  return rc == SQLITE_OK ? 0 : -1;
}

/* Execute a reusable INSERT statement and reset it for the next row. */
static int gamedb_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE)
    return -1;
  if (sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

/* Bind a MUX integer to SQLite's signed 64-bit integer representation. */
static int gamedb_bind_int(sqlite3_stmt *statement, int index, long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) == SQLITE_OK
             ? 0
             : -1;
}

/* Select the configured SQLite file for a normal or exceptional dump. */
static int gamedb_target_path(const PersistenceContext *context, char *target,
                              size_t target_size, int dump_type) {
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
static int gamedb_fsync_file(const char *path) {
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
static int gamedb_fsync_directory(const char *path) {
  char directory[PATH_MAX];
  char *slash;
  int fd;
  int result;

  if (strlen(path) >= sizeof(directory))
    return -1;
  StringCopy(directory, path);
  slash = strrchr(directory, '/');
  if (!slash)
    StringCopy(directory, ".");
  else if (slash == directory)
    slash[1] = '\0';
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
static int gamedb_prepare(sqlite3 *sqlite, sqlite3_stmt **statement,
                          const char *sql) {
  return sqlite3_prepare_v2(sqlite, sql, -1, statement, nullptr) == SQLITE_OK
             ? 0
             : -1;
}

/* Read an SQLite integer only when it fits the destination int exactly. */
static int gamedb_column_int(sqlite3_stmt *statement, int column, int *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < INT_MIN || number > INT_MAX)
    return -1;
  *value = (int)number;
  return 0;
}

static int gamedb_column_bool(sqlite3_stmt *statement, int column,
                              bool *value) {
  int number;

  if (gamedb_column_int(statement, column, &number) < 0 ||
      (number != 0 && number != 1))
    return -1;
  *value = number != 0;
  return 0;
}

/* Read an SQLite integer only when it fits the destination long exactly. */
static int gamedb_column_long(sqlite3_stmt *statement, int column,
                              long *value) {
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
static int gamedb_column_text(sqlite3_stmt *statement, int column,
                              const char **value, int maximum_size) {
  const unsigned char *text;
  int length;

  if (sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  text = sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  if (!text || length < 0 || length >= maximum_size ||
      (int)strlen((const char *)text) != length)
    return -1;
  *value = (const char *)text;
  return 0;
}

/* Validate singleton snapshot metadata and restore global allocation state. */
static int gamedb_load_metadata(PersistenceContext *context, sqlite3 *sqlite,
                                int *db_top, int *loaded_schema_version) {
  sqlite3_stmt *statement;
  int min_size;
  int record_players;
  int schema_version;
  int result;

  statement = nullptr;
  result = -1;
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
    *loaded_schema_version = schema_version;
    result = 0;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore object headers. */
static int gamedb_load_objects(PersistenceContext *context, sqlite3 *sqlite,
                               int db_top, int schema_version) {
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
  bool powers[POWER_TECH + 1];
  int result;
  int step;
  DbRef zone;

  statement = nullptr;
  result = -1;
  const char *query =
      "SELECT dbref, name, location, zone, contents, exits, link, next, "
      "type, lua_parent, has_ansi_flag, has_ansimap_flag, has_audible_flag, "
      "has_auditorium_flag, has_blind_flag, has_connected_flag, has_dark_flag, "
      "has_floating_flag, has_gagged_flag, has_going_flag, has_halted_flag, "
      "has_in_character_flag, has_light_flag, has_monitor_flag, "
      "has_no_command_flag, has_quiet_flag, has_safe_flag, "
      "has_suspect_flag, has_transparent_flag, has_wizard_flag, "
      "has_xcode_flag, has_zombie_flag, has_idle_power, "
      "has_long_fingers_power, has_comm_all_power, "
      "has_see_hidden_power, has_no_destroy_power, has_mech_power, "
      "has_security_power, has_mechrep_power, has_map_power, "
      "has_template_power, has_tech_power "
      "FROM objects "
      "ORDER BY dbref;";
  (void)schema_version;
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
         type != OBJECT_TYPE_GARBAGE)) {
      result = -1;
    } else {
      for (ObjectFlag flag = OBJECT_FLAG_ANSI;
           result == 0 && flag < OBJECT_FLAG_COUNT; flag++)
        if (gamedb_column_bool(statement, 9 + (int)flag, &object_flags[flag]) <
            0)
          result = -1;
      for (PowerId power = POWER_IDLE; result == 0 && power <= POWER_TECH;
           power++)
        if (gamedb_column_bool(statement, 31 + (int)power, &powers[power]) < 0)
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
                             object_flags[flag]);
      for (PowerId power = POWER_IDLE; power <= POWER_TECH; power++)
        game_object_set_power(context->database, object, power, powers[power]);
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

  for (size_t index = 0;
       index < sizeof(native_columns) / sizeof(*native_columns); index++) {
    const NativeColumn *column = &native_columns[index];
    sqlite3_stmt *statement = nullptr;
    int step;

    snprintf(
        query, sizeof(query),
        "SELECT object_dbref, CAST(%s AS TEXT) FROM %s WHERE %s IS NOT NULL;",
        column->column, column->table, column->column);
    if (gamedb_prepare(sqlite, &statement, query) < 0)
      return -1;
    while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
      const char *value;
      DbRef object;

      if (gamedb_column_long(statement, 0, &object) < 0 ||
          !is_good_obj(context->database, object) ||
          gamedb_column_text(statement, 1, &value, LBUF_SIZE) < 0) {
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

static int gamedb_load_attributes(PersistenceContext *context,
                                  sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  int step;

  if (gamedb_prepare(sqlite, &statement,
                     "SELECT object_dbref, name, value FROM attributes "
                     "ORDER BY object_dbref, name;") < 0)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const char *name;
    const char *value;
    DbRef object;

    if (gamedb_column_long(statement, 0, &object) < 0 ||
        !is_good_obj(context->database, object) ||
        gamedb_column_text(statement, 1, &name, SBUF_SIZE) < 0 ||
        gamedb_column_text(statement, 2, &value, LBUF_SIZE) < 0 ||
        !dynamic_attribute_set(context->database, object, name, value)) {
      sqlite3_finalize(statement);
      return -1;
    }
  }
  sqlite3_finalize(statement);
  return step == SQLITE_DONE ? 0 : -1;
}

/* Open, validate, and load one SQLite snapshot into the global database. */
int gamedb_load(PersistenceContext *context, const char *path) {
  sqlite3 *sqlite;
  int db_top;
  int schema_version;
  int result;

  sqlite = nullptr;
  result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, nullptr) !=
      SQLITE_OK) {
    gamedb_log_failure(context->log, "opening game database", path, sqlite);
  } else if (gamedb_load_metadata(context, sqlite, &db_top, &schema_version) <
             0) {
    gamedb_log_failure(context->log, "validating snapshot metadata", path,
                       sqlite);
  } else {
    db_free(context->database);
    db_grow(context->database, db_top);
    if (gamedb_load_objects(context, sqlite, db_top, schema_version) < 0 ||
        gamedb_load_native_state(context, sqlite) < 0 ||
        gamedb_load_attributes(context, sqlite) < 0) {
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
static int gamedb_finish_snapshot(sqlite3 *sqlite, sqlite3_stmt *snapshot,
                                  sqlite3_stmt *objects,
                                  sqlite3_stmt *attributes, int success) {
  if (!success)
    gamedb_exec(sqlite, "ROLLBACK;");
  sqlite3_finalize(snapshot);
  sqlite3_finalize(objects);
  sqlite3_finalize(attributes);
  return success ? 0 : -1;
}

static int gamedb_store_native_state(GameDatabase *database, sqlite3 *sqlite,
                                     DbRef object) {
  sqlite3_stmt *statement = nullptr;
  char query[256];

  const char *tables[] = {"object_state", "player_state", "btech_object_state"};
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
  for (size_t index = 0;
       index < sizeof(native_columns) / sizeof(*native_columns); index++) {
    const NativeColumn *column = &native_columns[index];
    const char *value = attribute_get_raw(database, object, column->field);

    if (!value)
      continue;
    snprintf(query, sizeof(query),
             "UPDATE %s SET %s = ? WHERE object_dbref = ?;", column->table,
             column->column);
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
  sqlite3_stmt *attributes;
  DbRef object;

  snapshot = nullptr;
  objects = nullptr;
  attributes = nullptr;

  if (gamedb_exec(sqlite,
                  "PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; "
                  "PRAGMA foreign_keys = ON;") < 0 ||
      gamedb_exec(sqlite, "BEGIN IMMEDIATE;") < 0 ||
      gamedb_exec(sqlite, schema_objects_sql) < 0 ||
      gamedb_exec(sqlite, schema_state_sql) < 0 ||
      gamedb_exec(sqlite, "PRAGMA application_id = "
                          "1112821080; PRAGMA user_version = 1;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);

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
          "has_ansi_flag, has_ansimap_flag, has_audible_flag, "
          "has_auditorium_flag, has_blind_flag, has_connected_flag, "
          "has_dark_flag, has_floating_flag, has_gagged_flag, has_going_flag, "
          "has_halted_flag, has_in_character_flag, has_light_flag, "
          "has_monitor_flag, has_no_command_flag, "
          "has_quiet_flag, has_safe_flag, has_suspect_flag, "
          "has_transparent_flag, has_wizard_flag, has_xcode_flag, "
          "has_zombie_flag, "
          "has_idle_power, has_long_fingers_power, has_comm_all_power, "
          "has_see_hidden_power, has_no_destroy_power, has_mech_power, "
          "has_security_power, has_mechrep_power, has_map_power, "
          "has_template_power, has_tech_power) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, "
          "?, ?);") < 0 ||
      gamedb_prepare(sqlite, &attributes,
                     "INSERT INTO attributes (object_dbref, name, value) "
                     "VALUES (?, ?, ?);") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);

  if (gamedb_bind_int(snapshot, 1, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 2, GAMEDB_SOURCE_FORMAT_SQLITE) < 0 ||
      gamedb_bind_int(snapshot, 3, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 4, dump_type) < 0 ||
      gamedb_bind_int(snapshot, 5, *context->now) < 0 ||
      gamedb_bind_int(snapshot, 6, context->database->top) < 0 ||
      gamedb_bind_int(snapshot, 7, context->database->minimum_size) < 0 ||
      gamedb_bind_int(snapshot, 8, *context->record_players) < 0 ||
      gamedb_step(snapshot) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);

  DO_WHOLE_DB(context->database, object) {
    if (is_going(context->database, object))
      continue;
    GameObject *game_object = game_database_object(context->database, object);
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
      return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
    }
    for (ObjectFlag flag = OBJECT_FLAG_ANSI; flag < OBJECT_FLAG_COUNT; flag++) {
      if (gamedb_bind_int(
              objects, 10 + (int)flag,
              game_object_has_flag(context->database, object, flag)) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
    }
    for (PowerId power = POWER_IDLE; power <= POWER_TECH; power++) {
      if (gamedb_bind_int(
              objects, 32 + (int)power,
              game_object_has_power(context->database, object, power)) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
    }
    if (gamedb_step(objects) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);

    if (gamedb_store_native_state(context->database, sqlite, object) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
    for (int index = 0; index < game_object->at_count; index++) {
      AttributeList *entry = &game_object->ahead[index];
      if (gamedb_bind_int(attributes, 1, object) < 0 ||
          sqlite3_bind_text(attributes, 2, entry->name, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          sqlite3_bind_text(attributes, 3, entry->data, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          gamedb_step(attributes) < 0)
        return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
    }
  }

  if (gamedb_store_extensions(context, sqlite) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);

  if (gamedb_exec(sqlite, "COMMIT;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 0);
  return gamedb_finish_snapshot(sqlite, snapshot, objects, attributes, 1);
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
