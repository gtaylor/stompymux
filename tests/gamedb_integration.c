/* gamedb_integration.c -- checkpoint-time SQLite mirror integration test */

#include <errno.h>
#include <limits.h>
#include <poll.h>
#include <signal.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static int run_server(const char *binary_path, const char *config,
                      int make_minimal, int *status) {
  struct timespec delay;
  pid_t child;

  child = fork();
  if (child < 0)
    return -1;
  if (child == 0) {
    if (make_minimal)
      execl(binary_path, binary_path, "-s", config, NULL);
    else
      execl(binary_path, binary_path, config, NULL);
    _exit(127);
  }

  delay.tv_sec = 1;
  delay.tv_nsec = 0;
  nanosleep(&delay, NULL);
  if (kill(child, SIGTERM) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

/* Run an isolated server instance so legacy dual-write artifacts stay
 * disposable. */
static int run_server_in_directory_for(const char *binary_path,
                                       const char *config,
                                       const char *directory, int make_minimal,
                                       time_t seconds, int *status) {
  struct timespec delay;
  pid_t child;

  child = fork();
  if (child < 0)
    return -1;
  if (child == 0) {
    if (chdir(directory) < 0)
      _exit(127);
    if (make_minimal)
      execl(binary_path, binary_path, "-s", config, NULL);
    else
      execl(binary_path, binary_path, config, NULL);
    _exit(127);
  }
  delay.tv_sec = seconds;
  delay.tv_nsec = 0;
  nanosleep(&delay, NULL);
  if (kill(child, SIGTERM) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

/* Start a child that has initialized its event loop but has not run a timer. */
static pid_t start_server_in_directory_after(const char *binary_path,
                                             const char *config,
                                             const char *directory,
                                             int make_minimal, int *status) {
  char ready_fd[32];
  char ready_signal;
  int ready_pipe[2];
  struct pollfd ready;
  pid_t child;

  if (pipe(ready_pipe) < 0)
    return -1;
  child = fork();
  if (child < 0) {
    close(ready_pipe[0]);
    close(ready_pipe[1]);
    return -1;
  }
  if (child == 0) {
    close(ready_pipe[0]);
    snprintf(ready_fd, sizeof(ready_fd), "%d", ready_pipe[1]);
    if (chdir(directory) < 0 || setenv("BTMUX_TEST_READY_FD", ready_fd, 1) < 0)
      _exit(127);
    if (make_minimal)
      execl(binary_path, binary_path, "-s", config, NULL);
    else
      execl(binary_path, binary_path, config, NULL);
    _exit(127);
  }
  close(ready_pipe[1]);
  ready.fd = ready_pipe[0];
  ready.events = POLLIN;
  ready.revents = 0;
  if (poll(&ready, 1, 5000) != 1 || !(ready.revents & POLLIN) ||
      read(ready_pipe[0], &ready_signal, sizeof(ready_signal)) !=
          sizeof(ready_signal)) {
    close(ready_pipe[0]);
    kill(child, SIGKILL);
    waitpid(child, status, 0);
    return -1;
  }
  close(ready_pipe[0]);
  return child;
}

/* Wait for the child to enter its event loop before sending its test signal. */
static int run_server_in_directory_after(const char *binary_path,
                                         const char *config,
                                         const char *directory,
                                         int make_minimal, int *status) {
  pid_t child;

  child = start_server_in_directory_after(binary_path, config, directory,
                                          make_minimal, status);
  if (child < 0)
    return -1;
  if (kill(child, SIGTERM) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

/* Start an isolated server long enough for normal startup work, then stop it.
 */
static int run_server_in_directory(const char *binary_path, const char *config,
                                   const char *directory, int make_minimal,
                                   int *status) {
  return run_server_in_directory_for(binary_path, config, directory,
                                     make_minimal, 1, status);
}

/* Trigger the fatal-signal crash dump without attempting process recovery. */
static int run_server_crash_in_directory(const char *binary_path,
                                         const char *config,
                                         const char *directory, int *status) {
  struct timespec delay;
  pid_t child;

  child = start_server_in_directory_after(binary_path, config, directory, 0,
                                          status);
  if (child < 0)
    return -1;
  if (kill(child, SIGBUS) < 0)
    return -1;
  delay.tv_sec = 1;
  delay.tv_nsec = 0;
  nanosleep(&delay, NULL);
  if (kill(child, SIGKILL) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

/* Exercise SIGUSR2's intentional DUMP_KILLED shutdown path. */
static int run_server_killed_in_directory(const char *binary_path,
                                          const char *config,
                                          const char *directory, int *status) {
  struct timespec delay;
  pid_t child;
  pid_t waited;
  int attempt;

  child = start_server_in_directory_after(binary_path, config, directory, 0,
                                          status);
  if (child < 0)
    return -1;
  if (kill(child, SIGUSR2) < 0)
    return -1;

  delay.tv_sec = 0;
  delay.tv_nsec = 100000000;
  for (attempt = 0; attempt < 20; attempt++) {
    waited = waitpid(child, status, WNOHANG);
    if (waited == child)
      return 0;
    if (waited < 0)
      return -1;
    nanosleep(&delay, NULL);
  }
  kill(child, SIGKILL);
  waitpid(child, status, 0);
  return -1;
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

static int query_text(sqlite3 *sqlite, const char *sql, const char *expected) {
  sqlite3_stmt *statement;
  const unsigned char *actual;
  int ok;

  statement = NULL;
  ok = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
       sqlite3_step(statement) == SQLITE_ROW;
  actual = ok ? sqlite3_column_text(statement, 0) : NULL;
  ok = ok && actual != NULL && !strcmp((const char *)actual, expected);
  sqlite3_finalize(statement);
  return ok ? 0 : -1;
}

static int check_minimal_lua_parents(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result = query_text(sqlite,
                      "SELECT lua_parent FROM object_state "
                      "WHERE object_dbref = 0;",
                      "room.lua") == 0 &&
                   query_text(sqlite,
                              "SELECT lua_parent FROM object_state "
                              "WHERE object_dbref = 1;",
                              "player.lua") == 0
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Identify the first non-default BTech value that failed to round-trip. */
static int check_btech_value(sqlite3 *sqlite, const char *label,
                             const char *sql, sqlite3_int64 expected) {
  sqlite3_stmt *statement;
  sqlite3_int64 actual;
  int result;

  statement = NULL;
  actual = 0;
  result = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
                   sqlite3_step(statement) == SQLITE_ROW
               ? 0
               : -1;
  if (result == 0) {
    actual = sqlite3_column_int64(statement, 0);
    if (actual != expected)
      result = -1;
  }
  if (result < 0)
    fprintf(stderr,
            "BTech SQLite round-trip mismatch for %s: expected %lld, got %lld "
            "(%s)\n",
            label, (long long)expected, (long long)actual,
            sqlite3_errmsg(sqlite));
  sqlite3_finalize(statement);
  return result;
}

#ifdef BTMUX_TEST_ADVANCED_ECON
/* Remove the required extension table to verify strict schema validation. */
static int drop_sqlite_economy(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                       SQLITE_OK &&
                   sqlite3_exec(sqlite, "DROP TABLE btech_economy_costs;", NULL,
                                NULL, NULL) == SQLITE_OK
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
  result =
      query_int(sqlite, "SELECT count(*) FROM btech_economy_costs;", 1) == 0 &&
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
  result =
      query_int(sqlite, "SELECT count(*) FROM btech_economy_costs;", 0) == 0 &&
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
           "IN ('snapshot', 'objects', 'object_state', 'player_state', "
           "'btech_object_state', 'attributes');",
           6) == 0 &&
       query_int(sqlite, "SELECT schema_version FROM snapshot WHERE id = 1;",
                 13) == 0 &&
       query_int(sqlite, "SELECT storage_format FROM snapshot WHERE id = 1;",
                 1) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('object_state') "
                 "WHERE name = 'semaphore_count';",
                 0) == 0 &&
       query_int(sqlite, "SELECT dump_type FROM snapshot WHERE id = 1;", 0) ==
           0 &&
       (query_int(sqlite, "SELECT count(*) FROM objects;", 2) == 0 ||
        query_int(sqlite, "SELECT count(*) FROM objects;", 7) == 0) &&
       (query_int(sqlite, "SELECT count(*) FROM attributes;", 0) == 0 ||
        query_int(sqlite, "SELECT count(*) FROM attributes;", 2) == 0) &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('attributes') "
                 "WHERE name IN ('number', 'flags', 'owner');",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') "
                 "WHERE name = 'owner';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name IN ('has_idle_power', 'has_long_fingers_power', "
                 "'has_comm_all_power', "
                 "'has_see_hidden_power', 'has_no_destroy_power', "
                 "'has_mech_power', 'has_security_power', 'has_mechrep_power', "
                 "'has_map_power', "
                 "'has_template_power', 'has_tech_power');",
                 11) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name IN ('powers', 'powers2');",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name = 'has_pass_locks_power';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name IN ('flags', 'flags2', 'flags3');",
                 0) == 0 &&
       query_int(
           sqlite,
           "SELECT count(*) FROM pragma_table_info('objects') WHERE "
           "name IN ('type', 'has_ansi_flag', 'has_ansimap_flag', "
           "'has_audible_flag', 'has_auditorium_flag', 'has_blind_flag', "
           "'has_connected_flag', 'has_dark_flag', 'has_floating_flag', "
           "'has_gagged_flag', 'has_going_flag', 'has_halted_flag', "
           "'has_in_character_flag', 'has_light_flag', 'has_monitor_flag', "
           "'has_no_command_flag', 'has_quiet_flag', "
           "'has_safe_flag', 'has_suspect_flag', 'has_transparent_flag', "
           "'has_wizard_flag', 'has_xcode_flag', 'has_zombie_flag');",
           23) == 0 &&
       query_int(
           sqlite,
           "SELECT count(*) FROM objects WHERE has_idle_power NOT IN (0, 1) "
           "OR has_long_fingers_power NOT IN (0, 1) OR has_comm_all_power NOT "
           "IN "
           "(0, 1) OR has_see_hidden_power NOT IN (0, 1) OR "
           "has_no_destroy_power "
           "NOT IN (0, 1) OR has_mech_power "
           "NOT IN (0, 1) OR has_security_power NOT IN (0, 1) OR "
           "has_mechrep_power "
           "NOT IN (0, 1) OR has_map_power NOT IN (0, 1) OR has_template_power "
           "NOT "
           "IN (0, 1) OR has_tech_power NOT IN (0, 1);",
           0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM sqlite_master WHERE name = 'vattrs';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('snapshot') WHERE "
                 "name = 'attr_next';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name = 'lock_expr';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                 "name = 'parent';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info("
                 "'btech_object_state') WHERE name = 'mech_status';",
                 0) == 0 &&
       query_int(
           sqlite,
           "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND "
           "name IN ('commac_entries', 'commac_aliases', 'comsys_channels', "
           "'comsys_channel_users', 'comsys_channel_messages', 'macro_sets', "
           "'macro_entries');",
           7) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info("
                 "'comsys_channel_users') WHERE name = 'title';",
                 0) == 0 &&
       query_int(sqlite,
                 "SELECT count(*) FROM pragma_table_info("
                 "'comsys_channels') WHERE name IN "
                 "('temp1', 'temp2', 'charge', 'charge_who', 'amount_col');",
                 0) == 0;
  ok = ok &&
       query_int(
           sqlite,
           "SELECT count(*) FROM sqlite_master WHERE type = 'table' "
           "AND name IN ('btech_persistence_metadata', 'btech_maps', "
           "'btech_map_hexes', 'btech_map_slots', "
           "'btech_map_los', 'btech_map_objects', 'btech_map_bits', "
           "'btech_repair_events', 'btech_mechrep', 'btech_turrets', "
           "'btech_turret_tics', 'btech_autopilots', "
           "'btech_autopilot_commands', 'btech_autopilot_command_args', "
           "'btech_autopilot_path', 'btech_mechs', 'btech_mech_sections', "
           "'btech_mech_criticals', 'btech_mech_positions', 'btech_mech_bays', "
           "'btech_mech_turrets', 'btech_mech_c3', 'btech_mech_c3_nodes', "
           "'btech_mech_tics', 'btech_mech_frequencies', 'btech_mech_runtime', "
           "'btech_mech_runtime_unused', 'btech_mech_unit_aux', "
           "'btech_mech_stagger_damage');",
           29) == 0;
  ok = ok && query_int(sqlite,
                       "SELECT schema_version FROM btech_persistence_metadata "
                       "WHERE id = 1;",
                       1) == 0;
#ifdef BTMUX_TEST_ADVANCED_ECON
  ok =
      ok && query_int(sqlite,
                      "SELECT count(*) FROM sqlite_master WHERE type = 'table' "
                      "AND name = 'btech_economy_costs';",
                      1) == 0;
#endif
  sqlite3_close(sqlite);
  return ok ? 0 : -1;
}

/* Read a checkpoint type independently of the normal-dump assertions. */
static int check_snapshot_dump_type(const char *path, sqlite3_int64 dump_type) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result = query_int(sqlite, "SELECT dump_type FROM snapshot WHERE id = 1;",
                     dump_type);
  sqlite3_close(sqlite);
  return result;
}

/* A failed named BTech writer must leave the completed SQLite file untouched.
 */
static int check_btech_writer_fault(const char *binary_path, const char *config,
                                    const char *directory, const char *database,
                                    const char *table, const char *phase) {
  struct stat before;
  struct stat after;
  int status;
  int result;

  if (stat(database, &before) < 0 ||
      setenv("BTMUX_TEST_BTECH_FAIL_TABLE", table, 1) < 0 ||
      setenv("BTMUX_TEST_BTECH_FAIL_PHASE", phase, 1) < 0)
    return -1;
  result =
      run_server_in_directory_after(binary_path, config, directory, 0, &status);
  unsetenv("BTMUX_TEST_BTECH_FAIL_TABLE");
  unsetenv("BTMUX_TEST_BTECH_FAIL_PHASE");
  if (result < 0 || !WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
      stat(database, &after) < 0 || before.st_dev != after.st_dev ||
      before.st_ino != after.st_ino || before.st_size != after.st_size)
    return -1;
  return 0;
}

/* Every table has a dedicated writer statement and receives both fault modes.
 */
static const char *const btech_special_writer_tables[] = {
    "btech_persistence_metadata",
    "btech_maps",
    "btech_map_hexes",
    "btech_map_slots",
    "btech_map_los",
    "btech_map_objects",
    "btech_map_bits",
    "btech_repair_events",
    "btech_mechrep",
    "btech_turrets",
    "btech_turret_tics",
    "btech_autopilots",
    "btech_mechs",
    "btech_mech_sections",
    "btech_mech_criticals",
    "btech_mech_positions",
    "btech_mech_bays",
    "btech_mech_turrets",
    "btech_mech_c3",
    "btech_mech_c3_nodes",
    "btech_mech_tics",
    "btech_mech_frequencies",
    "btech_mech_runtime",
    "btech_mech_runtime_unused",
    "btech_mech_unit_aux",
    "btech_mech_stagger_damage",
    "btech_autopilot_commands",
    "btech_autopilot_command_args",
    "btech_autopilot_path"};

/* Seed SQLite directly, then verify a fresh server process reads these rows. */
static int seed_commac_snapshot(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "INSERT INTO commac_entries VALUES (1, 0, 0, -1, -1, -1, -1);"
                  "INSERT INTO commac_aliases VALUES (1, 0, 'test', 'Public');"
                  "INSERT INTO comsys_channels VALUES ('Public', 0, 0, 0);"
                  "INSERT INTO comsys_channel_users VALUES ('Public', 0, 1, 1);"
                  "INSERT INTO comsys_channel_messages VALUES ('Public', 0, "
                  "123, 'test message');"
                  "INSERT INTO macro_sets VALUES (0, 1, 0, 'Test macros');"
                  "INSERT INTO macro_entries VALUES (0, 0, 'go', 'look');",
                  NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

static int check_commac_snapshot(const char *path) {
  sqlite3 *sqlite;
  int ok;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  ok =
      query_int(sqlite, "SELECT count(*) FROM commac_entries;", 1) == 0 &&
      query_int(sqlite, "SELECT count(*) FROM commac_aliases;", 1) == 0 &&
      query_int(sqlite, "SELECT count(*) FROM comsys_channels;", 1) == 0 &&
      query_int(sqlite, "SELECT count(*) FROM comsys_channel_users;", 1) == 0 &&
      query_int(sqlite, "SELECT count(*) FROM comsys_channel_messages;", 1) ==
          0 &&
      query_int(sqlite, "SELECT count(*) FROM macro_sets;", 1) == 0 &&
      query_int(sqlite, "SELECT count(*) FROM macro_entries;", 1) == 0;
  sqlite3_close(sqlite);
  return ok ? 0 : -1;
}

/* Turn a current snapshot into the previous schema and seed data that must be
 * discarded or scrubbed during the hard cutover to Lua behavior. */

/* Seed one core object for every BTech persisted special-object type. */
static int seed_btech_special_objects(const char *path) {
  sqlite3 *sqlite;
  char *error;
  int result;

  sqlite = NULL;
  error = NULL;
  result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "UPDATE snapshot SET db_top = 7 WHERE id = 1;"
                  "INSERT INTO objects "
                  "(dbref, name, location, contents, exits, next, link, zone, "
                  "type, has_xcode_flag) VALUES "
                  "(2, 'Test map', -1, -1, -1, -1, -1, -1, 1, 1),"
                  "(3, 'Test mech', -1, -1, -1, -1, -1, -1, 1, 1),"
                  "(4, 'Test repair', -1, -1, -1, -1, -1, -1, 1, 1),"
                  "(5, 'Test autopilot', -1, -1, -1, -1, -1, -1, 1, 1),"
                  "(6, 'Test turret', -1, -1, -1, -1, -1, -1, 1, 1);"
                  "INSERT INTO object_state (object_dbref) VALUES "
                  "(2),(3),(4),(5),(6);"
                  "INSERT INTO player_state (object_dbref) VALUES "
                  "(2),(3),(4),(5),(6);"
                  "INSERT INTO btech_object_state (object_dbref, object_type) "
                  "VALUES "
                  "(2, 'MAP'), (3, 'MECH'), (4, 'MECHREP'),"
                  "(5, 'AUTOPILOT'), (6, 'TURRET');"
                  "INSERT INTO attributes VALUES "
                  "(2, 'CaseKey', 'upper'), (2, 'casekey', 'lower');"
                  "UPDATE objects SET contents = 2 WHERE dbref = 1;"
                  "UPDATE objects SET location = 1, next = 3 WHERE dbref = 2;"
                  "UPDATE objects SET location = 1, next = 4 WHERE dbref = 3;"
                  "UPDATE objects SET location = 1, next = 5 WHERE dbref = 4;"
                  "UPDATE objects SET location = 1, next = 6 WHERE dbref = 5;"
                  "UPDATE objects SET location = 1, next = -1 WHERE dbref = 6;",
                  NULL, NULL, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "BTech fixture seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

/* Verify representative top-level and fixed-size child rows after a dump. */
static int check_btech_special_snapshot(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      query_int(sqlite, "SELECT count(*) FROM btech_maps;", 1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mechs;", 1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mechrep;", 1) ==
                  0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_autopilots;", 1) ==
                  0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_turrets;", 1) ==
                  0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM attributes WHERE object_dbref=2 "
                        "AND ((name='CaseKey' AND value='upper') OR "
                        "(name='casekey' AND value='lower'));",
                        2) == 0 &&
              query_int(
                  sqlite,
                  "SELECT count(*) = (SELECT width * height FROM btech_maps "
                  "WHERE dbref = 2) FROM btech_map_hexes;",
                  1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mech_sections;",
                        8) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mech_criticals;",
                        96) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mech_runtime;",
                        1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mech_tics;", 12) ==
                  0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mech_frequencies;",
                        16) == 0
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

/* A missing fixed MECH child row must make the SQLite-only startup fail. */
static int remove_btech_runtime_row(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                       SQLITE_OK &&
                   sqlite3_exec(
                       sqlite,
                       "DELETE FROM btech_mech_runtime WHERE mech_dbref = 3;",
                       NULL, NULL, NULL) == SQLITE_OK
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Bypass the column constraint to exercise the strict boolean loader. */
static int set_invalid_power_value(const char *path, int value) {
  sqlite3 *sqlite;
  char statement[160];
  int result;

  sqlite = NULL;
  if (snprintf(statement, sizeof(statement),
               "PRAGMA ignore_check_constraints = ON;"
               "UPDATE objects SET has_idle_power = %d WHERE dbref = 2;",
               value) < 0)
    return -1;
  result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite, statement, NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Make selected persisted fields non-default before the SQLite reload. */
static int seed_btech_nondefault_state(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "UPDATE btech_maps SET temperature = 17, regen_factor = 7 "
                  "WHERE dbref = 2;"
                  "UPDATE btech_map_hexes SET value = 42 "
                  "WHERE map_dbref = 2 AND x = 0 AND y = 0;"
                  "UPDATE btech_mech_sections SET armor = 19 "
                  "WHERE mech_dbref = 3 AND section = 0;"
                  "UPDATE btech_mech_criticals SET data = 3, fire_mode = 64 "
                  "WHERE mech_dbref = 3 AND section = 0 AND slot = 0;"
                  "UPDATE btech_mech_positions SET x = 2, y = 3, team = 9 "
                  "WHERE mech_dbref = 3;"
                  "UPDATE btech_mech_runtime SET heat = 12.5, status = 8, "
                  "last_used = 77, autopilot_num = 5 WHERE mech_dbref = 3;"
                  "UPDATE objects SET has_idle_power = 1, "
                  "has_long_fingers_power = 0, "
                  "has_comm_all_power = 1, has_see_hidden_power = 0, "
                  "has_no_destroy_power = 1, has_mech_power = 1, "
                  "has_security_power = 0, "
                  "has_mechrep_power = 1, has_map_power = 0, "
                  "has_template_power = 1, "
                  "has_tech_power = 0 WHERE dbref = 2;"
                  "UPDATE objects SET contents = 5 WHERE dbref = 3;"
                  "UPDATE objects SET next = 6 WHERE dbref = 4;"
                  "UPDATE objects SET location = 3, next = -1 WHERE dbref = 5;"
                  "UPDATE btech_maps SET first_free = 1 WHERE dbref = 2;"
                  "INSERT INTO btech_map_slots VALUES (2, 0, 3, 1);"
                  "INSERT INTO btech_map_los VALUES (2, 0, 0, 8193);"
                  "INSERT INTO btech_map_objects VALUES (2, 9, 0, 1, 1, -1, 4, "
                  "5, 6);"
                  "WITH RECURSIVE bytes(n) AS ("
                  " SELECT 0 UNION ALL SELECT n + 1 FROM bytes WHERE n + 1 < "
                  " (SELECT (width + 3) / 4 FROM btech_maps WHERE dbref = 2)"
                  ") INSERT INTO btech_map_bits "
                  "SELECT 2, 0, n, n + 10 FROM bytes;"
                  "UPDATE btech_mechs SET map_number = 0, map_dbref = 2 "
                  "WHERE dbref = 3;"
                  "UPDATE btech_mech_c3 SET channel_title = 'C3 test', "
                  "c3i_size = 1, "
                  "c3_size = 1, total_masters = 1, working_masters = 1, "
                  "frequency_mode = 2 WHERE mech_dbref = 3;"
                  "UPDATE btech_mech_c3_nodes SET node_dbref = 3 "
                  "WHERE mech_dbref = 3 AND network_type = 0 AND node_index = "
                  "0;"
                  "UPDATE btech_mech_c3_nodes SET node_dbref = 3 "
                  "WHERE mech_dbref = 3 AND network_type = 1 AND node_index = "
                  "0;"
                  "UPDATE btech_mech_tics SET value = 12345 "
                  "WHERE mech_dbref = 3 AND tic_index = 0 AND word_index = 0;"
                  "UPDATE btech_mech_frequencies SET frequency = 42, mode = 3, "
                  "title = 'test frequency' WHERE mech_dbref = 3 AND "
                  "frequency_index = 0;"
                  "INSERT INTO btech_mech_stagger_damage "
                  "VALUES (3, 0, 17, CAST(strftime('%s', 'now') AS INTEGER), "
                  "3, 1);"
                  "UPDATE btech_turrets SET arcs = 5, target_x = 2, target_y = "
                  "3 "
                  "WHERE dbref = 6;"
                  "UPDATE btech_autopilots SET mech_dbref = 3, map_dbref = 2, "
                  "speed_percent = 75, offset_x = 2, offset_y = 3, "
                  "verbose_level = 4 "
                  "WHERE dbref = 5;"
                  "INSERT INTO btech_autopilot_commands VALUES (5, 0, 23, 2);"
                  "INSERT INTO btech_autopilot_command_args VALUES (5, 0, 0, "
                  "'speed');"
                  "INSERT INTO btech_autopilot_command_args VALUES (5, 0, 1, "
                  "'50');"
                  "INSERT INTO btech_autopilot_commands VALUES (5, 1, 20, 1);"
                  "INSERT INTO btech_autopilot_command_args VALUES (5, 1, 0, "
                  "'report');"
                  "INSERT INTO btech_autopilot_path VALUES (5, 0, 2, 3, 1, 2, "
                  "4, 5, 9, 7);"
                  "INSERT INTO btech_repair_events "
                  "(mech_dbref, event_type, remaining_ticks, event_data, "
                  "is_fake) "
                  "VALUES (3, 57, 120, 0, 1);",
                  NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Verify non-default values survived a SQLite-only read and follow-up dump. */
static int check_btech_nondefault_state(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      check_btech_value(sqlite, "map temperature",
                        "SELECT temperature FROM btech_maps WHERE dbref = 2;",
                        17) == 0 &&
              check_btech_value(
                  sqlite, "individual object powers",
                  "SELECT count(*) FROM objects WHERE dbref = 2 AND "
                  "has_idle_power = 1 AND has_long_fingers_power = 0 AND "
                  "has_comm_all_power = 1 "
                  "AND has_see_hidden_power = 0 AND has_no_destroy_power = 1 "
                  "AND has_mech_power = 1 AND "
                  "has_security_power = 0 "
                  "AND has_mechrep_power = 1 AND has_map_power = 0 AND "
                  "has_template_power = 1 "
                  "AND has_tech_power = 0;",
                  1) == 0 &&
              check_btech_value(
                  sqlite, "map regeneration",
                  "SELECT regen_factor FROM btech_maps WHERE dbref = 2;",
                  7) == 0 &&
              check_btech_value(sqlite, "map hex",
                                "SELECT value FROM btech_map_hexes WHERE "
                                "map_dbref = 2 AND x = 0 AND y = 0;",
                                42) == 0 &&
              check_btech_value(sqlite, "mech section armor",
                                "SELECT armor FROM btech_mech_sections WHERE "
                                "mech_dbref = 3 AND section = 0;",
                                19) == 0 &&
              check_btech_value(sqlite, "mech critical data",
                                "SELECT data FROM btech_mech_criticals WHERE "
                                "mech_dbref = 3 AND section = 0 AND slot = 0;",
                                3) == 0 &&
              check_btech_value(
                  sqlite, "mech critical fire mode",
                  "SELECT fire_mode FROM btech_mech_criticals WHERE mech_dbref "
                  "= 3 AND section = 0 AND slot = 0;",
                  64) == 0 &&
              check_btech_value(
                  sqlite, "mech position x",
                  "SELECT x FROM btech_mech_positions WHERE mech_dbref = 3;",
                  2) == 0 &&
              check_btech_value(
                  sqlite, "mech position y",
                  "SELECT y FROM btech_mech_positions WHERE mech_dbref = 3;",
                  3) == 0 &&
              check_btech_value(
                  sqlite, "mech team",
                  "SELECT team FROM btech_mech_positions WHERE mech_dbref = 3;",
                  9) == 0 &&
              check_btech_value(
                  sqlite, "mech status",
                  "SELECT status FROM btech_mech_runtime WHERE mech_dbref = 3;",
                  8) == 0 &&
              check_btech_value(sqlite, "mech last used",
                                "SELECT last_used FROM btech_mech_runtime "
                                "WHERE mech_dbref = 3;",
                                77) == 0 &&
              check_btech_value(sqlite, "map occupancy",
                                "SELECT mech_dbref FROM btech_map_slots WHERE "
                                "map_dbref = 2 AND slot = 0;",
                                3) == 0 &&
              check_btech_value(
                  sqlite, "map LOS",
                  "SELECT flags FROM btech_map_los WHERE map_dbref = 2 AND "
                  "source_slot = 0 AND target_slot = 0;",
                  8193) == 0 &&
              check_btech_value(
                  sqlite, "map object",
                  "SELECT data_int FROM btech_map_objects WHERE map_dbref = 2 "
                  "AND object_type = 9 AND ordinal = 0;",
                  6) == 0 &&
              check_btech_value(sqlite, "map terrain bits",
                                "SELECT value FROM btech_map_bits WHERE "
                                "map_dbref = 2 AND y = 0 AND byte_index = 0;",
                                10) == 0 &&
              check_btech_value(
                  sqlite, "mech C3i node",
                  "SELECT node_dbref FROM btech_mech_c3_nodes WHERE mech_dbref "
                  "= 3 AND network_type = 0 AND node_index = 0;",
                  3) == 0 &&
              check_btech_value(
                  sqlite, "mech C3 node",
                  "SELECT node_dbref FROM btech_mech_c3_nodes WHERE mech_dbref "
                  "= 3 AND network_type = 1 AND node_index = 0;",
                  3) == 0 &&
              check_btech_value(
                  sqlite, "mech tic",
                  "SELECT value FROM btech_mech_tics WHERE mech_dbref = 3 AND "
                  "tic_index = 0 AND word_index = 0;",
                  12345) == 0 &&
              check_btech_value(sqlite, "mech frequency",
                                "SELECT frequency FROM btech_mech_frequencies "
                                "WHERE mech_dbref = 3 AND frequency_index = 0;",
                                42) == 0 &&
              check_btech_value(sqlite, "mech stagger history",
                                "SELECT amount FROM btech_mech_stagger_damage "
                                "WHERE mech_dbref = 3 AND position = 0;",
                                17) == 0 &&
              check_btech_value(
                  sqlite, "turret arcs",
                  "SELECT arcs FROM btech_turrets WHERE dbref = 6;", 5) == 0 &&
              check_btech_value(
                  sqlite, "turret target x",
                  "SELECT target_x FROM btech_turrets WHERE dbref = 6;",
                  2) == 0 &&
              check_btech_value(
                  sqlite, "turret target y",
                  "SELECT target_y FROM btech_turrets WHERE dbref = 6;",
                  3) == 0 &&
              check_btech_value(
                  sqlite, "autopilot requeued speed command",
                  "SELECT speed_percent FROM btech_autopilots WHERE dbref = 5;",
                  50) == 0 &&
              check_btech_value(
                  sqlite, "autopilot offset x",
                  "SELECT offset_x FROM btech_autopilots WHERE dbref = 5;",
                  2) == 0 &&
              check_btech_value(
                  sqlite, "autopilot offset y",
                  "SELECT offset_y FROM btech_autopilots WHERE dbref = 5;",
                  3) == 0 &&
              check_btech_value(
                  sqlite, "autopilot verbosity",
                  "SELECT verbose_level FROM btech_autopilots WHERE dbref = 5;",
                  4) == 0 &&
              check_btech_value(
                  sqlite, "autopilot command enum",
                  "SELECT command_enum FROM btech_autopilot_commands WHERE "
                  "autopilot_dbref = 5 AND position = 0;",
                  20) == 0 &&
              check_btech_value(
                  sqlite, "autopilot command argument",
                  "SELECT count(*) FROM btech_autopilot_command_args WHERE "
                  "autopilot_dbref = 5 AND command_position = 0 AND "
                  "argument_index = 0 AND value = 'report';",
                  1) == 0 &&
              check_btech_value(sqlite, "autopilot command queue",
                                "SELECT count(*) FROM btech_autopilot_commands "
                                "WHERE autopilot_dbref = 5;",
                                1) == 0 &&
              check_btech_value(sqlite, "autopilot path",
                                "SELECT f_score FROM btech_autopilot_path "
                                "WHERE autopilot_dbref = 5 AND position = 0;",
                                9) == 0 &&
              check_btech_value(
                  sqlite, "repair event",
                  "SELECT count(*) FROM btech_repair_events WHERE mech_dbref = "
                  "3 AND event_type = 57 AND is_fake = 1;",
                  1) == 0
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

/* Check command-queue state at a checkpoint before its dispatcher can run. */
static int check_btech_queued_command_state(const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      check_btech_value(
          sqlite, "reload autopilot speed",
          "SELECT speed_percent FROM btech_autopilots WHERE dbref = 5;",
          75) == 0 &&
              check_btech_value(sqlite, "reload autopilot command count",
                                "SELECT count(*) FROM btech_autopilot_commands "
                                "WHERE autopilot_dbref = 5;",
                                2) == 0 &&
              check_btech_value(
                  sqlite, "reload autopilot command enum",
                  "SELECT command_enum FROM btech_autopilot_commands WHERE "
                  "autopilot_dbref = 5 AND position = 0;",
                  23) == 0 &&
              check_btech_value(
                  sqlite, "reload autopilot command argument",
                  "SELECT count(*) FROM btech_autopilot_command_args WHERE "
                  "autopilot_dbref = 5 AND command_position = 0 AND "
                  "argument_index = 1 AND value = '50';",
                  1) == 0
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

int main(int argc, char *argv[]) {
  char directory[] = "/tmp/btmux-gamedb-test.XXXXXX";
  char config[PATH_MAX];
  char bootstrap_config[PATH_MAX];
  char sqlite_read_config[PATH_MAX];
  char missing_config[PATH_MAX];
  char sqlite_directory[PATH_MAX];
  char database[PATH_MAX];
  char crash_database[PATH_MAX];
  char killed_database[PATH_MAX];
  FILE *file;
  int dump_failure;
  int status;
  int result;
  struct stat snapshot_before;
  struct stat snapshot_after;

  if (argc != 2 || !mkdtemp(directory))
    return 2;
  if (snprintf(config, sizeof(config), "%s/game.conf", directory) < 0 ||
      snprintf(missing_config, sizeof(missing_config), "%s/missing.conf",
               directory) < 0 ||
      snprintf(bootstrap_config, sizeof(bootstrap_config), "%s/bootstrap.conf",
               directory) < 0 ||
      snprintf(sqlite_read_config, sizeof(sqlite_read_config),
               "%s/sqlite-read.conf", directory) < 0 ||
      snprintf(sqlite_directory, sizeof(sqlite_directory), "%s/sqlite",
               directory) < 0 ||
      snprintf(database, sizeof(database), "%s/sqlite/game.sqlite", directory) <
          0 ||
      snprintf(crash_database, sizeof(crash_database), "%s.CRASH", database) <
          0 ||
      snprintf(killed_database, sizeof(killed_database), "%s.KILLED",
               database) < 0 ||
      mkdir(sqlite_directory, 0700) < 0)
    return 2;

  file = fopen(config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[mux]\nhave_specials = 0\n");
  fprintf(file, "default_room_lua_parent = \"room.lua\"\n");
  fprintf(file, "default_player_lua_parent = \"player.lua\"\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;

  result = run_server(argv[1], config, 1, &status) == 0 && WIFEXITED(status) &&
                   WEXITSTATUS(status) == 0 && check_snapshot(database) == 0 &&
                   check_minimal_lua_parents(database) == 0
#ifdef BTMUX_TEST_ADVANCED_ECON
                   && check_zero_economy(database) == 0
#endif
               ? 0
               : 1;

  if (result == 0 && seed_commac_snapshot(database) < 0)
    result = 1;
  if (result == 0 && seed_btech_special_objects(database) < 0)
    result = 1;

  file = fopen(bootstrap_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[mux]\nhave_specials = 1\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0) {
    if (setenv("BTMUX_TEST_BTECH_BOOTSTRAP", "1", 1) < 0)
      return 1;
    result = run_server_in_directory(argv[1], bootstrap_config, directory, 0,
                                     &status) == 0 &&
                     WIFEXITED(status) && WEXITSTATUS(status) != 2 &&
                     check_btech_special_snapshot(database) == 0
                 ? 0
                 : 1;
    unsetenv("BTMUX_TEST_BTECH_BOOTSTRAP");
    if (result != 0) {
      fprintf(stderr, "BTech fixture bootstrap failed: %s (status=%d)\n",
              directory, status);
      return 1;
    }
  }
  if (result == 0 && seed_btech_nondefault_state(database) < 0)
    return 1;
  file = fopen(sqlite_read_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[mux]\nhave_specials = 1\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 &&
      (run_server_in_directory_after(argv[1], sqlite_read_config, directory, 0,
                                     &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
       check_snapshot_dump_type(database, 0) < 0 ||
       check_btech_special_snapshot(database) < 0 ||
       check_btech_queued_command_state(database) < 0)) {
    fprintf(stderr, "SQLite reload fixture failed: %s (status=%d)\n", directory,
            status);
    return 1;
  }
  if (result == 0 && (run_server_crash_in_directory(argv[1], sqlite_read_config,
                                                    directory, &status) < 0 ||
                      !WIFSIGNALED(status) || WTERMSIG(status) != SIGKILL ||
                      check_snapshot_dump_type(crash_database, 1) < 0 ||
                      check_btech_special_snapshot(crash_database) < 0 ||
                      check_btech_queued_command_state(crash_database) < 0)) {
    fprintf(stderr, "SQLite crash-dump fixture failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (result == 0 &&
      (run_server_killed_in_directory(argv[1], sqlite_read_config, directory,
                                      &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
       check_snapshot_dump_type(killed_database, 4) < 0 ||
       check_btech_special_snapshot(killed_database) < 0 ||
       check_btech_queued_command_state(killed_database) < 0)) {
    fprintf(stderr, "SQLite killed-dump fixture failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (result == 0 &&
      (run_server_in_directory_for(argv[1], sqlite_read_config, directory, 0, 2,
                                   &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) == 2 ||
       check_btech_special_snapshot(database) < 0 ||
       check_btech_nondefault_state(database) < 0)) {
    fprintf(stderr, "SQLite-read fixture startup failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (result == 0) {
    const char *const phases[] = {"prepare", "step"};
    size_t phase_index;
    size_t table_index;

    for (phase_index = 0;
         phase_index < sizeof(phases) / sizeof(phases[0]) && result == 0;
         phase_index++) {
      for (table_index = 0;
           table_index < sizeof(btech_special_writer_tables) /
                             sizeof(btech_special_writer_tables[0]);
           table_index++) {
        if (check_btech_writer_fault(argv[1], sqlite_read_config, directory,
                                     database,
                                     btech_special_writer_tables[table_index],
                                     phases[phase_index]) < 0) {
          fprintf(stderr, "BTech writer %s fault test failed for %s: %s\n",
                  phases[phase_index], btech_special_writer_tables[table_index],
                  directory);
          result = 1;
          break;
        }
      }
    }
    if (result != 0)
      return 1;
  }
  if (result == 0 && (set_invalid_power_value(database, 2) < 0 ||
                      run_server_in_directory(argv[1], sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0 ||
                      set_invalid_power_value(database, 1) < 0)) {
    fprintf(stderr, "Invalid object power fixture unexpectedly started: %s\n",
            directory);
    return 1;
  }
  if (result == 0) {
    dump_failure = stat(database, &snapshot_before) < 0 ||
                   chmod(sqlite_directory, 0500) < 0 ||
                   run_server_in_directory(argv[1], sqlite_read_config,
                                           directory, 0, &status) < 0 ||
                   !WIFEXITED(status) || WEXITSTATUS(status) != 0;
    if (chmod(sqlite_directory, 0700) < 0)
      dump_failure = 1;
    if (stat(database, &snapshot_after) < 0 ||
        snapshot_before.st_dev != snapshot_after.st_dev ||
        snapshot_before.st_ino != snapshot_after.st_ino ||
        snapshot_before.st_size != snapshot_after.st_size)
      dump_failure = 1;
    if (dump_failure || check_snapshot(database) < 0 ||
        check_btech_special_snapshot(database) < 0 ||
        check_btech_nondefault_state(database) < 0) {
      fprintf(stderr,
              "SQLite dump failure did not retain the prior snapshot: %s\n",
              directory);
      return 1;
    }
  }
  if (result == 0 && (remove_btech_runtime_row(database) < 0 ||
                      run_server_in_directory(argv[1], sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0)) {
    fprintf(stderr, "Corrupt SQLite BTech fixture unexpectedly started: %s\n",
            directory);
    return 1;
  }

#ifdef BTMUX_TEST_ADVANCED_ECON
  if (result == 0 &&
      (run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_zero_economy(database) < 0 || check_commac_snapshot(database) < 0))
    result = 1;

  if (result == 0 &&
      (insert_sparse_economy_cost(database) < 0 ||
       check_btech_writer_fault(argv[1], config, directory, database,
                                "btech_economy_costs", "prepare") < 0 ||
       check_btech_writer_fault(argv[1], config, directory, database,
                                "btech_economy_costs", "step") < 0 ||
       run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_sparse_economy_cost(database) < 0 ||
       check_commac_snapshot(database) < 0))
    result = 1;
#else
  if (result == 0 &&
      (run_server(argv[1], config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_commac_snapshot(database) < 0))
    result = 1;
#endif

#ifdef BTMUX_TEST_ADVANCED_ECON
  if (result == 0 && (drop_sqlite_economy(database) < 0 ||
                      run_server(argv[1], config, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) != 2))
    result = 1;
#endif

  file = fopen(missing_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 && (run_server(argv[1], missing_config, 1, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) != 2))
    result = 1;

  unlink(config);
  unlink(bootstrap_config);
  unlink(sqlite_read_config);
  unlink(missing_config);
  unlink(database);
  unlink(crash_database);
  unlink(killed_database);
  rmdir(sqlite_directory);
  rmdir(directory);
  return result;
}
