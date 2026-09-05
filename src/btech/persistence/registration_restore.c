/* registration_restore.c - Restore typed BTech special-object identity. */

#include "btech/persistence.h"

#include <sqlite3.h>
#include <string.h>

#include "btech/special_objects.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mux/objects/flags.h"
#include "mux/server/log.h"
#include "mux/server/server_config.h"
#include "registry_api.h"
#include "sqlite_internal.h"

static int registration_type(const char *name) {
  for (int type = 0; type < btech_special_object_type_count(); type++)
    if (!strcmp(name, btech_special_object_type_name(type)))
      return type;
  return -1;
}

static int load_registrations(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement = nullptr;
  if (btech_special_prepare_v2(
          sqlite,
          "SELECT dbref, special_type FROM btech_special_registrations "
          "ORDER BY dbref;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  int result = 0;
  int step;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long object;
    char name[16];
    if (btech_special_column_long(statement, 0, &object) < 0 ||
        btech_special_column_text(statement, 1, name, sizeof(name)) < 0) {
      result = -1;
      break;
    }
    if (!strcmp(name, "MECHREP"))
      continue;
    const int TYPE = registration_type(name);
    if (TYPE < 0) {
      result = -1;
      break;
    }
    if (!is_good_obj(context->database, object) ||
        !is_thing(context->database, object) ||
        is_going(context->database, object)) {
      log_error((LogEntry){.log = context->log,
                           .key = LOG_ALWAYS,
                           .primary = "BTP",
                           .secondary = "SKIP"},
                "Skipping stale BTech registration for #%ld", object);
      continue;
    }
    if (btech_context_find_object(context, object) != nullptr ||
        new_special_object(context, object, TYPE) == nullptr) {
      result = -1;
      break;
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

int btech_persistence_load_registrations_path(BtechContext *context,
                                              const char *path) {
  sqlite3 *sqlite = nullptr;
  int result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, nullptr) !=
      SQLITE_OK) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Cannot open BTech registrations from %s", path);
  } else if (btech_special_validate_metadata(sqlite) < 0 ||
             load_registrations(sqlite, context) < 0) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Invalid BTech registrations in %s: %s", path,
              sqlite3_errmsg(sqlite));
  } else {
    result = 0;
  }
  if (sqlite != nullptr)
    sqlite3_close(sqlite);
  return result;
}
