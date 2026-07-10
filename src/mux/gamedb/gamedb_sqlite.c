/* gamedb_sqlite.c -- SQLite game database snapshot writer */

#include "config.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <unistd.h>

#include "alloc.h"
#include "attrs.h"
#include "db.h"
#include "externs.h"
#include "flags.h"
#include "gamedb/gamedb.h"
#include "mudconf.h"
#include "powers.h"
#include "vattr.h"

#define GAMEDB_SCHEMA_VERSION 2
#define GAMEDB_SOURCE_FORMAT_SQLITE 1

#ifndef O_DIRECTORY
#define O_DIRECTORY 0
#endif

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
    " pennies INTEGER NOT NULL,"
    " flags INTEGER NOT NULL,"
    " flags2 INTEGER NOT NULL,"
    " flags3 INTEGER NOT NULL,"
    " powers INTEGER NOT NULL,"
    " powers2 INTEGER NOT NULL,"
    " lock_expr TEXT NOT NULL"
    ");"
    "CREATE TABLE attributes ("
    " object_dbref INTEGER NOT NULL REFERENCES objects(dbref),"
    " number INTEGER NOT NULL,"
    " value TEXT NOT NULL,"
    " PRIMARY KEY (object_dbref, number)"
    ") WITHOUT ROWID;";

static void gamedb_log_failure(const char *stage, const char *path,
                               sqlite3 *sqlite) {
  const char *detail;

  detail = sqlite ? sqlite3_errmsg(sqlite) : strerror(errno);
  log_error(LOG_ALWAYS, "GDB", "FAIL", "SQLite %s for %s: %s", (char *)stage,
            (char *)path, (char *)detail);
}

static int gamedb_exec(sqlite3 *sqlite, const char *sql) {
  char *errmsg;
  int rc;

  errmsg = NULL;
  rc = sqlite3_exec(sqlite, sql, NULL, NULL, &errmsg);
  if (errmsg)
    sqlite3_free(errmsg);
  return rc == SQLITE_OK ? 0 : -1;
}

static int gamedb_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE)
    return -1;
  if (sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

static int gamedb_bind_int(sqlite3_stmt *statement, int index, long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) ==
                 SQLITE_OK
             ? 0
             : -1;
}

static int gamedb_target_path(char *target, size_t target_size, int dump_type) {
  int length;

  switch (dump_type) {
  case DUMP_CRASHED:
    length = snprintf(target, target_size, "%s.CRASH", mudconf.gamedb);
    break;
  case DUMP_KILLED:
    length = snprintf(target, target_size, "%s.KILLED", mudconf.gamedb);
    break;
  default:
    length = snprintf(target, target_size, "%s", mudconf.gamedb);
    break;
  }
  return length < 0 || (size_t)length >= target_size ? -1 : 0;
}

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

static int gamedb_prepare(sqlite3 *sqlite, sqlite3_stmt **statement,
                          const char *sql) {
  return sqlite3_prepare_v2(sqlite, sql, -1, statement, NULL) == SQLITE_OK
             ? 0
             : -1;
}

static int gamedb_column_int(sqlite3_stmt *statement, int column,
                             int *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < INT_MIN || number > INT_MAX)
    return -1;
  *value = (int)number;
  return 0;
}

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

static int gamedb_load_metadata(sqlite3 *sqlite, int *db_top) {
  sqlite3_stmt *statement;
  int attr_next;
  int min_size;
  int record_players;
  int schema_version;
  int result;

  statement = NULL;
  result = -1;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT schema_version, db_top, min_size, attr_next, "
                     "record_players FROM "
                     "snapshot WHERE id = 1;") < 0 ||
      sqlite3_step(statement) != SQLITE_ROW ||
      gamedb_column_int(statement, 0, &schema_version) < 0 ||
      gamedb_column_int(statement, 1, db_top) < 0 ||
      gamedb_column_int(statement, 2, &min_size) < 0 ||
      gamedb_column_int(statement, 3, &attr_next) < 0 ||
      gamedb_column_int(statement, 4, &record_players) < 0 ||
      sqlite3_step(statement) != SQLITE_DONE)
    goto done;

  if ((schema_version != 1 && schema_version != GAMEDB_SCHEMA_VERSION) ||
      *db_top <= 0 || min_size < 0 || attr_next < 0 || record_players < 0)
    goto done;

  mudstate.min_size = min_size;
  mudstate.attr_next = attr_next;
  mudstate.record_players = record_players;
  result = 0;

