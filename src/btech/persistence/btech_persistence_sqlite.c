/* btech_persistence_sqlite.c -- BTech state in the MUX SQLite game database */

#include "config.h"

#include <errno.h>
#include <sqlite3.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "externs.h"
#include "mech.h"
#include "p.mech.utils.h"
#include "persistence/btech_persistence.h"
#include "persistence/gamedb.h"

#ifdef BT_ADVANCED_ECON

/* One in-memory price array and the first part ID it represents. */
typedef struct btech_economy_cost_set BTECH_ECONOMY_COST_SET;
struct btech_economy_cost_set {
  unsigned long long *costs;
  size_t count;
  int first_part;
};

extern unsigned long long int specialcost[SPECIALCOST_SIZE];
extern unsigned long long int ammocost[AMMOCOST_SIZE];
extern unsigned long long int weapcost[WEAPCOST_SIZE];
extern unsigned long long int cargocost[CARGOCOST_SIZE];
extern unsigned long long int bombcost[BOMBCOST_SIZE];
extern char *part_figure_out_name(int part);
extern int temp_brand_flag;

/* Each array corresponds to one contiguous range of canonical part IDs. */
static BTECH_ECONOMY_COST_SET economy_cost_sets[] = {
    {specialcost, SPECIALCOST_SIZE, SPECIAL_BASE_INDEX},
    {ammocost, AMMOCOST_SIZE, AMMO_BASE_INDEX},
    {weapcost, WEAPCOST_SIZE, WEAPON_BASE_INDEX},
    {cargocost, CARGOCOST_SIZE, CARGO_BASE_INDEX},
    {bombcost, BOMBCOST_SIZE, BOMB_BASE_INDEX},
};

/* Execute a statement that does not return rows. */
static int btech_sqlite_exec(sqlite3 *sqlite, const char *sql) {
  char *error;
  int rc;

  error = NULL;
  rc = sqlite3_exec(sqlite, sql, NULL, NULL, &error);
  if (error)
    sqlite3_free(error);
  return rc == SQLITE_OK ? 0 : -1;
}

/* Bind and execute one reusable insert, leaving it ready for the next row. */
static int btech_sqlite_step(sqlite3_stmt *statement) {
  if (sqlite3_step(statement) != SQLITE_DONE ||
      sqlite3_reset(statement) != SQLITE_OK)
    return -1;
  sqlite3_clear_bindings(statement);
  return 0;
}

/* Return whether this snapshot contains the BTech economy extension table. */
static int btech_economy_table_exists(sqlite3 *sqlite, int *exists) {
  sqlite3_stmt *statement;
  int step;
  int result;

  statement = NULL;
  result = -1;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT 1 FROM sqlite_master WHERE type = 'table' "
                         "AND name = 'btech_economy_costs';",
                         -1, &statement, NULL) == SQLITE_OK) {
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
  int step;

  statement = NULL;
  *has_name = 0;
  result = -1;
  if (sqlite3_prepare_v2(sqlite, "PRAGMA table_info(btech_economy_costs);", -1,
                         &statement, NULL) == SQLITE_OK) {
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
  unsigned long long value;

  if (!text || text[0] == '-')
    return -1;
  errno = 0;
  value = strtoull((const char *)text, &end, 10);
  if (errno == ERANGE || end == (char *)text || *end)
    return -1;
  *cost = value;
  return 0;
}

/* Return an unbranded canonical name without needing runtime name hashes. */
static const char *btech_part_name(int part) {
  const char *item_name;
  int saved_brand_flag;

  saved_brand_flag = temp_brand_flag;
  temp_brand_flag = 0;
  item_name = part_figure_out_name(part);
  temp_brand_flag = saved_brand_flag;
  return item_name;
}

/* Resolve an unbranded canonical name without needing runtime name hashes. */
static int btech_part_from_name(const char *item_name, int *part) {
  BTECH_ECONOMY_COST_SET *cost_set;
  const char *candidate;
  size_t index;
  size_t item_index;
  int candidate_part;

  for (index = 0; index < sizeof(economy_cost_sets) / sizeof(economy_cost_sets[0]);
       index++) {
    cost_set = &economy_cost_sets[index];
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      candidate_part = cost_set->first_part + item_index;
      candidate = btech_part_name(candidate_part);
      if (candidate && !strcmp(item_name, candidate)) {
        *part = candidate_part;
        return 1;
      }
    }
  }
  return 0;
}

