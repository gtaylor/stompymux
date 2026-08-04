/* schema.c - Explicit destructive reset for BTech-owned SQLite tables. */

#include "btech/persistence.h"

#include <sqlite3.h>
#include <stdlib.h>
#include <string.h>

typedef struct BtechTableNames {
  char **items;
  size_t count;
} BtechTableNames;

static void btech_table_names_destroy(BtechTableNames *names) {
  for (size_t index = 0; index < names->count; index++)
    free(names->items[index]);
  free(names->items);
  *names = (BtechTableNames){0};
}

static int btech_table_names_load(sqlite3 *database, BtechTableNames *names) {
  sqlite3_stmt *statement = nullptr;
  if (sqlite3_prepare_v2(database,
                         "SELECT name FROM sqlite_schema "
                         "WHERE type = 'table' AND name GLOB 'btech_*' "
                         "AND name <> 'btech_object_state' "
                         "ORDER BY name;",
                         -1, &statement, nullptr) != SQLITE_OK)
    return -1;

  int result = 0;
  int step;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    const unsigned char *value = sqlite3_column_text(statement, 0);
    if (value == nullptr) {
      result = -1;
      break;
    }
    char **items = realloc(names->items, (names->count + 1) * sizeof(*items));
    if (items == nullptr) {
      result = -1;
      break;
    }
    names->items = items;
    names->items[names->count] = strdup((const char *)value);
    if (names->items[names->count] == nullptr) {
      result = -1;
      break;
    }
    names->count++;
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

static int btech_tables_drop(sqlite3 *database, const BtechTableNames *names) {
  for (size_t index = 0; index < names->count; index++) {
    char *sql =
        sqlite3_mprintf("DROP TABLE IF EXISTS \"%w\";", names->items[index]);
    if (sql == nullptr)
      return -1;
    int status = sqlite3_exec(database, sql, nullptr, nullptr, nullptr);
    sqlite3_free(sql);
    if (status != SQLITE_OK)
      return -1;
  }
  return 0;
}

int btech_persistence_reset_schema_path(const char *path) {
  if (path == nullptr || path[0] == '\0')
    return -1;

  sqlite3 *database = nullptr;
  BtechTableNames names = {0};
  int result = -1;
  if (sqlite3_open_v2(path, &database, SQLITE_OPEN_READWRITE, nullptr) !=
          SQLITE_OK ||
      btech_table_names_load(database, &names) < 0 ||
      sqlite3_exec(database, "BEGIN IMMEDIATE;", nullptr, nullptr, nullptr) !=
          SQLITE_OK)
    goto done;

  if (btech_tables_drop(database, &names) == 0 &&
      sqlite3_exec(database, "COMMIT;", nullptr, nullptr, nullptr) == SQLITE_OK)
    result = 0;
  else
    sqlite3_exec(database, "ROLLBACK;", nullptr, nullptr, nullptr);

done:
  btech_table_names_destroy(&names);
  if (database != nullptr)
    sqlite3_close(database);
  return result;
}
