#include <sqlite3.h>

#include <stdbool.h>
#include <stdint.h>

#include "btech/context.h"
#include "btech/persistence/sqlite_internal.h"
#include "btech_event.h"
#include "context_internal.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mux/network/mux_event.h"
#include "mux/support/red_black_tree.h"
#include "repair_job.h"
#include "special_object.h"

static BtechSpecialObject registered_object = {.type = GTYPE_MECH};
static Mech *const test_mech = (Mech *)&registered_object;
static void *registered_data = &registered_object;
static DbRef registered_dbref = 77;

DbRef mech_dbref(const Mech *mech [[maybe_unused]]) { return registered_dbref; }

static void canonical_fix(MuxEvent *event [[maybe_unused]]) {}
static void canonical_fixi(MuxEvent *event [[maybe_unused]]) {}
static void wrong_callback(MuxEvent *event [[maybe_unused]]) {}

MuxEventCallback btech_special_repair_function_for_type(int type) {
  if (type == EVENT_REPAIR_FIX)
    return canonical_fix;
  if (type == EVENT_REPAIR_FIXI)
    return canonical_fixi;
  return nullptr;
}

BtechRepairEventClassification btech_special_repair_event_classify(
    Mech *mech [[maybe_unused]], int event_type [[maybe_unused]],
    intptr_t event_data [[maybe_unused]], bool fake [[maybe_unused]]) {
  return BTECH_REPAIR_EVENT_ACTIONABLE;
}

bool btech_special_repair_events_conflict(int first_type [[maybe_unused]],
                                          intptr_t first_data [[maybe_unused]],
                                          int second_type [[maybe_unused]],
                                          intptr_t second_data
                                          [[maybe_unused]]) {
  return false;
}

void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}

bool red_black_tree_walk(RedBlackTree tree [[maybe_unused]],
                         int how [[maybe_unused]], RedBlackTreeVisitor visitor,
                         void *context) {
  return visitor(&(RedBlackTreeVisitCall){.key = (void *)registered_dbref,
                                          .data = registered_data,
                                          .context = context});
}

int btech_special_bind_int(sqlite3_stmt *statement, int index,
                           sqlite3_int64 value) {
  return sqlite3_bind_int64(statement, index, value) == SQLITE_OK ? 0 : -1;
}

int btech_special_write_step(BtechSpecialWriteContext *fault [[maybe_unused]],
                             sqlite3_stmt *statement) {
  int result = sqlite3_step(statement);
  if (result != SQLITE_DONE)
    return -1;
  return sqlite3_reset(statement) == SQLITE_OK ? 0 : -1;
}

static bool query_row(sqlite3 *sqlite, int offset, int expected_type,
                      int expected_ticks, intptr_t expected_data,
                      int expected_fake) {
  sqlite3_stmt *statement = nullptr;
  bool passed =
      sqlite3_prepare_v2(
          sqlite,
          "SELECT mech_dbref, event_type, remaining_ticks, event_data, is_fake "
          "FROM btech_repair_events ORDER BY event_id LIMIT 1 OFFSET ?;",
          -1, &statement, nullptr) == SQLITE_OK &&
      sqlite3_bind_int(statement, 1, offset) == SQLITE_OK &&
      sqlite3_step(statement) == SQLITE_ROW &&
      sqlite3_column_int(statement, 0) == registered_dbref &&
      sqlite3_column_int(statement, 1) == expected_type &&
      sqlite3_column_int(statement, 2) == expected_ticks &&
      sqlite3_column_int64(statement, 3) == expected_data &&
      sqlite3_column_int(statement, 4) == expected_fake;
  sqlite3_finalize(statement);
  return passed;
}

static bool prepare_database(sqlite3 **sqlite, sqlite3_stmt **insert) {
  return sqlite3_open(":memory:", sqlite) == SQLITE_OK &&
         sqlite3_exec(*sqlite,
                      "CREATE TABLE btech_repair_events ("
                      "event_id INTEGER PRIMARY KEY, mech_dbref INTEGER, "
                      "event_type INTEGER, remaining_ticks INTEGER, "
                      "event_data INTEGER, is_fake INTEGER);",
                      nullptr, nullptr, nullptr) == SQLITE_OK &&
         sqlite3_prepare_v2(
             *sqlite,
             "INSERT INTO btech_repair_events "
             "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
             "VALUES (?, ?, ?, ?, ?);",
             -1, insert, nullptr) == SQLITE_OK;
}

