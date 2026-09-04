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

#include "mux/support/checked_storage.h"

static const char *string_catalog_item(const char *const *items, size_t count,
                                       size_t index) {
  return *(const char *const *)checked_storage_at_const(items, count,
                                                        sizeof(*items), index);
}

static int wait_for_child(pid_t child, int *status, int timeout_ms) {
  const struct timespec delay = {.tv_sec = 0, .tv_nsec = 25000000};

  for (int elapsed_ms = 0; elapsed_ms < timeout_ms; elapsed_ms += 25) {
    pid_t waited = waitpid(child, status, WNOHANG);
    if (waited == child)
      return 0;
    if (waited < 0)
      return -1;
    nanosleep(&delay, nullptr);
  }
  kill(child, SIGKILL);
  waitpid(child, status, 0);
  return -1;
}

/* Start a child and wait until it is ready or rejects its configuration. */
static pid_t start_server_after(const char *binary_path, const char *config,
                                const char *directory,
                                int make_minimal [[maybe_unused]],
                                bool wait_for_tick, int *status, bool *exited) {
  char ready_fd[32];
  char ready_signal;
  int ready_pipe[2];
  int tick_pipe[2] = {-1, -1};
  struct pollfd ready;
  pid_t child;

  if (pipe(ready_pipe) < 0)
    return -1;
  if (wait_for_tick && pipe(tick_pipe) < 0) {
    close(ready_pipe[0]);
    close(ready_pipe[1]);
    return -1;
  }
  child = fork();
  if (child < 0) {
    close(ready_pipe[0]);
    close(ready_pipe[1]);
    return -1;
  }
  if (child == 0) {
    close(ready_pipe[0]);
    if (wait_for_tick)
      close(tick_pipe[0]);
    snprintf(ready_fd, sizeof(ready_fd), "%d", ready_pipe[1]);
    if ((directory != nullptr && chdir(directory) < 0) ||
        setenv("BTECH_TEST_READY_FD", ready_fd, 1) < 0)
      _exit(127);
    if (wait_for_tick) {
      snprintf(ready_fd, sizeof(ready_fd), "%d", tick_pipe[1]);
      if (setenv("BTECH_TEST_TICK_FD", ready_fd, 1) < 0)
        _exit(127);
    }
    execl(binary_path, binary_path, config, NULL);
    _exit(127);
  }
  close(ready_pipe[1]);
  if (wait_for_tick)
    close(tick_pipe[1]);
  ready.fd = ready_pipe[0];
  ready.events = POLLIN;
  ready.revents = 0;
  *exited = false;
  for (int elapsed_ms = 0; elapsed_ms < 5000; elapsed_ms += 25) {
    pid_t waited = waitpid(child, status, WNOHANG);
    if (waited == child) {
      *exited = true;
      close(ready_pipe[0]);
      if (wait_for_tick)
        close(tick_pipe[0]);
      return child;
    }
    if (waited < 0)
      break;
    int poll_result = poll(&ready, 1, 25);
    if (poll_result == 1 && (ready.revents & POLLIN) &&
        read(ready_pipe[0], &ready_signal, sizeof(ready_signal)) ==
            sizeof(ready_signal)) {
      close(ready_pipe[0]);
      if (wait_for_tick) {
        ready.fd = tick_pipe[0];
        ready.events = POLLIN;
        ready.revents = 0;
        if (poll(&ready, 1, 5000) != 1 || !(ready.revents & POLLIN) ||
            read(tick_pipe[0], &ready_signal, sizeof(ready_signal)) !=
                sizeof(ready_signal)) {
          close(tick_pipe[0]);
          kill(child, SIGKILL);
          waitpid(child, status, 0);
          return -1;
        }
        close(tick_pipe[0]);
      }
      return child;
    }
    if (poll_result == 1 && (ready.revents & POLLHUP)) {
      close(ready_pipe[0]);
      if (wait_for_child(child, status, 5000) == 0) {
        if (wait_for_tick)
          close(tick_pipe[0]);
        *exited = true;
        return child;
      }
      if (wait_for_tick)
        close(tick_pipe[0]);
      return -1;
    }
  }
  {
    close(ready_pipe[0]);
    if (wait_for_tick)
      close(tick_pipe[0]);
    kill(child, SIGKILL);
    waitpid(child, status, 0);
    return -1;
  }
}

static int run_server_at(const char *binary_path, const char *config,
                         const char *directory, int make_minimal,
                         bool wait_for_tick, int *status) {
  bool exited;
  pid_t child;

  child = start_server_after(binary_path, config, directory, make_minimal,
                             wait_for_tick, status, &exited);
  if (child < 0)
    return -1;
  if (exited)
    return 0;
  if (kill(child, SIGTERM) < 0 && errno != ESRCH)
    return -1;
  return wait_for_child(child, status, 5000);
}

static int run_server(const char *binary_path, const char *config,
                      int make_minimal, int *status) {
  return run_server_at(binary_path, config, nullptr, make_minimal, false,
                       status);
}

static int run_server_in_directory_after(const char *binary_path,
                                         const char *config,
                                         const char *directory,
                                         int make_minimal, int *status) {
  return run_server_at(binary_path, config, directory, make_minimal, false,
                       status);
}

static int run_server_in_directory_after_tick(const char *binary_path,
                                              const char *config,
                                              const char *directory,
                                              int make_minimal, int *status) {
  return run_server_at(binary_path, config, directory, make_minimal, true,
                       status);
}

/* Start an isolated server long enough for normal startup work, then stop it.
 */
static int run_server_in_directory(const char *binary_path, const char *config,
                                   const char *directory, int make_minimal,
                                   int *status) {
  return run_server_in_directory_after(binary_path, config, directory,
                                       make_minimal, status);
}

