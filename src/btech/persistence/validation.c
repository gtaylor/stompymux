#include "sqlite_internal.h"

typedef struct btech_special_object_counts BTECH_SPECIAL_OBJECT_COUNTS;
struct btech_special_object_counts {
  int maps;
  int mechs;
  int mechreps;
  int turrets;
  int autopilots;
};

/* Count normal BTech instances, excluding DEBUG and other non-persisted types.
 */
static int btech_special_count_objects(void *key, void *data, int depth,
                                       void *argument) {
  BTECH_SPECIAL_OBJECT_COUNTS *counts = argument;
  BtechSpecialObject *xcode = data;

  (void)key;
  (void)depth;
  switch (xcode->type) {
  case GTYPE_MAP:
    counts->maps++;
    break;
  case GTYPE_MECH:
    counts->mechs++;
    break;
  case GTYPE_MECHREP:
    counts->mechreps++;
    break;
  case GTYPE_TURRET:
    counts->turrets++;
    break;
  case GTYPE_AUTO:
    counts->autopilots++;
    break;
  default:
    break;
  }
  return 1;
}

/* Read a count from one known table name in the fixed BTech schema. */
static int btech_special_table_count(sqlite3 *sqlite, const char *table,
                                     int *count) {
  sqlite3_stmt *statement;
  char sql[128];
  int result;

  statement = NULL;
  if (snprintf(sql, sizeof(sql), "SELECT count(*) FROM %s;", table) < 0)
    return -1;
  result = sqlite3_prepare_v2(sqlite, sql, -1, &statement, NULL) == SQLITE_OK &&
                   sqlite3_step(statement) == SQLITE_ROW &&
                   btech_special_column_int(statement, 0, count) == 0 &&
                   sqlite3_step(statement) == SQLITE_DONE
               ? 0
               : -1;
  sqlite3_finalize(statement);
  return result;
}

/* Require one parent and every fixed child row for each preallocated object. */
static int btech_special_validate_required_rows(sqlite3 *sqlite,
                                                BtechContext *context) {
  BTECH_SPECIAL_OBJECT_COUNTS counts = {0, 0, 0, 0, 0};
  int expected;
  int actual;

  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      btech_special_count_objects, &counts);
#define REQUIRE_ROWS(table, rows)                                              \
  do {                                                                         \
    expected = (rows);                                                         \
    if (expected < 0 ||                                                        \
        btech_special_table_count(sqlite, table, &actual) < 0 ||               \
        actual != expected)                                                    \
      return -1;                                                               \
  } while (0)
  REQUIRE_ROWS("btech_maps", counts.maps);
  REQUIRE_ROWS("btech_mechs", counts.mechs);
  REQUIRE_ROWS("btech_mechrep", counts.mechreps);
  REQUIRE_ROWS("btech_turrets", counts.turrets);
  REQUIRE_ROWS("btech_autopilots", counts.autopilots);
  REQUIRE_ROWS("btech_mech_sections", counts.mechs * NUM_SECTIONS);
  REQUIRE_ROWS("btech_mech_criticals",
               counts.mechs * NUM_SECTIONS * NUM_CRITICALS);
  REQUIRE_ROWS("btech_mech_positions", counts.mechs);
  REQUIRE_ROWS("btech_mech_bays", counts.mechs * NUM_BAYS);
  REQUIRE_ROWS("btech_mech_turrets", counts.mechs * NUM_TURRETS);
  REQUIRE_ROWS("btech_mech_c3", counts.mechs);
  REQUIRE_ROWS("btech_mech_c3_nodes",
               counts.mechs * (C3I_NETWORK_SIZE + C3_NETWORK_SIZE));
  REQUIRE_ROWS("btech_mech_tics", counts.mechs * NUM_TICS * TICLONGS);
  REQUIRE_ROWS("btech_mech_frequencies", counts.mechs * FREQS);
  REQUIRE_ROWS("btech_mech_runtime", counts.mechs);
  REQUIRE_ROWS("btech_mech_runtime_unused", counts.mechs * 5);
#ifndef BT_CALCULATE_BV
  REQUIRE_ROWS("btech_mech_unit_aux", counts.mechs * 11);
#else
  REQUIRE_ROWS("btech_mech_unit_aux", counts.mechs * 4);
#endif
  REQUIRE_ROWS("btech_turret_tics", counts.turrets * NUM_TICS);
#undef REQUIRE_ROWS
  return 0;
}

/* Load every BTech table only after the normal special-object allocators run.
 */