int main(void) {
  sqlite3 *sqlite = nullptr;
  sqlite3_stmt *insert = nullptr;
  if (!prepare_database(&sqlite, &insert)) {
    sqlite3_finalize(insert);
    sqlite3_close(sqlite);
    return 1;
  }

  MuxEventScheduler scheduler = {.tick = 100};
  BtechContext context = {
      .events = &scheduler,
      .special_objects = (RedBlackTree)(uintptr_t)1,
  };
  intptr_t fix_data = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 2, .player = 9});
  intptr_t fixi_data = repair_event_payload_pack(
      (RepairEventPayload){.location = 3, .position = 1, .player = 10});
  MuxEvent oldest = {.function = canonical_fix,
                     .data = test_mech,
                     .data2 = (void *)(intptr_t)fix_data,
                     .tick = 130,
                     .type = EVENT_REPAIR_FIX,
                     .scheduler = &scheduler};
  MuxEvent newer = {.function = mech_event_failure_marker,
                    .data = test_mech,
                    .data2 = (void *)(intptr_t)fixi_data,
                    .tick = 110,
                    .type = EVENT_REPAIR_FIXI,
                    .scheduler = &scheduler,
                    .next_in_main = &oldest};
  oldest.prev_in_main = &newer;
  intptr_t due_data = repair_event_payload_pack(
      (RepairEventPayload){.location = 4, .position = 1, .player = 11});
  MuxEvent at_due = {.function = canonical_fix,
                     .data = test_mech,
                     .data2 = (void *)(intptr_t)due_data,
                     .tick = scheduler.tick,
                     .type = EVENT_REPAIR_FIX,
                     .scheduler = &scheduler,
                     .next_in_main = &newer};
  newer.prev_in_main = &at_due;
  MuxEvent zombie = {.flags = FLAG_ZOMBIE,
                     .function = canonical_fix,
                     .data = (void *)(uintptr_t)1,
                     .tick = 105,
                     .type = EVENT_REPAIR_FIX,
                     .scheduler = &scheduler,
                     .next_in_main = &at_due};
  at_due.prev_in_main = &zombie;
  scheduler.events = &zombie;

  BtechSpecialWriteContext fault = {0};
  bool passed =
      btech_special_store_repair_events(&fault, insert, &context) == 0 &&
      query_row(sqlite, 0, EVENT_REPAIR_FIX, 30, fix_data, 0) &&
      query_row(sqlite, 1, EVENT_REPAIR_FIXI, 10, fixi_data, 1) &&
      query_row(sqlite, 2, EVENT_REPAIR_FIX, 1, due_data, 0);

  /* A new snapshot starts from an empty schema and reproduces the queue once.
   */
  passed = passed &&
           sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr,
                        nullptr, nullptr) == SQLITE_OK &&
           btech_special_store_repair_events(&fault, insert, &context) == 0 &&
           query_row(sqlite, 0, EVENT_REPAIR_FIX, 30, fix_data, 0) &&
           query_row(sqlite, 1, EVENT_REPAIR_FIXI, 10, fixi_data, 1) &&
           query_row(sqlite, 2, EVENT_REPAIR_FIX, 1, due_data, 0);

  /* Arbitrary callbacks under a repair type cannot be canonicalized on load. */
  MuxEvent invalid = {.function = wrong_callback,
                      .data = test_mech,
                      .tick = 101,
                      .type = EVENT_REPAIR_FIX,
                      .scheduler = &scheduler};
  scheduler.events = &invalid;
  passed = passed &&
           btech_special_store_repair_events(&fault, insert, &context) == -1;

  /* Native mech dbrefs avoid a registry walk for every queued event. */
  registered_data = &registered_object;
  registered_object.type = GTYPE_MAP;
  invalid.function = canonical_fix;
  invalid.data = test_mech;
  passed = passed &&
           btech_special_store_repair_events(&fault, insert, &context) == 0;

  sqlite3_finalize(insert);
  sqlite3_close(sqlite);
  return passed ? 0 : 1;
}