/* Trigger the fatal-signal crash dump without attempting process recovery. */
static int run_server_crash_in_directory(const char *binary_path,
                                         const char *config,
                                         const char *directory, int *status) {
  const struct timespec delay = {.tv_sec = 0, .tv_nsec = 25000000};
  pid_t child;
  bool exited;

  child = start_server_after(binary_path, config, directory, 0, false, status,
                             &exited);
  if (child < 0 || exited)
    return -1;
  if (kill(child, SIGBUS) < 0)
    return -1;
  for (int elapsed_ms = 0; elapsed_ms < 5000; elapsed_ms += 25) {
    char crash_path[PATH_MAX];
    struct stat crash_snapshot;

    if (snprintf(crash_path, sizeof(crash_path), "%s/sqlite/game.sqlite.CRASH",
                 directory) < 0)
      return -1;
    if (stat(crash_path, &crash_snapshot) == 0 && crash_snapshot.st_size > 0)
      break;
    if (elapsed_ms == 4975) {
      kill(child, SIGKILL);
      waitpid(child, status, 0);
      return -1;
    }
    nanosleep(&delay, nullptr);
  }
  if (kill(child, SIGKILL) < 0 && errno != ESRCH)
    return -1;
  return waitpid(child, status, 0) == child ? 0 : -1;
}

/* Exercise SIGUSR2's intentional DUMP_KILLED shutdown path. */
static int run_server_killed_in_directory(const char *binary_path,
                                          const char *config,
                                          const char *directory, int *status) {
  const struct timespec delay = {.tv_sec = 0, .tv_nsec = 25000000};
  pid_t child;
  bool exited;
  pid_t waited;

  child = start_server_after(binary_path, config, directory, 0, false, status,
                             &exited);
  if (child < 0 || exited)
    return -1;
  if (kill(child, SIGUSR2) < 0)
    return -1;

  for (int elapsed_ms = 0; elapsed_ms < 5000; elapsed_ms += 25) {
    waited = waitpid(child, status, WNOHANG);
    if (waited == child)
      return 0;
    if (waited < 0)
      return -1;
    nanosleep(&delay, nullptr);
  }
  kill(child, SIGKILL);
  waitpid(child, status, 0);
  return -1;
}

