#include "context_internal.h"

#undef NDEBUG
#include <assert.h>
#include <limits.h>

#include "btech/persistence/sqlite_internal.h"
#include "mechrep.h"
#include "mux/objects/flags.h"
#include "registry_api.h"

#include <sqlite3.h>

static RepairFacility facility = {.xcode = {.type = GTYPE_MECHREP}};
static BtechSpecialObject mech_object = {.type = GTYPE_MECH};
static BtechSpecialObject non_mech_object = {.type = GTYPE_TURRET};

bool is_good_obj(GameDatabase *database [[maybe_unused]], DbRef object) {
  return object >= 0 && object <= INT_MAX && object != 4;
}

int btech_context_which_special(BtechContext *context [[maybe_unused]],
                                DbRef object) {
  if (object == 1)
    return GTYPE_MECHREP;
  if (object == 2 || object == 1073741824)
    return GTYPE_MECH;
  if (object == 3)
    return GTYPE_TURRET;
  return -1;
}

void *btech_context_find_object(BtechContext *context [[maybe_unused]],
                                DbRef object) {
  if (object == 1)
    return &facility;
  if (object == 2 || object == 1073741824)
    return &mech_object;
  if (object == 3)
    return &non_mech_object;
  return nullptr;
}

int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail) {
  return sqlite3_prepare_v2(sqlite, sql, byte_count, statement, tail);
}

int btech_special_column_long(sqlite3_stmt *statement, int column,
                              long *value) {
  sqlite3_int64 number;

  if (sqlite3_column_type(statement, column) != SQLITE_INTEGER)
    return -1;
  number = sqlite3_column_int64(statement, column);
  if (number < LONG_MIN || number > LONG_MAX)
    return -1;
  *value = (long)number;
  return 0;
}

int btech_special_column_dbref(GameDatabase *database, sqlite3_stmt *statement,
                               int column, DbRef *value) {
  if (btech_special_column_long(statement, column, value) < 0 ||
      (*value != NOTHING && !is_good_obj(database, *value)))
    return -1;
  return 0;
}

static void execute(sqlite3 *sqlite, const char *sql) {
  assert(sqlite3_exec(sqlite, sql, nullptr, nullptr, nullptr) == SQLITE_OK);
}

static void reset_rows(sqlite3 *sqlite, const char *target) {
  execute(sqlite, "DELETE FROM btech_mechrep;");
  char sql[128];
  assert(snprintf(sql, sizeof(sql), "INSERT INTO btech_mechrep VALUES (1, %s);",
                  target) > 0);
  execute(sqlite, sql);
}

int main(void) {
  GameObject objects[2] = {};
  GameDatabase database = {.object_storage = objects, .size = 1};
  BtechContext context = {.database = &database};
  sqlite3 *sqlite = nullptr;

  assert(sqlite3_open(":memory:", &sqlite) == SQLITE_OK);
  execute(sqlite, "CREATE TABLE btech_mechrep (dbref INTEGER PRIMARY KEY, "
                  "current_target INTEGER NOT NULL);");

  reset_rows(sqlite, "2");
  facility.current_target = NOTHING;
  assert(btech_special_load_mechrep(sqlite, &context) == 0);
  assert(facility.current_target == 2);

  reset_rows(sqlite, "1073741824");
  facility.current_target = NOTHING;
  assert(btech_special_load_mechrep(sqlite, &context) == 0);
  assert(facility.current_target == 1073741824);

  reset_rows(sqlite, "3");
  facility.current_target = 2;
  assert(btech_special_load_mechrep(sqlite, &context) == 0);
  assert(facility.current_target == NOTHING);

  reset_rows(sqlite, "4");
  facility.current_target = 2;
  assert(btech_special_load_mechrep(sqlite, &context) == 0);
  assert(facility.current_target == NOTHING);

  reset_rows(sqlite, "'bad'");
  assert(btech_special_load_mechrep(sqlite, &context) == -1);

  reset_rows(sqlite, "-2");
  assert(btech_special_load_mechrep(sqlite, &context) == -1);

  sqlite3_close(sqlite);
  return 0;
}
