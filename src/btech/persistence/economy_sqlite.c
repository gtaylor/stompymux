#include "btech/context.h"
#include "context_internal.h" // IWYU pragma: keep
#include "missile_hit_registry.h"
#include "mux/persistence/gamedb.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "mux/support/utf8.h"
#include "part_cost_api.h"
#include "sqlite_internal.h"

#include <errno.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/support/checked_storage.h"
#include "template_api.h"

static BtechPartCostSet *cost_set_at(BtechPartCostSet *sets, size_t index) {
  return checked_storage_at(sets, BTECH_PART_COST_SET_COUNT, sizeof(*sets),
                            index);
}

static unsigned long long cost_at(const BtechPartCostSet *set, size_t index) {
  const unsigned long long *cost = checked_storage_at_const(
      set->costs, set->count, sizeof(*set->costs), index);
  return *cost;
}

/* Execute a statement that does not return rows. */
static int btech_sqlite_exec(sqlite3 *sqlite, const char *sql) {
  char *error;
  int rc;

  error = nullptr;
  rc = sqlite3_exec(sqlite, sql, nullptr, nullptr, &error);
  if (error)
    sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

/* Return whether this snapshot contains the BTech economy extension table. */
static int btech_economy_table_exists(sqlite3 *sqlite, int *exists) {
  sqlite3_stmt *statement;
  int step;
  int result;

  statement = nullptr;
  result = -1;
  if (btech_special_prepare_v2(
          sqlite,
          "SELECT 1 FROM sqlite_master WHERE type = 'table' "
          "AND name = 'btech_economy_costs';",
          -1, &statement, nullptr) == SQLITE_OK) {
    step = sqlite3_step(statement);
    if (step == SQLITE_ROW || step == SQLITE_DONE) {
      *exists = step == SQLITE_ROW;
      result = 0;
    }
  }
  sqlite3_finalize(statement);
  return result;
}

/* Return whether an existing table uses the current name-keyed schema. */
static int btech_economy_table_has_item_name(sqlite3 *sqlite, int *has_name) {
  sqlite3_stmt *statement;
  const unsigned char *column;
  int result;
  int step = SQLITE_DONE;

  statement = nullptr;
  *has_name = 0;
  result = -1;
  if (btech_special_prepare_v2(sqlite,
                               "PRAGMA table_info(btech_economy_costs);", -1,
                               &statement, nullptr) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      column = sqlite3_column_text(statement, 1);
      if (column && !strcmp((const char *)column, "item_name"))
        *has_name = 1;
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  return result;
}

/* Read a decimal SQLite value without narrowing the unsigned 64-bit price. */
static int btech_parse_cost(const unsigned char *text,
                            unsigned long long *cost) {
  char *end;
  const char *start;
  unsigned long long value;

  if (!text || text[0] == '-')
    return -1;
  errno = 0;
  start = (const char *)text;
  value = strtoull(start, &end, 10);
  if (errno == ERANGE || end == start || *end)
    return -1;
  *cost = value;
  return 0;
}

/* Return an unbranded canonical name without needing runtime name hashes. */
static const char *btech_part_name(const ServerConfiguration *configuration,
                                   int part,
                                   char buffer[static BTECH_TEXT_CAPACITY]) {
  return part_name_format(&(PartNameRequest){
      .configuration = configuration, .part = part, .buffer = buffer});
}

/* Resolve an unbranded canonical name without needing runtime name hashes. */
static bool btech_part_from_name(BtechContext *btech, const char *item_name,
                                 int *part) {
  BtechPartCostSet cost_sets[BTECH_PART_COST_SET_COUNT];
  BtechPartCostSet *cost_set;
  const char *candidate;
  size_t index;
  size_t item_index;
  int candidate_part;
  char candidate_name[BTECH_TEXT_CAPACITY];

  btech_part_cost_sets(btech, cost_sets);
  for (index = 0; index < BTECH_PART_COST_SET_COUNT; index++) {
    cost_set = cost_set_at(cost_sets, index);
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      if (cost_set->first_part > INT_MAX ||
          item_index > (size_t)(INT_MAX - cost_set->first_part))
        continue;
      candidate_part = cost_set->first_part + (int)item_index;
      candidate =
          btech_part_name(btech->configuration, candidate_part, candidate_name);
      if (candidate && !strcmp(item_name, candidate)) {
        *part = candidate_part;
        return true;
      }
    }
  }
  return false;
}

/* Restore sparse named prices, leaving omitted parts at the zero default. */
static int btech_load_costs(sqlite3 *sqlite, BtechContext *btech) {
  sqlite3_stmt *statement;
  const unsigned char *part_name;
  unsigned long long cost;
  int part;
  int result;
  int skipped;
  int step = SQLITE_DONE;

  statement = nullptr;
  result = -1;
  skipped = 0;
  if (btech_special_prepare_v2(
          sqlite, "SELECT item_name, cost FROM btech_economy_costs;", -1,
          &statement, nullptr) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      part_name = sqlite3_column_text(statement, 0);
      if (!part_name ||
          !utf8_validate((const char *)part_name,
                         (size_t)sqlite3_column_bytes(statement, 0)) ||
          !btech_part_from_name(btech, (const char *)part_name, &part)) {
        skipped++;
      } else if (btech_parse_cost(sqlite3_column_text(statement, 1), &cost) <
                 0) {
        result = -1;
      } else {
        btech_part_cost_set(btech, part, cost);
      }
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  if (skipped) {
    log_error((LogEntry){.log = btech->log,
                         .key = LOG_ALWAYS,
                         .primary = "ECO",
                         .secondary = "INFO"},
              "Ignored %d SQLite economy rows for parts unavailable in this "
              "build.",
              skipped);
  }
  return result;
}

/* Restore economy prices from the SQLite game database. */
int btech_persistence_load_economy(sqlite3 *sqlite,
                                   PersistenceContext *persistence
                                   [[maybe_unused]],
                                   void *extension_context) {
  BtechContext *btech = extension_context;
  int exists;
  int has_item_name;

  btech_part_costs_reset(btech);

  exists = 0;
  if (btech_economy_table_exists(sqlite, &exists) < 0)
    return -1;
  if (!exists) {
    log_error((LogEntry){.log = btech->log,
                         .key = LOG_ALWAYS,
                         .primary = "ECO",
                         .secondary = "FAIL"},
              "SQLite game database lacks required btech_economy_costs data.");
    return -1;
  }

  has_item_name = 0;
  if (btech_economy_table_has_item_name(sqlite, &has_item_name) < 0)
    return -1;
  if (!has_item_name) {
    log_error((LogEntry){.log = btech->log,
                         .key = LOG_ALWAYS,
                         .primary = "ECO",
                         .secondary = "FAIL"},
              "SQLite economy data lacks required item_name schema.");
    return -1;
  }

  return btech_load_costs(sqlite, btech);
}

/* Write non-default advanced-economy prices in the core snapshot transaction.
 */
int btech_persistence_store_economy(sqlite3 *sqlite,
                                    PersistenceContext *persistence
                                    [[maybe_unused]],
                                    void *extension_context) {
  BtechContext *btech = extension_context;
  BtechPartCostSet cost_sets[BTECH_PART_COST_SET_COUNT];
  BtechPartCostSet *cost_set;
  BtechSpecialWriteContext fault;
  sqlite3_stmt *statement;
  const char *part_name;
  int part;
  size_t index;
  size_t item_index;
  char cost[32];
  char generated_name[BTECH_TEXT_CAPACITY];
  int length;
  int result;

  statement = nullptr;
  /* This writer runs as a separate registered extension from the special-state
   * writer, so it owns an independent fault-injection context. */
  btech_special_write_context_init(&fault);
  if (btech_sqlite_exec(sqlite, "CREATE TABLE btech_economy_costs ("
                                " item_name TEXT PRIMARY KEY,"
                                " cost TEXT NOT NULL"
                                ") WITHOUT ROWID;") < 0 ||
      btech_special_write_prepare(
          &fault, sqlite,
          "INSERT INTO btech_economy_costs (item_name, cost) "
          "VALUES (?, ?);",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;

  result = 0;
  btech_part_cost_sets(btech, cost_sets);
  for (index = 0; result == 0 && index < BTECH_PART_COST_SET_COUNT; index++) {
    cost_set = cost_set_at(cost_sets, index);
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      const unsigned long long ITEM_COST = cost_at(cost_set, item_index);
      if (!ITEM_COST)
        continue;
      if (cost_set->first_part > INT_MAX ||
          item_index > (size_t)(INT_MAX - cost_set->first_part)) {
        result = -1;
        break;
      }
      part = cost_set->first_part + (int)item_index;
      part_name = btech_part_name(btech->configuration, part, generated_name);
      length = snprintf(cost, sizeof(cost), "%llu", ITEM_COST);
      if (!part_name || length < 0 || (size_t)length >= sizeof(cost) ||
          sqlite3_bind_text(statement, 1, part_name, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          sqlite3_bind_text(statement, 2, cost, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          btech_special_write_step(&fault, statement) < 0) {
        result = -1;
        break;
      }
    }
  }
  sqlite3_finalize(statement);
  return result;
}
/* Register BTech's data tables without making core MUX depend on BTech data. */
int btech_persistence_register(PersistenceContext *context,
                               BtechContext *btech) {
  if (persistence_register_sqlite_extension(
          context, "btech_special_state", nullptr,
          btech_persistence_store_special_state, btech) < 0)
    return -1;
  btech_part_costs_initialize(btech);
  return persistence_register_sqlite_extension(
      context, "btech_economy", btech_persistence_load_economy,
      btech_persistence_store_economy, btech);
}
