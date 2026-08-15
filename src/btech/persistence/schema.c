/* schema.c - Explicit destructive reset for BTech-owned SQLite tables. */

#include "btech/persistence.h"

#include <sqlite3.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"

typedef struct BtechTableNames {
  char **items;
  size_t count;
} BtechTableNames;

static char **btech_table_name_slot(char **items, size_t count, size_t index) {
  return (char **)checked_storage_at((void *)items, count, sizeof(*items),
                                     index);
}

static char *btech_table_name(const BtechTableNames *names, size_t index) {
  char *const *slot = (char *const *)checked_storage_at_const(
      (const void *)names->items, names->count, sizeof(*names->items), index);
  return *slot;
}

static void btech_table_names_destroy(BtechTableNames *names) {
  for (size_t index = 0; index < names->count; index++)
    free(btech_table_name(names, index));
  free((void *)names->items);
  *names = (BtechTableNames){};
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
    if (names->count == SIZE_MAX) {
      result = -1;
      break;
    }
    char **items = (char **)checked_storage_try_reallocate_array(
        (void *)names->items, names->count + 1, sizeof(*items));
    if (items == nullptr) {
      result = -1;
      break;
    }
    names->items = items;
    char **slot =
        btech_table_name_slot(names->items, names->count + 1, names->count);
    *slot = strdup((const char *)value);
    if (*slot == nullptr) {
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
    char *sql = sqlite3_mprintf("DROP TABLE IF EXISTS \"%w\";",
                                btech_table_name(names, index));
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
  BtechTableNames names = {};
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
