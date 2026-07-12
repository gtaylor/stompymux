/* restart_persistence_sqlite.c -- transient SQLite state for controlled restarts */

#include "config.h"

#include <limits.h>
#include <sqlite3.h>
#include <sys/stat.h>
#include <time.h>

#include "alloc.h"
#include "bsd.h"
#include "cque.h"
#include "db.h"
#include "dnschild.h"
#include "flags.h"
#include "interface.h"
#include "mudconf.h"
#include "netcommon.h"
#include "persistence/restart_persistence.h"

#define RESTART_SCHEMA_VERSION 1

static const char restart_schema_sql[] =
    "CREATE TABLE IF NOT EXISTS restart_metadata ("
    " id INTEGER PRIMARY KEY CHECK (id = 1),"
    " format_version INTEGER NOT NULL,"
    " created_at INTEGER NOT NULL,"
    " start_time INTEGER NOT NULL,"
    " doing_hdr TEXT NOT NULL,"
    " record_players INTEGER NOT NULL"
    ");"
    "CREATE TABLE IF NOT EXISTS restart_descriptors ("
    " descriptor INTEGER PRIMARY KEY,"
    " flags INTEGER NOT NULL,"
    " connected_at INTEGER NOT NULL,"
    " command_count INTEGER NOT NULL,"
    " timeout INTEGER NOT NULL,"
    " host_info INTEGER NOT NULL,"
    " player_dbref INTEGER NOT NULL,"
    " last_time INTEGER NOT NULL,"
    " output_prefix TEXT,"
    " output_suffix TEXT,"
    " addr TEXT NOT NULL,"
    " doing TEXT NOT NULL,"
    " username TEXT NOT NULL"
    ");"
    "CREATE TABLE IF NOT EXISTS restart_queue_objects ("
    " owner_dbref INTEGER PRIMARY KEY"
    ");"
    "CREATE TABLE IF NOT EXISTS restart_queue_entries ("
    " entry_id INTEGER PRIMARY KEY,"
    " queue_type INTEGER NOT NULL CHECK (queue_type BETWEEN 0 AND 2),"
    " owner_dbref INTEGER NOT NULL,"
    " position INTEGER NOT NULL CHECK (position >= 0),"
    " player_dbref INTEGER NOT NULL,"
    " cause_dbref INTEGER NOT NULL,"
    " semaphore_dbref INTEGER NOT NULL,"
    " wait_delay INTEGER NOT NULL CHECK (wait_delay >= 0),"
    " attribute_number INTEGER NOT NULL,"
    " text TEXT,"
    " command TEXT,"
    " nargs INTEGER NOT NULL CHECK (nargs >= 0),"
    " UNIQUE (queue_type, owner_dbref, position)"
    ");"
    "CREATE TABLE IF NOT EXISTS restart_queue_env ("
    " entry_id INTEGER NOT NULL REFERENCES restart_queue_entries(entry_id),"
    " position INTEGER NOT NULL CHECK (position >= 0),"
    " value TEXT,"
    " PRIMARY KEY (entry_id, position)"
    ") WITHOUT ROWID;"
    "CREATE TABLE IF NOT EXISTS restart_queue_scr ("
    " entry_id INTEGER NOT NULL REFERENCES restart_queue_entries(entry_id),"
    " position INTEGER NOT NULL CHECK (position >= 0),"
    " value TEXT,"
    " PRIMARY KEY (entry_id, position)"
    ") WITHOUT ROWID;";

static int restart_exec(sqlite3 *sqlite, const char *sql) {
  char *error;
  int rc;

  error = NULL;
  rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);
  sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

int restart_persistence_create_schema(sqlite3 *sqlite) {
  return restart_exec(sqlite, restart_schema_sql);
}

static int restart_open(sqlite3 **sqlite) {
  *sqlite = NULL;
  if (sqlite3_open_v2(mudconf.gamedb, sqlite, SQLITE_OPEN_READWRITE, NULL) !=
      SQLITE_OK)
    return -1;
  sqlite3_busy_timeout(*sqlite, 5000);
  return 0;
}

static int restart_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE || sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

static int restart_bind_long(sqlite3_stmt *statement, int index, long value) {
  return sqlite3_bind_int64(statement, index, (sqlite3_int64)value) == SQLITE_OK
             ? 0
             : -1;
}