static int query_int(sqlite3 *sqlite, const char *sql, sqlite3_int64 expected) {
  sqlite3_stmt *statement;
  int ok;
  sqlite3_int64 actual = 0;

  statement = NULL;
  ok = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
       sqlite3_step(statement) == SQLITE_ROW;
  if (ok)
    actual = sqlite3_column_int64(statement, 0);
  ok = ok && actual == expected;
  if (!ok)
    fprintf(stderr, "SQLite assertion failed: expected %lld, got %lld: %s\n",
            (long long)expected, (long long)actual, sql);
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
  result =
      query_text(sqlite, "SELECT lua_parent FROM objects WHERE dbref = 0;",
                 "room.lua") == 0 &&
              query_text(sqlite,
                         "SELECT lua_parent FROM objects WHERE dbref = 1;",
                         "player.lua") == 0 &&
              query_text(sqlite,
                         "SELECT lua_parent FROM objects WHERE dbref = 2;",
                         "player.lua") == 0 &&
              query_text(sqlite, "SELECT name FROM objects WHERE dbref = 1;",
                         "GOD") == 0 &&
              query_text(sqlite, "SELECT name FROM objects WHERE dbref = 2;",
                         "Wizard") == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM objects WHERE dbref IN "
                        "(0,3,4,5) AND type = 0;",
                        4) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM objects WHERE dbref IN "
                        "(1,2) AND type = 3 AND location = 4 AND link = 4 "
                        "AND has_wizard_flag = 1;",
                        2) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM objects WHERE "
                        "has_no_command_flag = 1 AND dbref IN (0,3,4,5);",
                        4) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM objects WHERE dbref IN "
                        "(1,2) AND has_ansi_flag = 1 AND "
                        "has_in_character_flag = 1;",
                        2) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM player_state WHERE "
                        "object_dbref IN (1,2) AND password_hash IS NOT "
                        "NULL AND password_hash != '';",
                        2) == 0 &&
              query_int(sqlite,
                        "SELECT count(DISTINCT password_hash) FROM "
                        "player_state WHERE object_dbref IN (1,2);",
                        2) == 0
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
  if (result < 0) {
    fprintf(stderr,
            "BTech SQLite round-trip mismatch for %s: expected %lld, got %lld "
            "(%s)\n",
            label, (long long)expected, (long long)actual,
            sqlite3_errmsg(sqlite));
  }
  sqlite3_finalize(statement);
  return result;
}

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
static int check_snapshot(const char *path) {
  sqlite3 *sqlite;
  int ok;

  sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  ok =
      query_int(
          sqlite,
          "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name "
          "IN ('snapshot', 'objects', 'player_state', "
          "'btech_special_registrations', "
          "'object_state', 'player_login_history', "
          "'player_last_page_recipients', 'btech_economy_parts', "
          "'btech_character_state', 'btech_character_values');",
          10) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('btech_maps') "
                "WHERE name = 'display_name';",
                0) == 0 &&
      query_int(sqlite, "SELECT schema_version FROM snapshot WHERE id = 1;",
                32) == 0 &&
      query_int(sqlite, "SELECT storage_format FROM snapshot WHERE id = 1;",
                1) == 0 &&
      query_int(sqlite, "SELECT storage_version FROM snapshot WHERE id = 1;",
                32) == 0 &&
      query_int(sqlite, "SELECT dump_type FROM snapshot WHERE id = 1;", 0) ==
          0 &&
      (query_int(sqlite, "SELECT count(*) FROM objects;", 6) == 0 ||
       query_int(sqlite, "SELECT count(*) FROM objects;", 7) == 0) &&
      query_int(sqlite, "SELECT count(*) FROM objects WHERE affiliation != -1;",
                0) == 0 &&
      (query_int(sqlite, "SELECT count(*) FROM object_state;", 0) == 0 ||
       query_int(sqlite, "SELECT count(*) FROM object_state;", 6) == 0) &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('object_state') "
                "WHERE name IN ('namespace', 'key', 'value_type', 'value');",
                4) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') "
                "WHERE name = 'owner';",
                0) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                "name = 'has_idle_power';",
                1) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                "name IN ('has_long_fingers_power', 'has_comm_all_power', "
                "'has_see_hidden_power', 'has_no_destroy_power');",
                0) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                "name IN ('has_mech_power', 'has_mechrep_power', "
                "'has_map_power', 'has_template_power', 'has_tech_power', "
                "'has_security_power');",
                0) == 0 &&
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
                "name = 'admin_comment';",
                0) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                "name IN ('enter_alias', 'leave_alias');",
                0) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM pragma_table_info('objects') WHERE "
                "name IN ('flags', 'flags2', 'flags3');",
                0) == 0 &&
      query_int(
          sqlite,
          "SELECT count(*) FROM pragma_table_info('objects') WHERE "
          "name IN ('type', 'lua_parent', 'has_ansi_flag', "
          "'has_audible_flag', 'has_auditorium_flag', 'has_blind_flag', "
          "'has_connected_flag', 'has_dark_flag', 'has_floating_flag', "
          "'has_gagged_flag', 'has_going_flag', 'has_halted_flag', "
          "'has_in_character_flag', 'has_light_flag', 'has_monitor_flag', "
          "'has_no_command_flag', "
          "'has_safe_flag', 'has_suspect_flag', 'has_transparent_flag', "
          "'has_wizard_flag', 'has_zombie_flag');",
          21) == 0 &&
      query_int(
          sqlite,
          "SELECT count(*) FROM objects WHERE has_idle_power NOT IN (0, 1);",
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
           "AND name IN ('btech_persistence_metadata', "
           "'btech_special_registrations', 'btech_unit_configuration', "
           "'btech_player_configuration', 'btech_map_cargo_configuration', "
           "'btech_map_links', 'btech_map_entrances', 'btech_maps', "
           "'btech_map_hexes', 'btech_map_slots', "
           "'btech_map_los', 'btech_map_objects', 'btech_map_bits', "
           "'btech_repair_events', 'btech_mechrep', 'btech_turrets', "
           "'btech_turret_tics', 'btech_autopilots', "
           "'btech_autopilot_commands', 'btech_autopilot_command_args', "
           "'btech_autopilot_path', 'btech_mechs', 'btech_mech_sections', "
           "'btech_mech_criticals', 'btech_mech_positions', 'btech_mech_bays', "
           "'btech_mech_turrets', 'btech_mech_c3', 'btech_mech_c3_nodes', "
           "'btech_mech_tics', 'btech_mech_frequencies', 'btech_mech_runtime', "
           "'btech_mech_unit_aux', "
           "'btech_mech_stagger_damage');",
           34) == 0;
  ok = ok && query_int(sqlite,
                       "SELECT schema_version FROM btech_persistence_metadata "
                       "WHERE id = 1;",
                       5) == 0;
  ok =
      ok && query_int(sqlite,
                      "SELECT count(*) FROM sqlite_master WHERE type = 'table' "
                      "AND name = 'btech_economy_costs';",
                      1) == 0;
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
      setenv("BTECH_TEST_BTECH_FAIL_TABLE", table, 1) < 0 ||
      setenv("BTECH_TEST_BTECH_FAIL_PHASE", phase, 1) < 0)
    return -1;
  result =
      run_server_in_directory_after(binary_path, config, directory, 0, &status);
  unsetenv("BTECH_TEST_BTECH_FAIL_TABLE");
  unsetenv("BTECH_TEST_BTECH_FAIL_PHASE");
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
                  "DELETE FROM player_state WHERE object_dbref = 2;"
                  "INSERT OR REPLACE INTO objects "
                  "(dbref, name, location, contents, exits, next, link, zone, "
                  "type) VALUES "
                  "(2, 'Test map', -1, -1, -1, -1, -1, -1, 1),"
                  "(3, 'Test mech', -1, -1, -1, -1, -1, -1, 1),"
                  "(4, 'Test repair', -1, -1, -1, -1, -1, -1, 1),"
                  "(5, 'Test autopilot', -1, -1, -1, -1, -1, -1, 1),"
                  "(6, 'Test turret', -1, -1, -1, -1, -1, -1, 1);"
                  "INSERT OR REPLACE INTO btech_special_registrations "
                  "(dbref, special_type) "
                  "VALUES "
                  "(2, 'MAP'), (3, 'MECH'), (4, 'MECHREP'),"
                  "(5, 'AUTOPILOT'), (6, 'TURRET');"
                  "INSERT INTO btech_unit_configuration VALUES "
                  "(3, 'ZZ', 'Test Display', 'stripes', 1);"
                  "INSERT INTO btech_map_cargo_configuration VALUES "
                  "(2, 1, 2, 1);"
                  "INSERT INTO object_state VALUES "
                  "(2, 'test', 'CaseKey', 1, CAST('upper' AS BLOB)),"
                  "(2, 'test', 'casekey', 1, CAST('lower' AS BLOB)),"
                  "(2, 'test', 'enabled', 2, 1),"
                  "(2, 'test', 'count', 3, 42),"
                  "(2, 'test', 'rate', 4, 1.25),"
                  "(2, 'test', 'empty', 1, X'');"
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