int btech_special_load_all(sqlite3 *sqlite, BtechContext *context) {
#define BTECH_LOAD(stage, function)                                            \
  do {                                                                         \
    if ((function)(sqlite) < 0) {                                              \
      log_error(context->log, LOG_ALWAYS, "BTP", "FAIL",                       \
                "SQLite BTech validation failed at %s.", (char *)stage);       \
      return -1;                                                               \
    }                                                                          \
  } while (0)
#define BTECH_LOAD_CONTEXT(stage, function)                                    \
  do {                                                                         \
    if ((function)(sqlite, context) < 0) {                                     \
      log_error(context->log, LOG_ALWAYS, "BTP", "FAIL",                       \
                "SQLite BTech validation failed at %s.", (char *)stage);       \
      return -1;                                                               \
    }                                                                          \
  } while (0)
  BTECH_LOAD("metadata", btech_special_validate_metadata);
  BTECH_LOAD_CONTEXT("required rows", btech_special_validate_required_rows);
  BTECH_LOAD_CONTEXT("map parents", btech_special_load_map_parents);
  BTECH_LOAD_CONTEXT("map hexes", btech_special_load_map_hexes);
  BTECH_LOAD_CONTEXT("map slots", btech_special_load_map_slots);
  BTECH_LOAD_CONTEXT("map LOS", btech_special_load_map_los);
  BTECH_LOAD("map child counts", btech_special_validate_map_child_counts);
  BTECH_LOAD_CONTEXT("map objects", btech_special_load_map_objects);
  BTECH_LOAD_CONTEXT("map bits", btech_special_load_map_bits);
  BTECH_LOAD_CONTEXT("mech parents", btech_special_load_mech_parents);
  BTECH_LOAD_CONTEXT("mech sections", btech_special_load_mech_sections);
  BTECH_LOAD_CONTEXT("mech criticals", btech_special_load_mech_criticals);
  BTECH_LOAD_CONTEXT("mech positions", btech_special_load_mech_positions);
  BTECH_LOAD_CONTEXT("mech bays", btech_special_load_mech_bays);
  BTECH_LOAD_CONTEXT("mech turrets", btech_special_load_mech_turrets);
  BTECH_LOAD_CONTEXT("mech C3", btech_special_load_mech_c3);
  BTECH_LOAD_CONTEXT("mech C3 nodes", btech_special_load_mech_c3_nodes);
  BTECH_LOAD_CONTEXT("mech tics", btech_special_load_mech_tics);
  BTECH_LOAD_CONTEXT("mech frequencies", btech_special_load_mech_frequencies);
  BTECH_LOAD_CONTEXT("mech runtime", btech_special_load_mech_runtime);
  BTECH_LOAD_CONTEXT("mech unit auxiliary", btech_special_load_mech_unit_aux);
  BTECH_LOAD_CONTEXT("mech runtime auxiliary",
                     btech_special_load_mech_runtime_unused);
  BTECH_LOAD_CONTEXT("mech stagger damage",
                     btech_special_load_mech_stagger_damage);
  BTECH_LOAD_CONTEXT("mech repair consoles", btech_special_load_mechrep);
  BTECH_LOAD_CONTEXT("turrets", btech_special_load_turrets);
  BTECH_LOAD_CONTEXT("turret tics", btech_special_load_turret_tics);
  BTECH_LOAD_CONTEXT("autopilots", btech_special_load_autopilots);
  BTECH_LOAD_CONTEXT("autopilot commands",
                     btech_special_load_autopilot_commands);
  BTECH_LOAD_CONTEXT("autopilot paths", btech_special_load_autopilot_path);
  BTECH_LOAD_CONTEXT("repair events", btech_special_load_repair_events);
#undef BTECH_LOAD_CONTEXT
#undef BTECH_LOAD
  return 0;
}

/* Open the completed core snapshot for the post-core BTech restoration step. */
int btech_persistence_load_special_state_path(BtechContext *context,
                                              const char *path) {
  sqlite3 *sqlite;
  int result;

  sqlite = NULL;
  result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, NULL) != SQLITE_OK) {
    log_error(context->log, LOG_ALWAYS, "BTP", "FAIL",
              "Cannot open SQLite BTech state from %s: %s", (char *)path,
              sqlite ? sqlite3_errmsg(sqlite) : strerror(errno));
  } else if (btech_special_load_all(sqlite, context) < 0) {
    log_error(context->log, LOG_ALWAYS, "BTP", "FAIL",
              "Invalid or incomplete SQLite BTech state in %s: %s",
              (char *)path, sqlite3_errmsg(sqlite));
  } else {
    result = 0;
  }
  if (sqlite)
    sqlite3_close(sqlite);
  return result;
}
