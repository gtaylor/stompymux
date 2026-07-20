/* gamedb_sqlite.c -- SQLite game-database persistence */

#include "mux/server/platform.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <unistd.h>

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/database/powers.h"
#include "mux/database/vattr.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"

// Increment whenever the schema written by this module changes.
constexpr int GAMEDB_SCHEMA_VERSION = 4;
constexpr int LEGACY_AF_LOCK = 0x0040;
constexpr int LEGACY_AF_IS_LOCK = 0x0400;

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
static const char schema_sql[] =
    "CREATE TABLE snapshot ("
    " id INTEGER PRIMARY KEY CHECK (id = 1),"
    " schema_version INTEGER NOT NULL,"
    " storage_format INTEGER NOT NULL,"
    " storage_version INTEGER NOT NULL,"
    " dump_type INTEGER NOT NULL,"
    " dump_time INTEGER NOT NULL,"
    " db_top INTEGER NOT NULL,"
    " min_size INTEGER NOT NULL,"
    " attr_next INTEGER NOT NULL,"
    " record_players INTEGER NOT NULL"
    ");"
    "CREATE TABLE vattrs ("
    " number INTEGER PRIMARY KEY,"
    " name TEXT NOT NULL,"
    " flags INTEGER NOT NULL"
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
    " owner INTEGER NOT NULL,"
    " parent INTEGER NOT NULL,"
    " flags INTEGER NOT NULL,"
    " flags2 INTEGER NOT NULL,"
    " flags3 INTEGER NOT NULL,"
    " powers INTEGER NOT NULL,"
    " powers2 INTEGER NOT NULL"
    ");"
    "CREATE TABLE attributes ("
    " object_dbref INTEGER NOT NULL REFERENCES objects(dbref),"
    " number INTEGER NOT NULL,"
    " value TEXT NOT NULL,"
    " PRIMARY KEY (object_dbref, number)"
    ") WITHOUT ROWID;";

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
  int attr_next;
  int min_size;
  int record_players;
  int schema_version;
  int result;

  statement = nullptr;
  result = -1;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT schema_version, db_top, min_size, attr_next, "
                     "record_players FROM snapshot WHERE id = 1;") == 0 &&
      sqlite3_step(statement) == SQLITE_ROW &&
      gamedb_column_int(statement, 0, &schema_version) == 0 &&
      gamedb_column_int(statement, 1, db_top) == 0 &&
      gamedb_column_int(statement, 2, &min_size) == 0 &&
      gamedb_column_int(statement, 3, &attr_next) == 0 &&
      gamedb_column_int(statement, 4, &record_players) == 0 &&
      sqlite3_step(statement) == SQLITE_DONE && schema_version >= 1 &&
      schema_version <= GAMEDB_SCHEMA_VERSION && *db_top > 0 && min_size >= 0 &&
      attr_next >= 0 && record_players >= 0) {
    context->database->minimum_size = min_size;
    vattr_store_set_next_number(context->vattrs, attr_next);
    *context->record_players = record_players;
    *loaded_schema_version = schema_version;
    result = 0;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore user-defined attribute declarations before loading their values. */
static int gamedb_load_vattrs(PersistenceContext *context, sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  const char *name;
  char vattr_name[VNAME_SIZE + 1];
  int flags;
  int number;
  int result;
  int step;

  statement = nullptr;
  result = -1;
  if (gamedb_prepare(
          sqlite, &statement,
          "SELECT number, name, flags FROM vattrs ORDER BY number;") == 0) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      if (gamedb_column_int(statement, 0, &number) < 0 ||
          gamedb_column_text(statement, 1, &name, sizeof(vattr_name)) < 0 ||
          gamedb_column_int(statement, 2, &flags) < 0) {
        result = -1;
      } else {
        StringCopy(vattr_name, name);
        flags &= ~(LEGACY_AF_LOCK | LEGACY_AF_IS_LOCK);
        if (!vattr_define(context->vattrs, vattr_name, number, flags))
          result = -1;
      }
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Restore object headers. Schema versions through 3 also contain discarded
 * legacy lock expressions. */
static int gamedb_load_objects(PersistenceContext *context, sqlite3 *sqlite,
                               int db_top, int schema_version) {
  sqlite3_stmt *statement;
  const char *lock_text;
  const char *name;
  DbRef object;
  DbRef contents;
  DbRef exits;
  Flag flags;
  Flag flags2;
  Flag flags3;
  DbRef link;
  DbRef location;
  DbRef next;
  DbRef owner;
  DbRef parent;
  int powers;
  int powers2;
  int result;
  int step;
  DbRef zone;
  int discarded_locks;

  statement = nullptr;
  result = -1;
  const char *query =
      schema_version <= 3
          ? "SELECT dbref, name, location, zone, contents, exits, link, next, "
            "owner, parent, flags, flags2, flags3, powers, powers2, lock_expr "
            "FROM objects ORDER BY dbref;"
          : "SELECT dbref, name, location, zone, contents, exits, link, next, "
            "owner, parent, flags, flags2, flags3, powers, powers2 FROM "
            "objects "
            "ORDER BY dbref;";
  if (gamedb_prepare(sqlite, &statement, query) < 0) {
    sqlite3_finalize(statement);
    return -1;
  }

  result = 0;
  discarded_locks = 0;
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
        gamedb_column_long(statement, 8, &owner) < 0 ||
        gamedb_column_long(statement, 9, &parent) < 0 ||
        gamedb_column_long(statement, 10, &flags) < 0 ||
        gamedb_column_long(statement, 11, &flags2) < 0 ||
        gamedb_column_long(statement, 12, &flags3) < 0 ||
        gamedb_column_int(statement, 13, &powers) < 0 ||
        gamedb_column_int(statement, 14, &powers2) < 0 ||
        (schema_version <= 3 &&
         gamedb_column_text(statement, 15, &lock_text, LBUF_SIZE) < 0)) {
      result = -1;
    } else {
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
      game_object_set_owner(context->database, object, owner);
      game_object_set_parent(context->database, object, parent);
      game_object_set_flags(context->database, object, flags);
      game_object_set_flags2(context->database, object, flags2);
      game_object_set_flags3(context->database, object, flags3);
      game_object_set_powers(context->database, object, powers);
      game_object_set_powers2(context->database, object, powers2);
      if (schema_version <= 3 && lock_text[0] != '\0')
        discarded_locks++;
      if (typeof_obj(context->database, object) == TYPE_PLAYER)
        c_connected(context->database, object);
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  if (result == 0 && discarded_locks > 0)
    log_error(context->log, LOG_ALWAYS, "GDB", "LOCK",
              "Discarded %d legacy object lock expressions while loading "
              "schema version %d",
              discarded_locks, schema_version);
  return result;
}

static bool gamedb_is_retired_lock_attribute(int number) {
  switch (number) {
  case 2:
  case 3:
  case 42:
  case 59:
  case 60:
  case 62:
  case 63:
  case 66:
  case 67:
  case 69:
  case 70:
  case 75:
  case 76:
  case 85:
  case 86:
  case 87:
  case 93:
  case 94:
  case 97:
  case 98:
  case 129:
  case 130:
  case 132:
  case 133:
  case 135:
  case 136:
  case 138:
  case 139:
  case 141:
  case 142:
  case 209:
    return true;
  default:
    return false;
  }
}

/* Restore ordinary attribute values after all object rows exist. */
static int gamedb_load_attributes(PersistenceContext *context, sqlite3 *sqlite,
                                  int db_top) {
  sqlite3_stmt *statement;
  const char *value;
  DbRef object;
  int attribute;
  int result;
  int step;
  int discarded_attributes;
  int scrubbed_attribute_flags;

  statement = nullptr;
  result = -1;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT object_dbref, number, value FROM attributes "
                     "ORDER BY object_dbref, number;") < 0) {
    sqlite3_finalize(statement);
    return -1;
  }

  result = 0;
  discarded_attributes = 0;
  scrubbed_attribute_flags = 0;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (gamedb_column_long(statement, 0, &object) < 0 || object < 0 ||
        object >= db_top || gamedb_column_int(statement, 1, &attribute) < 0 ||
        attribute <= 0 ||
        gamedb_column_text(statement, 2, &value, LBUF_SIZE) < 0)
      result = -1;
    else if (gamedb_is_retired_lock_attribute(attribute)) {
      discarded_attributes++;
    } else {
      /* attribute_add_raw()'s buffer parameter isn't const-correct; value is
         only read (copied) here, never mutated. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wcast-qual"
      attribute_add_raw(context->database, object, attribute, (char *)value);
#pragma clang diagnostic pop
      {
        DbRef owner;
        long flags;

        if (attribute_get_info(context->database, object, attribute, &owner,
                               &flags) &&
            (flags & (LEGACY_AF_LOCK | LEGACY_AF_IS_LOCK))) {
          attribute_set_flags(context->database, object, attribute,
                              flags & ~(LEGACY_AF_LOCK | LEGACY_AF_IS_LOCK));
          scrubbed_attribute_flags++;
        }
      }
    }
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  if (result == 0 && discarded_attributes > 0)
    log_error(context->log, LOG_ALWAYS, "GDB", "LOCK",
              "Discarded %d legacy lock and lock-failure attributes",
              discarded_attributes);
  if (result == 0 && scrubbed_attribute_flags > 0)
    log_error(context->log, LOG_ALWAYS, "GDB", "LOCK",
              "Removed legacy lock flags from %d attributes",
              scrubbed_attribute_flags);
  return result;
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
    if (gamedb_load_vattrs(context, sqlite) < 0 ||
        gamedb_load_objects(context, sqlite, db_top, schema_version) < 0 ||
        gamedb_load_attributes(context, sqlite, db_top) < 0) {
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
                                  sqlite3_stmt *vattrs, sqlite3_stmt *objects,
                                  sqlite3_stmt *attributes, int success) {
  if (!success)
    gamedb_exec(sqlite, "ROLLBACK;");
  sqlite3_finalize(snapshot);
  sqlite3_finalize(vattrs);
  sqlite3_finalize(objects);
  sqlite3_finalize(attributes);
  return success ? 0 : -1;
}

/*
 * Populate a newly created SQLite database from the live in-memory game
 * state. The transaction is committed only after every table is complete.
 */
static int gamedb_store_snapshot(PersistenceContext *context, sqlite3 *sqlite,
                                 int dump_type) {
  sqlite3_stmt *snapshot;
  sqlite3_stmt *vattrs;
  sqlite3_stmt *objects;
  sqlite3_stmt *attributes;
  VATTR *vattr;
  Attribute *attribute;
  char *attr_cursor;
  char *attr_text;
  DbRef object;
  DbRef attr_number;

  snapshot = nullptr;
  vattrs = nullptr;
  objects = nullptr;
  attributes = nullptr;

  if (gamedb_exec(sqlite,
                  "PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; "
                  "PRAGMA foreign_keys = ON;") < 0 ||
      gamedb_exec(sqlite, "BEGIN IMMEDIATE;") < 0 ||
      gamedb_exec(sqlite, schema_sql) < 0 ||
      gamedb_exec(sqlite, "PRAGMA application_id = "
                          "1112821080; PRAGMA user_version = 1;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                  0);

  if (gamedb_prepare(
          sqlite, &snapshot,
          "INSERT INTO snapshot "
          "(id, schema_version, storage_format, storage_version, dump_type, "
          "dump_time, db_top, min_size, attr_next, record_players) "
          "VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_prepare(sqlite, &vattrs,
                     "INSERT INTO vattrs (number, name, flags) VALUES (?, ?, "
                     "?);") < 0 ||
      gamedb_prepare(
          sqlite, &objects,
          "INSERT INTO objects "
          "(dbref, name, location, zone, contents, exits, link, next, owner, "
          "parent, flags, flags2, flags3, powers, powers2) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_prepare(sqlite, &attributes,
                     "INSERT INTO attributes (object_dbref, number, value) "
                     "VALUES (?, ?, ?);") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                  0);

  if (gamedb_bind_int(snapshot, 1, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 2, GAMEDB_SOURCE_FORMAT_SQLITE) < 0 ||
      gamedb_bind_int(snapshot, 3, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 4, dump_type) < 0 ||
      gamedb_bind_int(snapshot, 5, *context->now) < 0 ||
      gamedb_bind_int(snapshot, 6, context->database->top) < 0 ||
      gamedb_bind_int(snapshot, 7, context->database->minimum_size) < 0 ||
      gamedb_bind_int(snapshot, 8, vattr_store_next_number(context->vattrs)) <
          0 ||
      gamedb_bind_int(snapshot, 9, *context->record_players) < 0 ||
      gamedb_step(snapshot) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                  0);

  for (vattr = vattr_first(context->vattrs); vattr;
       vattr = vattr_next(context->vattrs, vattr)) {
    if (vattr->flags & AF_DELETED)
      continue;
    if (gamedb_bind_int(vattrs, 1, vattr->number) < 0 ||
        sqlite3_bind_text(vattrs, 2, vattr->name, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_bind_int(vattrs, 3, vattr->flags) < 0 || gamedb_step(vattrs) < 0)
      return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects,
                                    attributes, 0);
  }

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
        gamedb_bind_int(objects, 9,
                        game_object_owner(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 10,
                        game_object_parent(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 11,
                        game_object_flags(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 12,
                        game_object_flags2(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 13,
                        game_object_flags3(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 14,
                        game_object_powers(context->database, object)) < 0 ||
        gamedb_bind_int(objects, 15,
                        game_object_powers2(context->database, object)) < 0 ||
        gamedb_step(objects) < 0) {
      return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects,
                                    attributes, 0);
    }

    for (attr_number =
             attribute_list_first(context->database, object, &attr_cursor);
         attr_number; attr_number = attribute_list_next(&attr_cursor)) {
      attribute = attribute_by_number(context->database, (int)attr_number);
      if (!attribute)
        continue;
      switch (attribute->number) {
      case A_NAME:
      case A_LIST:
        continue;
      default:
        break;
      }
      attr_text =
          attribute_get_raw(context->database, object, attribute->number);
      if (!attr_text || gamedb_bind_int(attributes, 1, object) < 0 ||
          gamedb_bind_int(attributes, 2, attribute->number) < 0 ||
          sqlite3_bind_text(attributes, 3, attr_text, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          gamedb_step(attributes) < 0) {
        if (attr_cursor)
          free(attr_cursor);
        return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects,
                                      attributes, 0);
      }
    }
    if (attr_cursor)
      free(attr_cursor);
  }

  if (gamedb_store_extensions(context, sqlite) < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                  0);

  if (gamedb_exec(sqlite, "COMMIT;") < 0)
    return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                  0);
  return gamedb_finish_snapshot(sqlite, snapshot, vattrs, objects, attributes,
                                1);
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