/* Restore sparse named prices, leaving omitted parts at the zero default. */
static int btech_load_costs(sqlite3 *sqlite) {
  sqlite3_stmt *statement;
  const unsigned char *part_name;
  unsigned long long cost;
  int part;
  int result;
  int skipped;
  int step;

  statement = NULL;
  result = -1;
  skipped = 0;
  if (sqlite3_prepare_v2(sqlite,
                         "SELECT item_name, cost FROM btech_economy_costs;",
                         -1, &statement, NULL) == SQLITE_OK) {
    result = 0;
    while (result == 0 && (step = sqlite3_step(statement)) == SQLITE_ROW) {
      part_name = sqlite3_column_text(statement, 0);
      if (!part_name ||
          !btech_part_from_name((const char *)part_name, &part)) {
        skipped++;
      } else if (btech_parse_cost(sqlite3_column_text(statement, 1), &cost) <
                 0) {
        result = -1;
      } else {
        SetPartCost(part, cost);
      }
    }
    if (result == 0 && step != SQLITE_DONE)
      result = -1;
  }
  sqlite3_finalize(statement);
  if (skipped)
    log_error(LOG_ALWAYS, "ECO", "INFO",
              "Ignored %d SQLite economy rows for parts unavailable in this "
              "build.",
              skipped);
  return result;
}

/* Restore economy prices from the SQLite game database. */
static int btech_persistence_load_economy(sqlite3 *sqlite) {
  size_t index;
  int exists;
  int has_item_name;

  for (index = 0; index < sizeof(economy_cost_sets) / sizeof(economy_cost_sets[0]);
       index++)
    memset(economy_cost_sets[index].costs, 0,
           economy_cost_sets[index].count *
               sizeof(*economy_cost_sets[index].costs));

  exists = 0;
  if (btech_economy_table_exists(sqlite, &exists) < 0)
    return -1;
  if (!exists) {
    log_error(LOG_ALWAYS, "ECO", "FAIL",
              "SQLite game database lacks required btech_economy_costs data.");
    return -1;
  }

  has_item_name = 0;
  if (btech_economy_table_has_item_name(sqlite, &has_item_name) < 0)
    return -1;
  if (!has_item_name) {
    log_error(LOG_ALWAYS, "ECO", "FAIL",
              "SQLite economy data lacks required item_name schema.");
    return -1;
  }

  return btech_load_costs(sqlite);
}

/* Write non-default advanced-economy prices in the core snapshot transaction. */
static int btech_persistence_store_economy(sqlite3 *sqlite) {
  BTECH_ECONOMY_COST_SET *cost_set;
  sqlite3_stmt *statement;
  const char *part_name;
  int part;
  size_t index;
  size_t item_index;
  char cost[32];
  int length;
  int result;

  statement = NULL;
  if (btech_sqlite_exec(
          sqlite, "CREATE TABLE btech_economy_costs ("
                  " item_name TEXT PRIMARY KEY,"
                  " cost TEXT NOT NULL"
                  ") WITHOUT ROWID;") < 0 ||
      sqlite3_prepare_v2(
          sqlite, "INSERT INTO btech_economy_costs (item_name, cost) "
                  "VALUES (?, ?);",
          -1, &statement, NULL) != SQLITE_OK)
    return -1;

  result = 0;
  for (index = 0;
       result == 0 && index < sizeof(economy_cost_sets) / sizeof(economy_cost_sets[0]);
       index++) {
    cost_set = &economy_cost_sets[index];
    for (item_index = 0; item_index < cost_set->count; item_index++) {
      if (!cost_set->costs[item_index])
        continue;
      part = cost_set->first_part + item_index;
      part_name = btech_part_name(part);
      length = snprintf(cost, sizeof(cost), "%llu", cost_set->costs[item_index]);
      if (!part_name || length < 0 || (size_t)length >= sizeof(cost) ||
          sqlite3_bind_text(statement, 1, part_name, -1,
                            SQLITE_TRANSIENT) != SQLITE_OK ||
          sqlite3_bind_text(statement, 2, cost, -1, SQLITE_TRANSIENT) !=
              SQLITE_OK ||
          btech_sqlite_step(statement) < 0) {
        result = -1;
        break;
      }
    }
  }
  sqlite3_finalize(statement);
  return result;
}
#endif

/* Register BTech's data tables without making core MUX depend on BTech data. */
int btech_persistence_register(void) {
#ifdef BT_ADVANCED_ECON
  return persistence_register_sqlite_extension(
      "btech_economy", btech_persistence_load_economy,
      btech_persistence_store_economy);
#else
  return 0;
#endif
}