static int seed_styled_object_text(const char *path) {
  sqlite3 *sqlite = NULL;
  char *error = NULL;
  int result = sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                           SQLITE_OK &&
                       sqlite3_exec(sqlite,
                                    "UPDATE objects SET "
                                    "name = '[fg=#112233]Styled[/]',"
                                    "description = '[fg=red]Description[/]',"
                                    "internal_description = "
                                    "'[bg=blue]Inside[/]' WHERE dbref = 2;",
                                    NULL, NULL, &error) == SQLITE_OK
                   ? 0
                   : -1;

  if (result < 0)
    fprintf(stderr, "Styled object fixture seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

static int check_styled_object_text(const char *path) {
  sqlite3 *sqlite = NULL;
  int result;

  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      query_text(sqlite, "SELECT name FROM objects WHERE dbref = 2;",
                 "[fg=#112233]Styled[/]") == 0 &&
              query_text(sqlite,
                         "SELECT description FROM objects WHERE dbref = 2;",
                         "[fg=red]Description[/]") == 0 &&
              query_text(
                  sqlite,
                  "SELECT internal_description FROM objects WHERE dbref = 2;",
                  "[bg=blue]Inside[/]") == 0
          ? 0
          : -1;
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
      query_int(sqlite, "SELECT count(*) FROM btech_special_registrations;",
                5) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_maps;", 1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mechs;", 1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM btech_unit_configuration "
                        "WHERE object_dbref=3 AND preferred_id='ZZ' AND "
                        "display_name='Test Display' AND markings='stripes' "
                        "AND assigned_pilot=1;",
                        1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM "
                        "btech_map_cargo_configuration WHERE map_dbref=2 "
                        "AND reveal_hint=1 AND ((x=1 AND y=2 AND "
                        "(SELECT width FROM btech_maps WHERE dbref=2)=21) OR "
                        "(x=25 AND y=25 AND (SELECT width FROM btech_maps "
                        "WHERE dbref=2)=30));",
                        1) == 0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_mechrep;", 1) ==
                  0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_autopilots;", 1) ==
                  0 &&
              query_int(sqlite, "SELECT count(*) FROM btech_turrets;", 1) ==
                  0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM object_state "
                        "WHERE object_dbref=2 AND namespace='test';",
                        6) == 0 &&
              query_int(sqlite,
                        "SELECT value FROM object_state WHERE object_dbref=2 "
                        "AND namespace='test' AND key='count' "
                        "AND value_type=3;",
                        42) == 0 &&
              query_int(sqlite,
                        "SELECT value = 1.25 FROM object_state "
                        "WHERE object_dbref=2 AND namespace='test' "
                        "AND key='rate' AND value_type=4 "
                        "AND typeof(value)='real';",
                        1) == 0 &&
              query_int(sqlite,
                        "SELECT length(value) = 0 FROM object_state "
                        "WHERE object_dbref=2 AND namespace='test' "
                        "AND key='empty' AND value_type=1 "
                        "AND typeof(value)='blob';",
                        1) == 0 &&
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

static int set_invalid_utf8_name(const char *path, int invalid) {
  sqlite3 *sqlite = NULL;
  const char *statement = invalid
                              ? "UPDATE objects SET name = CAST(X'80' AS TEXT) "
                                "WHERE dbref = 0;"
                              : "UPDATE objects SET name = 'Limbo' WHERE "
                                "dbref = 0;";
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite, statement, NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;

  sqlite3_close(sqlite);
  return result;
}

static int set_nonascii_player_alias(const char *path, int invalid) {
  sqlite3 *sqlite = NULL;
  const char *statement =
      invalid ? "UPDATE player_state SET alias = 'Jos\xC3\xA9' WHERE "
                "object_dbref = 1;"
              : "UPDATE player_state SET alias = NULL WHERE object_dbref = 1;";
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite, statement, NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;

  sqlite3_close(sqlite);
  return result;
}

static int set_spaced_player_alias(const char *path, bool enabled) {
  sqlite3 *sqlite = NULL;
  const char *statement =
      enabled ? "UPDATE player_state SET alias = 'Existing Player' WHERE "
                "object_dbref = 1;"
              : "UPDATE player_state SET alias = NULL WHERE object_dbref = 1;";
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite, statement, NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;

  sqlite3_close(sqlite);
  return result;
}

static int set_invalid_character_value_name(const char *path, int invalid) {
  sqlite3 *sqlite = NULL;
  const char *statement =
      invalid ? "UPDATE btech_character_values SET value_name = 'running' "
                "WHERE player_dbref = 1 AND value_name = 'Running';"
              : "UPDATE btech_character_values SET value_name = 'Running' "
                "WHERE player_dbref = 1 AND value_name = 'running';";
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite, statement, NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;

  sqlite3_close(sqlite);
  return result;
}

static int seed_player_account_state(const char *path) {
  sqlite3 *sqlite = NULL;
  char *error = NULL;
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "UPDATE player_state SET password_hash = 'test-hash', "
                  "alias = 'PersistedAlias', "
                  "last_login = 123456789, last_site = 'user@example.test', "
                  "successful_login_count = 9, failed_login_count = 4, "
                  "unreported_failed_login_count = 2 WHERE object_dbref = 1;"
                  "INSERT INTO player_login_history "
                  "(player_dbref, outcome, position, occurred_at, host) VALUES "
                  "(1, 0, 0, 123456789, 'good.example.test'),"
                  "(1, 1, 0, 123456700, 'bad.example.test');"
                  "INSERT INTO player_last_page_recipients "
                  "(player_dbref, position, recipient_dbref) VALUES "
                  "(1, 0, 1), (1, 1, 9999);",
                  NULL, NULL, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "Player account fixture seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

static int seed_economy_parts(const char *path) {
  sqlite3 *sqlite = NULL;
  char *error = NULL;
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite,
                           "INSERT INTO btech_economy_parts "
                           "(object_dbref, part_id, brand_id, quantity) VALUES "
                           "(1, 42, 0, 3), (1, 5, 2, 9);",
                           NULL, NULL, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "Economy parts fixture seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

/* A bad persisted repair event must reject startup before it can be queued. */
static int check_repair_event_rejection(const char *server, const char *config,
                                        const char *directory,
                                        const char *database, int event_type,
                                        int remaining_ticks, long event_data,
                                        int is_fake, int mech_dbref,
                                        int *status) {
  sqlite3 *sqlite = nullptr;
  sqlite3_stmt *statement = nullptr;
  int result = -1;

  if (sqlite3_open_v2(database, &sqlite, SQLITE_OPEN_READWRITE, nullptr) !=
          SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (?, ?, ?, ?, ?);",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_bind_int(statement, 1, mech_dbref) != SQLITE_OK ||
      sqlite3_bind_int(statement, 2, event_type) != SQLITE_OK ||
      sqlite3_bind_int(statement, 3, remaining_ticks) != SQLITE_OK ||
      sqlite3_bind_int64(statement, 4, event_data) != SQLITE_OK ||
      sqlite3_bind_int(statement, 5, is_fake) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_DONE)
    goto done;
  sqlite3_finalize(statement);
  statement = nullptr;
  if (run_server_in_directory(server, config, directory, 0, status) < 0 ||
      !WIFEXITED(*status) || WEXITSTATUS(*status) == 0)
    goto done;
  if (sqlite3_exec(sqlite,
                   "DELETE FROM btech_repair_events WHERE event_id = "
                   "(SELECT max(event_id) FROM btech_repair_events);",
                   nullptr, nullptr, nullptr) != SQLITE_OK)
    goto done;
  result = 0;

done:
  sqlite3_finalize(statement);
  sqlite3_close(sqlite);
  return result;
}

static int check_economy_parts(const char *path) {
  sqlite3 *sqlite = NULL;
  int result;

  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result = query_int(sqlite,
                     "SELECT count(*) FROM btech_economy_parts "
                     "WHERE object_dbref = 1 AND part_id = 42 AND "
                     "brand_id = 0 AND quantity = 3;",
                     1) == 0 &&
                   query_int(sqlite,
                             "SELECT count(*) FROM btech_economy_parts "
                             "WHERE object_dbref = 1 AND part_id = 5 AND "
                             "brand_id = 2 AND quantity = 9;",
                             1) == 0
               ? 0
               : -1;
  sqlite3_close(sqlite);
  return result;
}

static int seed_character_state(const char *path) {
  sqlite3 *sqlite = NULL;
  char *error = NULL;
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(
                  sqlite,
                  "INSERT INTO btech_character_state "
                  "(player_dbref, bruise, lethal, build, reflexes, intuition, "
                  "learn, charisma) VALUES (1, 2, 3, 4, 5, 6, 7, 8);"
                  "INSERT INTO btech_character_values "
                  "(player_dbref, value_name, value, xp, last_used) VALUES "
                  "(1, 'Running', 2, 300, 123456789),"
                  "(1, 'Toughness', 1, 0, 0),"
                  "(1, 'Lives', 0, 0, 0);",
                  NULL, NULL, &error) == SQLITE_OK
          ? 0
          : -1;
  if (result < 0)
    fprintf(stderr, "Character state fixture seed failed: %s\n",
            error ? error : sqlite3_errmsg(sqlite));
  sqlite3_free(error);
  sqlite3_close(sqlite);
  return result;
}

static int check_character_state(const char *path) {
  sqlite3 *sqlite = NULL;
  int result;

  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      query_int(sqlite,
                "SELECT bruise = 2 AND lethal = 3 AND build = 4 AND "
                "reflexes = 5 AND intuition = 6 AND learn = 7 AND "
                "charisma = 8 FROM btech_character_state "
                "WHERE player_dbref = 1;",
                1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM btech_character_values WHERE "
                        "player_dbref = 1 AND value_name = 'Running' AND "
                        "value = 2 AND xp = 300 AND "
                        "last_used = 123456789;",
                        1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM btech_character_values WHERE "
                        "player_dbref = 1 AND value_name = 'Lives' AND "
                        "value = 0;",
                        1) == 0
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return result;
}

static int check_character_state_delete_cascade(const char *path) {
  sqlite3 *sqlite = NULL;
  int result = -1;

  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) != SQLITE_OK)
    return -1;
  if (query_int(sqlite,
                "SELECT count(*) FROM pragma_foreign_key_list("
                "'btech_character_state') WHERE \"table\" = 'objects' "
                "AND on_delete = 'CASCADE';",
                1) == 0 &&
      sqlite3_exec(sqlite,
                   "PRAGMA foreign_keys = ON; BEGIN;"
                   "DELETE FROM btech_character_state "
                   "WHERE player_dbref = 1;",
                   NULL, NULL, NULL) == SQLITE_OK &&
      query_int(sqlite,
                "SELECT count(*) FROM btech_character_state "
                "WHERE player_dbref = 1;",
                0) == 0 &&
      query_int(sqlite,
                "SELECT count(*) FROM btech_character_values "
                "WHERE player_dbref = 1;",
                0) == 0)
    result = 0;
  sqlite3_exec(sqlite, "ROLLBACK;", NULL, NULL, NULL);
  sqlite3_close(sqlite);
  return result;
}

static int check_player_account_state(const char *path) {
  sqlite3 *sqlite = NULL;
  int result;

  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  result =
      query_text(sqlite,
                 "SELECT alias FROM player_state WHERE object_dbref = 1;",
                 "PersistedAlias") == 0 &&
              query_int(
                  sqlite,
                  "SELECT last_login = 123456789 AND "
                  "successful_login_count = 9 AND failed_login_count = 4 AND "
                  "unreported_failed_login_count = 2 FROM player_state "
                  "WHERE object_dbref = 1;",
                  1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM player_state WHERE "
                        "object_dbref NOT IN (SELECT dbref FROM objects WHERE "
                        "type = 3);",
                        0) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM player_login_history WHERE "
                        "player_dbref = 1 AND outcome = 0 AND position = 0 AND "
                        "occurred_at = 123456789 AND "
                        "host = 'good.example.test';",
                        1) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM player_last_page_recipients "
                        "WHERE player_dbref = 1 AND position = 1 AND "
                        "recipient_dbref = 9999;",
                        1) == 0
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
                  "UPDATE btech_maps SET temperature = 17, regen_factor = 7, "
                  "width = 30, height = 30 "
                  "WHERE dbref = 2;"
                  "DELETE FROM btech_map_hexes WHERE map_dbref = 2;"
                  "WITH RECURSIVE cells(x, y) AS ("
                  " VALUES(0, 0)"
                  " UNION ALL"
                  " SELECT CASE WHEN x=29 THEN 0 ELSE x+1 END,"
                  " CASE WHEN x=29 THEN y+1 ELSE y END"
                  " FROM cells WHERE x<29 OR y<29"
                  ") INSERT INTO btech_map_hexes(map_dbref, x, y, value)"
                  " SELECT 2, x, y, CASE WHEN x=0 AND y=0 THEN 42 ELSE 0 END "
                  "FROM cells;"
                  "UPDATE btech_map_cargo_configuration SET x=25, y=25 "
                  "WHERE map_dbref=2;"
                  "UPDATE btech_mech_sections SET armor = 19 "
                  "WHERE mech_dbref = 3 AND section = 0;"
                  "UPDATE btech_mech_criticals SET data = 3, fire_mode = 64 "
                  "WHERE mech_dbref = 3 AND section = 0 AND slot = 0;"
                  "UPDATE btech_mech_positions SET x = 2, y = 3, team = 9 "
                  "WHERE mech_dbref = 3;"
                  "UPDATE btech_mech_runtime SET heat = 12.5, status = 8, "
                  "last_used = 77, autopilot_num = 5 WHERE mech_dbref = 3;"
                  "UPDATE objects SET has_idle_power = 1 "
                  "WHERE dbref = 2;"
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

