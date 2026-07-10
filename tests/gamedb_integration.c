/* gamedb_integration.c -- checkpoint-time SQLite mirror integration test */

#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static int run_server(const char *netmux, const char *config, int make_minimal,
                      int *status) {
  struct timespec delay;
  pid_t child;

  child = fork();
  if (child < 0)
    return -1;
  if (child == 0) {
    if (make_minimal)
      execl(netmux, netmux, "-s", config, NULL);
    else
      execl(netmux, netmux, config, NULL);
    _exit(127);
  }

  delay.tv_sec = 1;
  delay.tv_nsec = 0;
  nanosleep(&delay, NULL);
  if (kill(child, SIGTERM) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

static int query_int(sqlite3 *sqlite, const char *sql, sqlite3_int64 expected) {
  sqlite3_stmt *statement;
  int ok;

  statement = NULL;
  ok = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
       sqlite3_step(statement) == SQLITE_ROW &&
       sqlite3_column_int64(statement, 0) == expected;
  sqlite3_finalize(statement);
  return ok ? 0 : -1;
}

#ifdef BTMUX_TEST_ADVANCED_ECON
/* Recreate the prior empty index schema to verify the no-data transition. */
static int create_empty_obsolete_economy(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                   SQLITE_OK &&
               sqlite3_exec(sqlite,
                            "DROP TABLE btech_economy_costs;"
                            "CREATE TABLE btech_economy_costs ("
                            " category TEXT NOT NULL,"
                            " item_index INTEGER NOT NULL,"
                            " cost TEXT NOT NULL,"
                            " PRIMARY KEY (category, item_index)"
                            ") WITHOUT ROWID;",
                            NULL, NULL, NULL) == SQLITE_OK
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Populate the obsolete schema to verify that no price migration is retained. */
static int create_nonempty_obsolete_economy(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                   SQLITE_OK &&
               sqlite3_exec(sqlite,
                            "DROP TABLE btech_economy_costs;"
                            "CREATE TABLE btech_economy_costs ("
                            " category TEXT NOT NULL,"
                            " item_index INTEGER NOT NULL,"
                            " cost TEXT NOT NULL,"
                            " PRIMARY KEY (category, item_index)"
                            ") WITHOUT ROWID;"
                            "INSERT INTO btech_economy_costs "
                            "(category, item_index, cost) "
                            "VALUES ('weapon', 0, '987');",
                            NULL, NULL, NULL) == SQLITE_OK
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Add one non-default row to verify sparse data is loaded and preserved. */
static int insert_sparse_economy_cost(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                   SQLITE_OK &&
               sqlite3_exec(sqlite,
                            "INSERT INTO btech_economy_costs "
                            "(item_name, cost) VALUES ('CL.A-Pod', '987');",
                            NULL, NULL, NULL) == SQLITE_OK
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Verify that one sparse row remained after a full SQLite reload and dump. */
static int check_sparse_economy_cost(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result = query_int(sqlite, "SELECT count(*) FROM btech_economy_costs;", 1) ==
               0 &&
           query_int(sqlite,
                     "SELECT CAST(cost AS INTEGER) FROM btech_economy_costs "
                     "WHERE item_name = 'CL.A-Pod';",
                     987) == 0
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Confirm that a newly created minimal game has zero-initialized prices. */
static int check_zero_economy(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result = query_int(sqlite, "SELECT count(*) FROM btech_economy_costs;", 0) ==
                   0 &&
               query_int(sqlite,
                         "SELECT count(*) FROM pragma_table_info("
                         "'btech_economy_costs') WHERE name = 'item_name';",
                         1) == 0 &&
               query_int(
                   sqlite,
                   "SELECT count(*) FROM btech_economy_costs WHERE cost != '0';",
                   0) == 0
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}
#endif

static int check_snapshot(const char *path) {
  sqlite3 *sqlite;
  int ok;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  ok = query_int(
           sqlite,
           "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name "
           "IN ('snapshot', 'vattrs', 'objects', 'attributes');",
           4) == 0 &&
      query_int(sqlite,
                 "SELECT schema_version FROM snapshot WHERE id = 1;", 2) ==
           0 &&
      query_int(sqlite,
                 "SELECT storage_format FROM snapshot WHERE id = 1;", 1) ==
           0 &&
       query_int(sqlite, "SELECT dump_type FROM snapshot WHERE id = 1;", 0) ==
           0 &&
       query_int(sqlite, "SELECT count(*) FROM objects;", 2) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM attributes WHERE number IN (25, 42, 43);",
                 0) == 0;
#ifdef BTMUX_TEST_ADVANCED_ECON
  ok = ok &&
       query_int(sqlite,
                 "SELECT count(*) FROM sqlite_master WHERE type = 'table' "
                 "AND name = 'btech_economy_costs';",
                 1) == 0;
#endif
  sqlite3_close(sqlite);
  return ok ? 0 : -1;
}

int main(int argc, char *argv[]) {
  char directory[] = "/tmp/btmux-gamedb-test.XXXXXX";
  char config[PATH_MAX];
  char missing_config[PATH_MAX];
  char sqlite_directory[PATH_MAX];
  char database[PATH_MAX];
  FILE *file;
  int status;
  int result;

  if (argc != 2 || !mkdtemp(directory))
    return 2;
  if (snprintf(config, sizeof(config), "%s/game.conf", directory) < 0 ||
      snprintf(missing_config, sizeof(missing_config), "%s/missing.conf",
               directory) < 0 ||
      snprintf(sqlite_directory, sizeof(sqlite_directory), "%s/sqlite",
               directory) < 0 ||
      snprintf(database, sizeof(database), "%s/sqlite/game.sqlite", directory) <
          0 ||
      mkdir(sqlite_directory, 0700) < 0)
    return 2;

  file = fopen(config, "w");
  if (!file)
    return 2;
  fprintf(file, "game_database %s\n", database);
  fprintf(file, "have_specials 0\n");
  fprintf(file, "port 0\n");
  if (fclose(file) != 0)
    return 2;

  result = run_server(argv[1], config, 1, &status) == 0 && WIFEXITED(status) &&
           WEXITSTATUS(status) == 0 && check_snapshot(database) == 0
#ifdef BTMUX_TEST_ADVANCED_ECON
           && check_zero_economy(database) == 0
#endif
               ? 0
               : 1;

#ifdef BTMUX_TEST_ADVANCED_ECON
  if (result == 0 &&
      (run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_zero_economy(database) < 0))
    result = 1;

  if (result == 0 &&
      (insert_sparse_economy_cost(database) < 0 ||
       run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_sparse_economy_cost(database) < 0))
    result = 1;
#else
  if (result == 0 &&
      (run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0))
    result = 1;
#endif

#ifdef BTMUX_TEST_ADVANCED_ECON
  if (result == 0 &&
      (create_empty_obsolete_economy(database) < 0 ||
       run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_zero_economy(database) < 0))
    result = 1;

  if (result == 0 &&
      (create_nonempty_obsolete_economy(database) < 0 ||
       run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) != 2))
    result = 1;
#endif

  file = fopen(missing_config, "w");
  if (!file)
    return 2;
  fprintf(file, "port 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 &&
      (run_server(argv[1], missing_config, 1, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) != 2))
    result = 1;

  unlink(config);
  unlink(missing_config);
  unlink(database);
  rmdir(sqlite_directory);
  rmdir(directory);
  return result;
}
