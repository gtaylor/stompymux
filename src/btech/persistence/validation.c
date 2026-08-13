#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "missile_hit_registry.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "mux/support/red_black_tree.h"
#include "special_object.h"
#include "sqlite_internal.h"
#include <errno.h>
#include <stdio.h>
#include <string.h>

typedef struct BtechSpecialObjectCounts BtechSpecialObjectCounts;
struct BtechSpecialObjectCounts {
  int maps;
  int mechs;
  int mechreps;
  int turrets;
  int autopilots;
};

/* Count normal BTech instances, excluding DEBUG and other non-persisted types.
 */
static int btech_special_count_objects(const RedBlackTreeVisitCall *call) {
  void *key = call->key;
  void *data = call->data;
  int depth = call->depth;
  void *argument = call->context;
  BtechSpecialObjectCounts *counts = argument;
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
  case GTYPE_DEBUG:
  case GTYPE_UNUSED1:
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
  result = btech_special_prepare_v2(sqlite, sql, -1, &statement, NULL) ==
                       SQLITE_OK &&
                   sqlite3_step(statement) == SQLITE_ROW &&
                   btech_special_column_int(statement, 0, count) == 0 &&
                   sqlite3_step(statement) == SQLITE_DONE
               ? 0
               : -1;
  sqlite3_finalize(statement);
  return result;
}

static int btech_special_require_rows(sqlite3 *sqlite, const char *table,
                                      int expected) {
  int actual;
  return expected < 0 ||
                 btech_special_table_count(sqlite, table, &actual) < 0 ||
                 actual != expected
             ? -1
             : 0;
}

/* Require one parent and every fixed child row for each preallocated object. */
static int btech_special_validate_required_rows(sqlite3 *sqlite,
                                                BtechContext *context) {
  BtechSpecialObjectCounts counts = {0, 0, 0, 0, 0};

  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      btech_special_count_objects, &counts);
  if (btech_special_require_rows(sqlite, "btech_maps", counts.maps) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mechs", counts.mechs) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mechrep", counts.mechreps) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_turrets", counts.turrets) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_autopilots",
                                 counts.autopilots) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_sections",
                                 counts.mechs * NUM_SECTIONS) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_criticals",
                                 counts.mechs * NUM_SECTIONS * NUM_CRITICALS) <
      0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_positions", counts.mechs) <
      0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_bays",
                                 counts.mechs * NUM_BAYS) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_turrets",
                                 counts.mechs * NUM_TURRETS) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_c3", counts.mechs) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_c3_nodes",
                                 counts.mechs *
                                     (C3I_NETWORK_SIZE + C3_NETWORK_SIZE)) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_tics",
                                 counts.mechs * NUM_TICS * TICLONGS) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_frequencies",
                                 counts.mechs * FREQS) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_runtime", counts.mechs) <
      0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_mech_unit_aux",
                                 counts.mechs * 4) < 0)
    return -1;
  if (btech_special_require_rows(sqlite, "btech_turret_tics",
                                 counts.turrets * NUM_TICS) < 0)
    return -1;
  return 0;
}

typedef int (*BtechSqliteLoader)(sqlite3 *sqlite);
typedef int (*BtechSqliteContextLoader)(sqlite3 *sqlite, BtechContext *context);

static int btech_special_load_stage(sqlite3 *sqlite, BtechContext *context,
                                    const char *stage,
                                    BtechSqliteLoader loader) {
  if (loader(sqlite) >= 0)
    return 0;
  log_error((LogEntry){.log = context->log,
                       .key = LOG_ALWAYS,
                       .primary = "BTP",
                       .secondary = "FAIL"},
            "SQLite BTech validation failed at %s.", stage);
  return -1;
}

static int btech_special_load_context_stage(sqlite3 *sqlite,
                                            BtechContext *context,
                                            const char *stage,
                                            BtechSqliteContextLoader loader) {
  if (loader(sqlite, context) >= 0)
    return 0;
  log_error((LogEntry){.log = context->log,
                       .key = LOG_ALWAYS,
                       .primary = "BTP",
                       .secondary = "FAIL"},
            "SQLite BTech validation failed at %s.", stage);
  return -1;
}

/* Load every BTech table only after the normal special-object allocators run.
 */
static int btech_special_load_all(sqlite3 *sqlite, BtechContext *context) {
  if (btech_special_load_stage(sqlite, context, "metadata",
                               btech_special_validate_metadata) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "required rows",
                                       btech_special_validate_required_rows) <
      0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map parents",
                                       btech_special_load_map_parents) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map hexes",
                                       btech_special_load_map_hexes) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map slots",
                                       btech_special_load_map_slots) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map LOS",
                                       btech_special_load_map_los) < 0)
    return -1;
  if (btech_special_load_stage(sqlite, context, "map child counts",
                               btech_special_validate_map_child_counts) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map objects",
                                       btech_special_load_map_objects) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "map bits",
                                       btech_special_load_map_bits) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech parents",
                                       btech_special_load_mech_parents) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech sections",
                                       btech_special_load_mech_sections) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech criticals",
                                       btech_special_load_mech_criticals) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech positions",
                                       btech_special_load_mech_positions) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech bays",
                                       btech_special_load_mech_bays) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech turrets",
                                       btech_special_load_mech_turrets) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech C3",
                                       btech_special_load_mech_c3) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech C3 nodes",
                                       btech_special_load_mech_c3_nodes) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech tics",
                                       btech_special_load_mech_tics) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech frequencies",
                                       btech_special_load_mech_frequencies) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech runtime",
                                       btech_special_load_mech_runtime) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech unit auxiliary",
                                       btech_special_load_mech_unit_aux) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech stagger damage",
                                       btech_special_load_mech_stagger_damage) <
      0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "mech repair consoles",
                                       btech_special_load_mechrep) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "turrets",
                                       btech_special_load_turrets) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "turret tics",
                                       btech_special_load_turret_tics) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "autopilots",
                                       btech_special_load_autopilots) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "autopilot commands",
                                       btech_special_load_autopilot_commands) <
      0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "autopilot paths",
                                       btech_special_load_autopilot_path) < 0)
    return -1;
  if (btech_special_load_context_stage(sqlite, context, "repair events",
                                       btech_special_load_repair_events) < 0)
    return -1;
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
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Cannot open SQLite BTech state from %s: %s", path,
              sqlite ? sqlite3_errmsg(sqlite) : strerror(errno));
  } else if (btech_special_load_all(sqlite, context) < 0) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Invalid or incomplete SQLite BTech state in %s: %s", path,
              sqlite3_errmsg(sqlite));
  } else {
    result = 0;
  }
  if (sqlite)
    sqlite3_close(sqlite);
  return result;
}