static int restart_store_descriptors(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  DESC *descriptor;
  int result;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "INSERT INTO restart_descriptors "
               "(descriptor, flags, connected_at, command_count, timeout, "
               "host_info, player_dbref, last_time, output_prefix, output_suffix, "
               "addr, doing, username) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  DESC_ITER_CONN(descriptor) {
    if (result < 0)
      break;
    if (restart_bind_long(statement, 1, descriptor->descriptor) < 0 ||
        restart_bind_long(statement, 2, descriptor->flags) < 0 ||
        restart_bind_long(statement, 3, descriptor->connected_at) < 0 ||
        restart_bind_long(statement, 4, descriptor->command_count) < 0 ||
        restart_bind_long(statement, 5, descriptor->timeout) < 0 ||
        restart_bind_long(statement, 6, descriptor->host_info) < 0 ||
        restart_bind_long(statement, 7, descriptor->player) < 0 ||
        restart_bind_long(statement, 8, descriptor->last_time) < 0 ||
        sqlite3_bind_text(statement, 9, descriptor->output_prefix, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(statement, 10, descriptor->output_suffix, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        sqlite3_bind_text(statement, 11, descriptor->addr, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        sqlite3_bind_text(statement, 12, descriptor->doing, -1, SQLITE_TRANSIENT) !=
            SQLITE_OK ||
        sqlite3_bind_text(statement, 13, descriptor->username, -1,
                          SQLITE_TRANSIENT) != SQLITE_OK ||
        restart_step(statement) < 0)
      result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

int restart_persistence_store(void) {
  sqlite3 *sqlite;
  sqlite3_stmt *metadata;
  int result;

  sqlite = NULL;
  metadata = NULL;
  result = restart_open(&sqlite) == 0 && restart_exec(sqlite, "PRAGMA foreign_keys = ON;") == 0 &&
                   restart_exec(sqlite, "PRAGMA synchronous = FULL;") == 0 &&
                   restart_exec(sqlite, "BEGIN IMMEDIATE;") == 0 &&
                   restart_persistence_create_schema(sqlite) == 0 &&
                   restart_exec(sqlite,
                                "DELETE FROM restart_queue_env;"
                                "DELETE FROM restart_queue_scr;"
                                "DELETE FROM restart_queue_entries;"
                                "DELETE FROM restart_queue_objects;"
                                "DELETE FROM restart_descriptors;"
                                "DELETE FROM restart_metadata;") == 0 &&
                   sqlite3_prepare_v2(
                       sqlite,
                       "INSERT INTO restart_metadata "
                       "(id, format_version, created_at, start_time, doing_hdr, "
                       "record_players) VALUES (1, ?, ?, ?, ?, ?);",
                       -1, &metadata, NULL) == SQLITE_OK &&
                   restart_bind_long(metadata, 1, RESTART_SCHEMA_VERSION) == 0 &&
                   restart_bind_long(metadata, 2, time(NULL)) == 0 &&
                   restart_bind_long(metadata, 3, mudstate.start_time) == 0 &&
                   sqlite3_bind_text(metadata, 4, mudstate.doing_hdr, -1,
                                     SQLITE_TRANSIENT) == SQLITE_OK &&
                   restart_bind_long(metadata, 5, mudstate.record_players) == 0 &&
                   restart_step(metadata) == 0 && restart_store_descriptors(sqlite) == 0 &&
                   cque_restart_store(sqlite) == 0 && restart_exec(sqlite, "COMMIT;") == 0
               ? 0
               : -1;
  if (result < 0 && sqlite)
    restart_exec(sqlite, "ROLLBACK;");
  sqlite3_finalize(metadata);
  if (sqlite && sqlite3_close(sqlite) != SQLITE_OK)
    result = -1;
  return result;
}

static int restart_column_long(sqlite3_stmt *statement, int column, long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < LONG_MIN || number > LONG_MAX)
    return -1;
  *value = (long)number;
  return 0;
}

static int restart_column_text(sqlite3_stmt *statement, int column, char *target,
                               size_t target_size, int nullable) {
  const unsigned char *text;
  int length;

  if (sqlite3_column_type(statement, column) == SQLITE_NULL)
    return nullable ? 1 : -1;
  if (sqlite3_column_type(statement, column) != SQLITE_TEXT)
    return -1;
  text = sqlite3_column_text(statement, column);
  length = sqlite3_column_bytes(statement, column);
  if (!text || length < 0 || (size_t)length >= target_size ||
      (int)strlen((const char *)text) != length)
    return -1;
  memcpy(target, text, (size_t)length + 1);
  return 0;
}

static void restart_add_descriptor(DESC *descriptor) {
  if (descriptor_list)
    descriptor_list->prev = descriptor;
  descriptor->next = descriptor_list;
  descriptor->prev = NULL;
  descriptor_list = descriptor;
  descriptor->sock_buff = bufferevent_new(
      descriptor->descriptor, bsd_write_callback, bsd_read_callback,
      bsd_error_callback, NULL);
  bufferevent_disable(descriptor->sock_buff, EV_READ);
  bufferevent_enable(descriptor->sock_buff, EV_WRITE);
  event_set(&descriptor->sock_ev, descriptor->descriptor, EV_READ | EV_PERSIST,
            accept_client_input, descriptor);
  event_add(&descriptor->sock_ev, NULL);
  desc_addhash(descriptor);
  if (isPlayer(descriptor->player))
    s_Flags2(descriptor->player, Flags2(descriptor->player) | CONNECTED);
}

static int restart_load_descriptors(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  DESC *descriptor;
  DESC *next;
  long value;
  int result;
  int step;
  char prefix[LBUF_SIZE];
  char suffix[LBUF_SIZE];
  int has_prefix;
  int has_suffix;

  statement = NULL;
  result = sqlite3_prepare_v2(
               sqlite,
               "SELECT descriptor, flags, connected_at, command_count, timeout, "
               "host_info, player_dbref, last_time, output_prefix, output_suffix, "
               "addr, doing, username FROM restart_descriptors ORDER BY descriptor;",
               -1, &statement, NULL) == SQLITE_OK
               ? 0
               : -1;
  while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
    descriptor = calloc(1, sizeof(DESC));
    if (!descriptor || restart_column_long(statement, 0, &value) < 0 || value <= 0 ||
        value > INT_MAX) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->descriptor = value;
    if (restart_column_long(statement, 1, &value) < 0 || value < INT_MIN ||
        value > INT_MAX) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->flags = value;
    if (restart_column_long(statement, 2, &value) < 0) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->connected_at = value;
    if (restart_column_long(statement, 3, &value) < 0 || value < INT_MIN ||
        value > INT_MAX) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->command_count = value;
    if (restart_column_long(statement, 4, &value) < 0 || value < INT_MIN ||
        value > INT_MAX) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->timeout = value;
    if (restart_column_long(statement, 5, &value) < 0 || value < INT_MIN ||
        value > INT_MAX) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->host_info = value;
    if (restart_column_long(statement, 6, &value) < 0 ||
        (value != NOTHING && !isPlayer(value))) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->player = value;
    if (restart_column_long(statement, 7, &value) < 0) {
      free(descriptor);
      result = -1;
      break;
    }
    descriptor->last_time = value;
    has_prefix = restart_column_text(statement, 8, prefix, sizeof(prefix), 1);
    has_suffix = restart_column_text(statement, 9, suffix, sizeof(suffix), 1);
    if (has_prefix < 0 || has_suffix < 0 ||
        restart_column_text(statement, 10, descriptor->addr, sizeof(descriptor->addr),
                            0) < 0 ||
        restart_column_text(statement, 11, descriptor->doing, sizeof(descriptor->doing),
                            0) < 0 ||
        restart_column_text(statement, 12, descriptor->username,
                            sizeof(descriptor->username), 0) < 0) {
      free(descriptor);
      result = -1;
      break;
    }
    if (has_prefix == 0 &&
        !(descriptor->output_prefix = alloc_lbuf("restart.output_prefix"))) {
      free(descriptor);
      result = -1;
      break;
    }
    if (has_prefix == 0)
      strcpy(descriptor->output_prefix, prefix);
    if (has_suffix == 0 &&
        !(descriptor->output_suffix = alloc_lbuf("restart.output_suffix"))) {
      free_lbuf(descriptor->output_prefix);
      free(descriptor);
      result = -1;
      break;
    }
    if (has_suffix == 0)
      strcpy(descriptor->output_suffix, suffix);
    descriptor->quota = mudconf.cmd_quota_max;
    descriptor->refcount = 1;
    descriptor->saddr_len = sizeof(descriptor->saddr);
    getpeername(descriptor->descriptor, (struct sockaddr *)&descriptor->saddr,
                (socklen_t *)&descriptor->saddr_len);
    descriptor->outstanding_dnschild_query = dnschild_request(descriptor);
    restart_add_descriptor(descriptor);
  }
  if (result == 0 && step != SQLITE_DONE)
    result = -1;
  DESC_SAFEITER_ALL(descriptor, next) {
    struct stat status;

    if (!isPlayer(descriptor->player) ||
        fstat(descriptor->descriptor, &status) < 0)
      shutdownsock(descriptor, R_QUIT);
  }
  sqlite3_finalize(statement);
  return result;
}

int restart_persistence_load(void) {
  sqlite3 *sqlite;
  sqlite3_stmt *metadata;
  long version;
  long start_time;
  long record_players;
  char doing_hdr[sizeof(mudstate.doing_hdr)];
  int result;

  sqlite = NULL;
  metadata = NULL;
  result = restart_open(&sqlite) == 0 && restart_exec(sqlite, "PRAGMA foreign_keys = ON;") == 0 &&
                   restart_exec(sqlite, "BEGIN IMMEDIATE;") == 0 &&
                   sqlite3_prepare_v2(
                       sqlite,
                       "SELECT format_version, start_time, doing_hdr, record_players "
                       "FROM restart_metadata WHERE id = 1;",
                       -1, &metadata, NULL) == SQLITE_OK &&
                   sqlite3_step(metadata) == SQLITE_ROW &&
                   restart_column_long(metadata, 0, &version) == 0 &&
                   restart_column_long(metadata, 1, &start_time) == 0 &&
                   restart_column_text(metadata, 2, doing_hdr, sizeof(doing_hdr), 0) ==
                       0 &&
                   restart_column_long(metadata, 3, &record_players) == 0 &&
                   sqlite3_step(metadata) == SQLITE_DONE &&
                   version == RESTART_SCHEMA_VERSION && start_time >= 0 &&
                   record_players >= 0
               ? 0
               : -1;
  sqlite3_finalize(metadata);
  if (result == 0) {
    mudstate.start_time = start_time;
    mudstate.record_players = record_players;
    strcpy(mudstate.doing_hdr, doing_hdr);
    result = restart_load_descriptors(sqlite) == 0 && cque_restart_load(sqlite) == 0 &&
                     restart_exec(sqlite,
                                  "DELETE FROM restart_queue_env;"
                                  "DELETE FROM restart_queue_scr;"
                                  "DELETE FROM restart_queue_entries;"
                                  "DELETE FROM restart_queue_objects;"
                                  "DELETE FROM restart_descriptors;"
                                  "DELETE FROM restart_metadata;") == 0 &&
                     restart_exec(sqlite, "COMMIT;") == 0
                 ? 0
                 : -1;
  }
  if (result < 0 && sqlite)
    restart_exec(sqlite, "ROLLBACK;");
  if (sqlite && sqlite3_close(sqlite) != SQLITE_OK)
    result = -1;
  if (result == 0) {
    mudstate.restarting = 1;
    raw_broadcast(0, "Game: Restart finished.");
  }
  return result;
}

int restart_persistence_discard(void) {
  sqlite3 *sqlite;
  sqlite3_stmt *statement;
  sqlite3_int64 count;
  int result;

  sqlite = NULL;
  statement = NULL;
  count = 0;
  if (sqlite3_open_v2(mudconf.gamedb, &sqlite, SQLITE_OPEN_READONLY, NULL) !=
      SQLITE_OK) {
    if (sqlite)
      sqlite3_close(sqlite);
    return -1;
  }
  result = sqlite3_prepare_v2(sqlite,
                              "SELECT count(*) FROM restart_metadata;", -1,
                              &statement, NULL) == SQLITE_OK &&
                   sqlite3_step(statement) == SQLITE_ROW
               ? 0
               : -1;
  if (result == 0)
    count = sqlite3_column_int64(statement, 0);
  sqlite3_finalize(statement);
  sqlite3_close(sqlite);
  if (result < 0)
    return 0;
  if (count == 0)
    return 0;

  if (restart_open(&sqlite) < 0)
    return -1;
  result = restart_exec(sqlite, "BEGIN IMMEDIATE;") == 0 &&
                   restart_persistence_create_schema(sqlite) == 0 &&
                   restart_exec(sqlite,
                                "DELETE FROM restart_queue_env;"
                                "DELETE FROM restart_queue_scr;"
                                "DELETE FROM restart_queue_entries;"
                                "DELETE FROM restart_queue_objects;"
                                "DELETE FROM restart_descriptors;"
                                "DELETE FROM restart_metadata;") == 0 &&
                   restart_exec(sqlite, "COMMIT;") == 0
               ? 0
               : -1;
  if (result < 0)
    restart_exec(sqlite, "ROLLBACK;");
  if (sqlite3_close(sqlite) != SQLITE_OK)
    result = -1;
  return result;
}