done:
  sqlite3_finalize(statement);
  return result;
}

static int gamedb_load_vattrs(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  const char *name;
  char vattr_name[VNAME_SIZE + 1];
  int flags;
  int number;
  int result;
  int step;

  statement = NULL;
  result = -1;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT number, name, flags FROM vattrs ORDER BY number;") <
      0)
    goto done;

  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (gamedb_column_int(statement, 0, &number) < 0 ||
        gamedb_column_text(statement, 1, &name, sizeof(vattr_name)) < 0 ||
        gamedb_column_int(statement, 2, &flags) < 0)
      goto done;
    StringCopy(vattr_name, name);
    if (!vattr_define(vattr_name, number, flags))
      goto done;
  }
  if (step != SQLITE_DONE)
    goto done;
  result = 0;

done:
  sqlite3_finalize(statement);
  return result;
}

static int gamedb_load_objects(sqlite3 *sqlite, int db_top) {
  sqlite3_stmt *statement;
  BOOLEXP *lock;
  const char *lock_text;
  const char *name;
  dbref object;
  dbref contents;
  dbref exits;
  FLAG flags;
  FLAG flags2;
  FLAG flags3;
  dbref link;
  dbref location;
  dbref next;
  dbref owner;
  dbref parent;
  int pennies;
  int powers;
  int powers2;
  int result;
  int step;
  dbref zone;

  statement = NULL;
  result = -1;
  if (gamedb_prepare(
          sqlite, &statement,
          "SELECT dbref, name, location, zone, contents, exits, link, next, "
          "owner, parent, pennies, flags, flags2, flags3, powers, powers2, "
          "lock_expr FROM objects ORDER BY dbref;") < 0)
    goto done;

  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
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
        gamedb_column_int(statement, 10, &pennies) < 0 ||
        gamedb_column_long(statement, 11, &flags) < 0 ||
        gamedb_column_long(statement, 12, &flags2) < 0 ||
        gamedb_column_long(statement, 13, &flags3) < 0 ||
        gamedb_column_int(statement, 14, &powers) < 0 ||
        gamedb_column_int(statement, 15, &powers2) < 0 ||
        gamedb_column_text(statement, 16, &lock_text, LBUF_SIZE) < 0)
      goto done;

    s_Name(object, (char *)name);
    s_Location(object, location);
    s_Zone(object, zone);
    s_Contents(object, contents);
    s_Exits(object, exits);
    s_Link(object, link);
    s_Next(object, next);
    s_Owner(object, owner);
    s_Parent(object, parent);
    s_Pennies(object, pennies);
    s_Flags(object, flags);
    s_Flags2(object, flags2);
    s_Flags3(object, flags3);
    s_Powers(object, powers);
    s_Powers2(object, powers2);
    lock = parse_boolexp(GOD, lock_text, 1);
    atr_add_raw(object, A_LOCK, unparse_boolexp_quiet(GOD, lock));
    free_boolexp(lock);
    if (Typeof(object) == TYPE_PLAYER)
      c_Connected(object);
  }
  if (step != SQLITE_DONE)
    goto done;
  result = 0;

done:
  sqlite3_finalize(statement);
  return result;
}

static int gamedb_load_attributes(sqlite3 *sqlite, int db_top) {
  sqlite3_stmt *statement;
  const char *value;
  dbref object;
  int attribute;
  int result;
  int step;

  statement = NULL;
  result = -1;
  if (gamedb_prepare(sqlite, &statement,
                     "SELECT object_dbref, number, value FROM attributes "
                     "ORDER BY object_dbref, number;") < 0)
    goto done;

  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    if (gamedb_column_long(statement, 0, &object) < 0 || object < 0 ||
        object >= db_top || gamedb_column_int(statement, 1, &attribute) < 0 ||
        attribute <= 0 ||
        gamedb_column_text(statement, 2, &value, LBUF_SIZE) < 0)
      goto done;
    atr_add_raw(object, attribute, (char *)value);
  }
  if (step != SQLITE_DONE)
    goto done;
  result = 0;

done:
  sqlite3_finalize(statement);
  return result;
}

