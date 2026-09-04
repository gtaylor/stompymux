#include "btech/persistence.h"

#include <sqlite3.h>
#include <stdlib.h>
#include <unistd.h>

static int table_count(sqlite3 *database, const char *pattern) {
  sqlite3_stmt *statement = nullptr;
  int count = -1;
  if (sqlite3_prepare_v2(database,
                         "SELECT count(*) FROM sqlite_schema "
                         "WHERE type = 'table' AND name GLOB ?;",
                         -1, &statement, nullptr) == SQLITE_OK &&
      sqlite3_bind_text(statement, 1, pattern, -1, SQLITE_STATIC) ==
          SQLITE_OK &&
      sqlite3_step(statement) == SQLITE_ROW)
    count = sqlite3_column_int(statement, 0);
  sqlite3_finalize(statement);
  return count;
}

int main(void) {
  char path[] = "/tmp/btech-schema-test-XXXXXX";
  int descriptor = mkstemp(path);
  if (descriptor < 0)
    return 1;
  close(descriptor);

  sqlite3 *database = nullptr;
  int result = 1;
  if (sqlite3_open(path, &database) != SQLITE_OK ||
      sqlite3_exec(database,
                   "CREATE TABLE btech_old_state (id INTEGER);"
                   "CREATE TABLE btech_object_state (id INTEGER);"
                   "CREATE TABLE btech_character_state (id INTEGER);"
                   "CREATE TABLE btech_character_values (id INTEGER);"
                   "CREATE TABLE btech_economy_parts (id INTEGER);"
                   "CREATE TABLE unrelated_state (id INTEGER);",
                   nullptr, nullptr, nullptr) != SQLITE_OK)
    goto done;
  sqlite3_close(database);
  database = nullptr;

  if (btech_persistence_reset_schema_path(path) < 0 ||
      sqlite3_open(path, &database) != SQLITE_OK ||
      table_count(database, "btech_old_state") != 0 ||
      table_count(database, "btech_object_state") != 0 ||
      table_count(database, "btech_character_state") != 1 ||
      table_count(database, "btech_character_values") != 1 ||
      table_count(database, "btech_economy_parts") != 1 ||
      table_count(database, "unrelated_state") != 1)
    goto done;
  result = 0;

done:
  if (database != nullptr)
    sqlite3_close(database);
  unlink(path);
  return result;
}