/* A stale coordinate must be discarded instead of making startup fail. */
static int seed_invalid_btech_configuration(const char *path) {
  sqlite3 *sqlite = NULL;
  int result =
      sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READWRITE, NULL) ==
                  SQLITE_OK &&
              sqlite3_exec(sqlite,
                           "UPDATE btech_map_cargo_configuration SET x=30 "
                           "WHERE map_dbref=2;",
                           NULL, NULL, NULL) == SQLITE_OK
          ? 0
          : -1;
  if (sqlite != NULL)
    sqlite3_close(sqlite);
  return result;
}

static int check_invalid_btech_configuration_removed(const char *path) {
  sqlite3 *sqlite = NULL;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK)
    return -1;
  const int RESULT =
      query_int(sqlite,
                "SELECT count(*) FROM btech_map_cargo_configuration "
                "WHERE map_dbref=2;",
                0) == 0 &&
              query_int(sqlite,
                        "SELECT count(*) FROM btech_unit_configuration "
                        "WHERE object_dbref=3 AND preferred_id='ZZ';",
                        1) == 0
          ? 0
          : -1;
  sqlite3_close(sqlite);
  return RESULT;
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
                  "has_idle_power = 1;",
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
  char config[PATH_MAX];
  char bootstrap_config[PATH_MAX];
  char invalid_god_config[PATH_MAX];
  char name_limit_config[PATH_MAX];
  char long_path_config[PATH_MAX];
  char sqlite_read_config[PATH_MAX];
  char missing_config[PATH_MAX];
  char sqlite_directory[PATH_MAX];
  char database[PATH_MAX];
  char crash_database[PATH_MAX];
  char killed_database[PATH_MAX];
  FILE *file;
  int dump_failure;
  int writer_shard_count = 1;
  int writer_shard_index = 0;
  int status;
  int result;
  bool recovery_rejection;
  bool recovery_snapshot;
  bool recovery_writer;
  struct stat snapshot_before;
  struct stat snapshot_after;

  if (argc != 4 && argc != 6)
    return 2;
  const char *server =
      string_catalog_item((const char *const *)argv, (size_t)argc, 1);
  const char *suite =
      string_catalog_item((const char *const *)argv, (size_t)argc, 2);
  const char *directory = string_catalog_item((const char *const *)argv,
                                              (size_t)argc, (size_t)argc - 1);
  recovery_snapshot = strcmp(suite, "recovery-snapshot") == 0;
  recovery_writer = strcmp(suite, "recovery-writer") == 0;
  recovery_rejection = strcmp(suite, "recovery-rejection") == 0;
  if (recovery_writer) {
    char *end;
    const char *shard_count_text;
    const char *shard_index_text;
    long parsed_index;
    long parsed_count;

    if (argc != 6)
      return 2;
    shard_index_text =
        string_catalog_item((const char *const *)argv, (size_t)argc, 3);
    shard_count_text =
        string_catalog_item((const char *const *)argv, (size_t)argc, 4);
    parsed_index = strtol(shard_index_text, &end, 10);
    if (*shard_index_text == '\0' || *end != '\0')
      return 2;
    writer_shard_index = (int)parsed_index;
    parsed_count = strtol(shard_count_text, &end, 10);
    if (*shard_count_text == '\0' || *end != '\0' || parsed_count < 1 ||
        parsed_index < 0 || parsed_index >= parsed_count)
      return 2;
    writer_shard_count = (int)parsed_count;
  } else if (argc != 4) {
    return 2;
  }
  if (snprintf(config, sizeof(config), "%s/game.conf", directory) < 0 ||
      snprintf(missing_config, sizeof(missing_config), "%s/missing.conf",
               directory) < 0 ||
      snprintf(bootstrap_config, sizeof(bootstrap_config), "%s/bootstrap.conf",
               directory) < 0 ||
      snprintf(invalid_god_config, sizeof(invalid_god_config),
               "%s/invalid-god.conf", directory) < 0 ||
      snprintf(name_limit_config, sizeof(name_limit_config),
               "%s/name-limit.conf", directory) < 0 ||
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
  fprintf(file, "[database.bootstrap.objects]\n");
  fprintf(file, "0 = { type = \"room\", name = \"Limbo\" }\n");
  fprintf(file, "1 = { type = \"player\", name = \"GOD\", wizard = true }\n");
  fprintf(file,
          "2 = { type = \"player\", name = \"Wizard\", wizard = true }\n");
  fprintf(file, "3 = { type = \"room\", name = \"Store\" }\n");
  fprintf(file, "4 = { type = \"room\", name = \"Start\" }\n");
  fprintf(file, "5 = { type = \"room\", name = \"Afterlife\" }\n");
  fprintf(file, "[mux]\n");
  fprintf(file, "default_room_lua_parent = \"room.lua\"\n");
  fprintf(file, "default_player_lua_parent = \"player.lua\"\n");
  fprintf(file, "default_room_flags = [\"no_command\"]\n");
  fprintf(file, "default_player_flags = [\"ansi\", \"in_character\"]\n");
  fprintf(file, "player_starting_room = 4\nplayer_starting_home = 4\n");
  fprintf(file, "[battletech]\nusedmechstore = 3\nafterlife_dbref = 5\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;

  result = run_server(server, config, 1, &status) == 0 && WIFEXITED(status) &&
                   WEXITSTATUS(status) == 0 && check_snapshot(database) == 0 &&
                   check_minimal_lua_parents(database) == 0 &&
                   check_zero_economy(database) == 0
               ? 0
               : 1;

  file = fopen(invalid_god_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[database.bootstrap.objects]\n");
  fprintf(file, "1 = { type = \"player\", name = \"GOD\" }\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 && (run_server(server, invalid_god_config, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0))
    result = 1;
  if (strcmp(suite, "bootstrap") == 0) {
    file = fopen(missing_config, "w");
    if (!file)
      return 2;
    fprintf(file, "[server]\nport = 0\n");
    if (fclose(file) != 0)
      return 2;
    if (result == 0 && (run_server(server, missing_config, 1, &status) < 0 ||
                        !WIFEXITED(status) || WEXITSTATUS(status) != 2))
      result = 1;
    return result;
  }
  if (strcmp(suite, "persistence") != 0 && !recovery_snapshot &&
      !recovery_writer && !recovery_rejection)
    return 2;

  if (strcmp(suite, "persistence") == 0) {
    file = fopen(name_limit_config, "w");
    if (file == nullptr)
      return 2;
    fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
    fprintf(file, "[names]\nmaximum_length = 2\n");
    fprintf(file, "[mux]\nplayer_name_spaces = false\n");
    fprintf(file, "[server]\nport = 0\n");
    if (fclose(file) != 0 || set_spaced_player_alias(database, true) < 0 ||
        (result == 0 &&
         (run_server(server, name_limit_config, 0, &status) < 0 ||
          !WIFEXITED(status) || WEXITSTATUS(status) == 2)) ||
        set_spaced_player_alias(database, false) < 0)
      result = 1;

    /* A configuration path longer than ServerConfiguration::config_file must
     * be reported and rejected instead of silently recorded truncated. */
    char long_name[160];
    memset(long_name, 'c', sizeof(long_name) - 1);
    long_name[sizeof(long_name) - 1] = '\0';
    if (snprintf(long_path_config, sizeof(long_path_config), "%s/%s.conf",
                 directory, long_name) < 0)
      return 2;
    file = fopen(long_path_config, "w");
    if (file == nullptr)
      return 2;
    fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
    fprintf(file, "[server]\nport = 0\n");
    if (fclose(file) != 0 ||
        (result == 0 && (run_server(server, long_path_config, 0, &status) < 0 ||
                         !WIFEXITED(status) || WEXITSTATUS(status) != 2)))
      result = 1;
  }

  if (result == 0 && seed_commac_snapshot(database) < 0)
    result = 1;
  if (result == 0 && seed_btech_special_objects(database) < 0)
    result = 1;

  file = fopen(bootstrap_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[database.bootstrap.objects]\n");
  fprintf(file, "1 = { type = \"player\", name = \"GOD\", wizard = true }\n");
  fprintf(file, "[mux]\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0) {
    if (setenv("BTECH_TEST_BTECH_BOOTSTRAP", "1", 1) < 0)
      return 1;
    result = run_server_in_directory(server, bootstrap_config, directory, 0,
                                     &status) == 0 &&
                     WIFEXITED(status) && WEXITSTATUS(status) != 2 &&
                     check_btech_special_snapshot(database) == 0
                 ? 0
                 : 1;
    unsetenv("BTECH_TEST_BTECH_BOOTSTRAP");
    if (result != 0) {
      fprintf(stderr, "BTech fixture bootstrap failed: %s (status=%d)\n",
              directory, status);
      return 1;
    }
  }
  if (result == 0 && seed_btech_nondefault_state(database) < 0)
    return 1;
  if (result == 0 && seed_styled_object_text(database) < 0)
    return 1;
  if (result == 0 && seed_player_account_state(database) < 0)
    return 1;
  if (result == 0 && seed_economy_parts(database) < 0)
    return 1;
  if (result == 0 && seed_character_state(database) < 0)
    return 1;
  if (result == 0 && check_character_state_delete_cascade(database) < 0)
    return 1;
  file = fopen(sqlite_read_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[database]\ngame_database = \"%s\"\n", database);
  fprintf(file, "[database.bootstrap.objects]\n");
  fprintf(file, "1 = { type = \"player\", name = \"GOD\", wizard = true }\n");
  fprintf(file, "[mux]\n");
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 &&
      (run_server_in_directory_after(server, sqlite_read_config, directory, 0,
                                     &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
       check_snapshot_dump_type(database, 0) < 0 ||
       check_btech_special_snapshot(database) < 0 ||
       check_btech_queued_command_state(database) < 0 ||
       check_styled_object_text(database) < 0 ||
       check_player_account_state(database) < 0 ||
       check_economy_parts(database) < 0 ||
       check_character_state(database) < 0)) {
    fprintf(stderr, "SQLite reload fixture failed: %s (status=%d)\n", directory,
            status);
    return 1;
  }
  if (strcmp(suite, "persistence") == 0) {
    if (run_server(server, config, 0, &status) < 0 || !WIFEXITED(status) ||
        WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
        check_zero_economy(database) < 0 ||
        check_commac_snapshot(database) < 0 ||
        insert_sparse_economy_cost(database) < 0 ||
        run_server(server, config, 0, &status) < 0 || !WIFEXITED(status) ||
        WEXITSTATUS(status) == 2 || check_sparse_economy_cost(database) < 0 ||
        seed_invalid_btech_configuration(database) < 0 ||
        run_server(server, config, 0, &status) < 0 || !WIFEXITED(status) ||
        WEXITSTATUS(status) != 0 ||
        check_invalid_btech_configuration_removed(database) < 0)
      return 1;
    return result;
  }
  if (recovery_snapshot && result == 0 &&
      (run_server_crash_in_directory(server, sqlite_read_config, directory,
                                     &status) < 0 ||
       !WIFSIGNALED(status) || WTERMSIG(status) != SIGKILL ||
       check_snapshot_dump_type(crash_database, 1) < 0 ||
       check_btech_special_snapshot(crash_database) < 0 ||
       check_btech_queued_command_state(crash_database) < 0 ||
       check_player_account_state(crash_database) < 0 ||
       check_economy_parts(crash_database) < 0 ||
       check_character_state(crash_database) < 0)) {
    fprintf(stderr, "SQLite crash-dump fixture failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (recovery_snapshot && result == 0 &&
      (run_server_killed_in_directory(server, sqlite_read_config, directory,
                                      &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) != 0 ||
       check_snapshot_dump_type(killed_database, 4) < 0 ||
       check_btech_special_snapshot(killed_database) < 0 ||
       check_btech_queued_command_state(killed_database) < 0 ||
       check_player_account_state(killed_database) < 0 ||
       check_economy_parts(killed_database) < 0 ||
       check_character_state(killed_database) < 0)) {
    fprintf(stderr, "SQLite killed-dump fixture failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (recovery_snapshot && result == 0 &&
      (run_server_in_directory_after_tick(server, sqlite_read_config, directory,
                                          0, &status) < 0 ||
       !WIFEXITED(status) || WEXITSTATUS(status) == 2 ||
       check_btech_special_snapshot(database) < 0 ||
       check_btech_nondefault_state(database) < 0 ||
       check_player_account_state(database) < 0 ||
       check_economy_parts(database) < 0 ||
       check_character_state(database) < 0)) {
    fprintf(stderr, "SQLite-read fixture startup failed: %s (status=%d)\n",
            directory, status);
    return 1;
  }
  if (recovery_snapshot && result == 0) {
    dump_failure = stat(database, &snapshot_before) < 0 ||
                   chmod(sqlite_directory, 0500) < 0 ||
                   run_server_in_directory(server, sqlite_read_config,
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
        check_btech_nondefault_state(database) < 0 ||
        check_player_account_state(database) < 0) {
      fprintf(stderr,
              "SQLite dump failure did not retain the prior snapshot: %s\n",
              directory);
      return 1;
    }
  }
  if (recovery_snapshot)
    return result;

  if (recovery_writer && result == 0) {
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
        if (table_index % (size_t)writer_shard_count !=
            (size_t)writer_shard_index)
          continue;
        const char *table =
            string_catalog_item(btech_special_writer_tables,
                                sizeof(btech_special_writer_tables) /
                                    sizeof(btech_special_writer_tables[0]),
                                table_index);
        const char *phase = string_catalog_item(
            phases, sizeof(phases) / sizeof(phases[0]), phase_index);
        if (check_btech_writer_fault(server, sqlite_read_config, directory,
                                     database, table, phase) < 0) {
          fprintf(stderr, "BTech writer %s fault test failed for %s: %s\n",
                  phase, table, directory);
          result = 1;
          break;
        }
      }
    }
    if (result != 0)
      return 1;
  }
  if (recovery_writer)
    return result;

  if (recovery_rejection && result == 0 &&
      (check_repair_event_rejection(server, sqlite_read_config, directory,
                                    database, 999, 1, 0, 0, 3, &status) < 0 ||
       check_repair_event_rejection(server, sqlite_read_config, directory,
                                    database, 47, 0, 0, 0, 3, &status) < 0 ||
       check_repair_event_rejection(server, sqlite_read_config, directory,
                                    database, 47, 1, 0, 0, 3, &status) < 0 ||
       check_repair_event_rejection(server, sqlite_read_config, directory,
                                    database, 47, 1, 16, 1, 3, &status) < 0 ||
       check_repair_event_rejection(server, sqlite_read_config, directory,
                                    database, 47, 1, 0, 0, 999, &status) < 0)) {
    fprintf(stderr,
            "Invalid BTech repair-event fixture unexpectedly started: "
            "%s\n",
            directory);
    return 1;
  }

  if (result == 0 && (set_invalid_power_value(database, 2) < 0 ||
                      run_server_in_directory(server, sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0 ||
                      set_invalid_power_value(database, 1) < 0)) {
    fprintf(stderr, "Invalid object power fixture unexpectedly started: %s\n",
            directory);
    return 1;
  }
  if (result == 0 && (set_invalid_utf8_name(database, 1) < 0 ||
                      run_server_in_directory(server, sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0 ||
                      set_invalid_utf8_name(database, 0) < 0)) {
    fprintf(stderr, "Invalid UTF-8 object name unexpectedly started: %s\n",
            directory);
    return 1;
  }
  if (result == 0 && (set_nonascii_player_alias(database, 1) < 0 ||
                      run_server_in_directory(server, sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0 ||
                      set_nonascii_player_alias(database, 0) < 0)) {
    fprintf(stderr, "Non-ASCII player alias unexpectedly started: %s\n",
            directory);
    return 1;
  }
  if (result == 0 && (set_invalid_character_value_name(database, 1) < 0 ||
                      run_server_in_directory(server, sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0 ||
                      set_invalid_character_value_name(database, 0) < 0)) {
    fprintf(stderr,
            "Noncanonical character value name unexpectedly started: %s\n",
            directory);
    return 1;
  }
  if (result == 0 &&
      (run_server(server, config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_zero_economy(database) < 0 || check_commac_snapshot(database) < 0))
    result = 1;

  if (result == 0 &&
      (insert_sparse_economy_cost(database) < 0 ||
       check_btech_writer_fault(server, config, directory, database,
                                "btech_economy_costs", "prepare") < 0 ||
       check_btech_writer_fault(server, config, directory, database,
                                "btech_economy_costs", "step") < 0 ||
       run_server(server, config, 0, &status) < 0 || !WIFEXITED(status) ||
       WEXITSTATUS(status) == 2 || check_snapshot(database) < 0 ||
       check_sparse_economy_cost(database) < 0 ||
       check_commac_snapshot(database) < 0))
    result = 1;

  if (result == 0 && (drop_sqlite_economy(database) < 0 ||
                      run_server(server, config, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) != 2))
    result = 1;

  if (result == 0 && (remove_btech_runtime_row(database) < 0 ||
                      run_server_in_directory(server, sqlite_read_config,
                                              directory, 0, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) == 0)) {
    fprintf(stderr, "Corrupt SQLite BTech fixture unexpectedly started: %s\n",
            directory);
    return 1;
  }

  file = fopen(missing_config, "w");
  if (!file)
    return 2;
  fprintf(file, "[server]\nport = 0\n");
  if (fclose(file) != 0)
    return 2;
  if (result == 0 && (run_server(server, missing_config, 1, &status) < 0 ||
                      !WIFEXITED(status) || WEXITSTATUS(status) != 2))
    result = 1;

  return result;
}