int gamedb_load(const char *path) {
  sqlite3 *sqlite;
  int db_top;
  int result;

  sqlite = NULL;
  result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK) {
    gamedb_log_failure("opening game database", path, sqlite);
    goto done;
  }
  if (gamedb_load_metadata(sqlite, &db_top) < 0) {
    gamedb_log_failure("validating snapshot metadata", path, sqlite);
    goto done;
  }

  db_free();
  db_grow(db_top);
  if (gamedb_load_vattrs(sqlite) < 0 ||
      gamedb_load_objects(sqlite, db_top) < 0 ||
      gamedb_load_attributes(sqlite, db_top) < 0) {
    gamedb_log_failure("loading snapshot data", path, sqlite);
    goto done;
  }
  load_player_names();
  result = 0;

done:
  if (sqlite)
    sqlite3_close(sqlite);
  return result;
}

static int gamedb_store_snapshot(sqlite3 *sqlite, int dump_type) {
  sqlite3_stmt *snapshot;
  sqlite3_stmt *vattrs;
  sqlite3_stmt *objects;
  sqlite3_stmt *attributes;
  VATTR *vattr;
  BOOLEXP *lock;
  ATTR *attribute;
  char *attr_cursor;
  char *lock_text;
  char *lock_source;
  char *attr_text;
  dbref object;
  dbref attr_number;
  dbref attr_owner;
  long attr_flags;
  int rc;

  snapshot = NULL;
  vattrs = NULL;
  objects = NULL;
  attributes = NULL;
  rc = -1;

  if (gamedb_exec(sqlite,
                  "PRAGMA journal_mode = DELETE; PRAGMA synchronous = FULL; "
                  "PRAGMA foreign_keys = ON;") < 0 ||
      gamedb_exec(sqlite, "BEGIN IMMEDIATE;") < 0 ||
      gamedb_exec(sqlite, schema_sql) < 0 ||
      gamedb_exec(sqlite, "PRAGMA application_id = "
                          "1112821080; PRAGMA user_version = 1;") < 0)
    goto done;

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
          "parent, pennies, flags, flags2, flags3, powers, powers2, lock_expr) "
          "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);") < 0 ||
      gamedb_prepare(sqlite, &attributes,
                     "INSERT INTO attributes (object_dbref, number, value) "
                     "VALUES (?, ?, ?);") < 0)
    goto done;

  if (gamedb_bind_int(snapshot, 1, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 2, GAMEDB_SOURCE_FORMAT_SQLITE) < 0 ||
      gamedb_bind_int(snapshot, 3, GAMEDB_SCHEMA_VERSION) < 0 ||
      gamedb_bind_int(snapshot, 4, dump_type) < 0 ||
      gamedb_bind_int(snapshot, 5, mudstate.now) < 0 ||
      gamedb_bind_int(snapshot, 6, mudstate.db_top) < 0 ||
      gamedb_bind_int(snapshot, 7, mudstate.min_size) < 0 ||
      gamedb_bind_int(snapshot, 8, mudstate.attr_next) < 0 ||
      gamedb_bind_int(snapshot, 9, mudstate.record_players) < 0 ||
      gamedb_step(snapshot) < 0)
    goto done;

  for (vattr = vattr_first(); vattr; vattr = vattr_next(vattr)) {
    if (vattr->flags & AF_DELETED)
      continue;
    if (gamedb_bind_int(vattrs, 1, vattr->number) < 0 ||
        sqlite3_bind_text(vattrs, 2, vattr->name, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_bind_int(vattrs, 3, vattr->flags) < 0 ||
        gamedb_step(vattrs) < 0)
      goto done;
  }

  DO_WHOLE_DB(object) {
    if (Going(object))
      continue;

    lock_source = atr_get(object, A_LOCK, &attr_owner, &attr_flags);
    lock = parse_boolexp(GOD, lock_source, 1);
    lock_text = unparse_boolexp_quiet(GOD, lock);
    if (gamedb_bind_int(objects, 1, object) < 0 ||
        sqlite3_bind_text(objects, 2, Name(object), -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_bind_int(objects, 3, Location(object)) < 0 ||
        gamedb_bind_int(objects, 4, Zone(object)) < 0 ||
        gamedb_bind_int(objects, 5, Contents(object)) < 0 ||
        gamedb_bind_int(objects, 6, Exits(object)) < 0 ||
        gamedb_bind_int(objects, 7, Link(object)) < 0 ||
        gamedb_bind_int(objects, 8, Next(object)) < 0 ||
        gamedb_bind_int(objects, 9, Owner(object)) < 0 ||
        gamedb_bind_int(objects, 10, Parent(object)) < 0 ||
        gamedb_bind_int(objects, 11, Pennies(object)) < 0 ||
        gamedb_bind_int(objects, 12, Flags(object)) < 0 ||
        gamedb_bind_int(objects, 13, Flags2(object)) < 0 ||
        gamedb_bind_int(objects, 14, Flags3(object)) < 0 ||
        gamedb_bind_int(objects, 15, Powers(object)) < 0 ||
        gamedb_bind_int(objects, 16, Powers2(object)) < 0 ||
        sqlite3_bind_text(objects, 17, lock_text, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        gamedb_step(objects) < 0) {
      free_boolexp(lock);
      free_lbuf(lock_source);
      goto done;
    }
    free_boolexp(lock);
    free_lbuf(lock_source);

    for (attr_number = atr_head(object, &attr_cursor); attr_number;
         attr_number = atr_next(&attr_cursor)) {
      attribute = atr_num(attr_number);
      if (!attribute)
        continue;
      switch (attribute->number) {
      case A_NAME:
      case A_LOCK:
      case A_LIST:
      case A_MONEY:
        continue;
      default:
        break;
      }
      attr_text = atr_get_raw(object, attribute->number);
      if (!attr_text ||
          gamedb_bind_int(attributes, 1, object) < 0 ||
          gamedb_bind_int(attributes, 2, attribute->number) < 0 ||
          sqlite3_bind_text(attributes, 3, attr_text, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          gamedb_step(attributes) < 0) {
        if (attr_cursor)
          free(attr_cursor);
        goto done;
      }
    }
    if (attr_cursor)
      free(attr_cursor);
  }

  if (gamedb_exec(sqlite, "COMMIT;") < 0)
    goto done;
  rc = 0;

done:
  if (rc != 0)
    gamedb_exec(sqlite, "ROLLBACK;");
  sqlite3_finalize(snapshot);
  sqlite3_finalize(vattrs);
  sqlite3_finalize(objects);
  sqlite3_finalize(attributes);
  return rc;
}

int gamedb_dump(int dump_type) {
  char target[PATH_MAX];
  char temporary[PATH_MAX];
  sqlite3 *sqlite;
  int fd;
  int length;
  int rc;

  if (gamedb_target_path(target, sizeof(target), dump_type) < 0) {
    gamedb_log_failure("building path", mudconf.gamedb, NULL);
    return -1;
  }
  length = snprintf(temporary, sizeof(temporary), "%s.tmp.XXXXXX", target);
  if (length < 0 || (size_t)length >= sizeof(temporary)) {
    gamedb_log_failure("building temporary path", target, NULL);
    return -1;
  }

  fd = mkstemp(temporary);
  if (fd < 0) {
    gamedb_log_failure("creating temporary file", target, NULL);
    return -1;
  }
  if (close(fd) < 0) {
    gamedb_log_failure("closing temporary file", temporary, NULL);
    unlink(temporary);
    return -1;
  }

  sqlite = NULL;
  rc = sqlite3_open_v2(temporary, &sqlite, SQLITE_OPEN_READWRITE, NULL);
  if (rc != SQLITE_OK) {
    gamedb_log_failure("opening temporary database", temporary, sqlite);
    if (sqlite)
      sqlite3_close(sqlite);
    unlink(temporary);
    return -1;
  }

  if (gamedb_store_snapshot(sqlite, dump_type) < 0) {
    gamedb_log_failure("writing snapshot", temporary, sqlite);
    sqlite3_close(sqlite);
    unlink(temporary);
    return -1;
  }
  if (sqlite3_close(sqlite) != SQLITE_OK) {
    gamedb_log_failure("closing snapshot", temporary, sqlite);
    unlink(temporary);
    return -1;
  }
  if (gamedb_fsync_file(temporary) < 0) {
    gamedb_log_failure("syncing snapshot", temporary, NULL);
    unlink(temporary);
    return -1;
  }
  if (rename(temporary, target) < 0) {
    gamedb_log_failure("replacing snapshot", target, NULL);
    unlink(temporary);
    return -1;
  }
  if (gamedb_fsync_directory(target) < 0) {
    gamedb_log_failure("syncing snapshot directory", target, NULL);
    return -1;
  }
  return 0;
}
