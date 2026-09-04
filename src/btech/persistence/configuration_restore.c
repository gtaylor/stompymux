/* configuration_restore.c - Restore typed BTech configuration. */

#include "btech/configuration.h"
#include <stddef.h>

#include "context_internal.h" // IWYU pragma: keep
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "sqlite_internal.h"
#include <sqlite3.h>

static const char *column_text(sqlite3_stmt *statement, int column) {
  return sqlite3_column_type(statement, column) == SQLITE_NULL
             ? ""
             : (const char *)sqlite3_column_text(statement, column);
}

static void log_stale_configuration(BtechContext *context, const char *table,
                                    long object) {
  log_error((LogEntry){.log = context->log,
                       .key = LOG_ALWAYS,
                       .primary = "BTP",
                       .secondary = "SKIP"},
            "Skipping stale %s row for #%ld", table, object);
}

static void log_invalid_configuration(BtechContext *context, const char *table,
                                      long object) {
  log_error((LogEntry){.log = context->log,
                       .key = LOG_ALWAYS,
                       .primary = "BTP",
                       .secondary = "SKIP"},
            "Skipping invalid %s row for #%ld", table, object);
}

static void clear_unit_configuration(BtechContext *context, long object) {
  (void)btech_unit_preferred_id_set(context, object, nullptr);
  (void)btech_unit_display_name_set(context, object, nullptr);
  (void)btech_unit_markings_set(context, object, nullptr);
  (void)btech_unit_assigned_pilot_set(context, object, NOTHING);
}

static void clear_player_configuration(BtechContext *context, long player) {
  btech_player_ui_preferences_clear(context, player);
  (void)btech_player_mechwarrior_template_set(context, player, nullptr);
  btech_player_loadout_clear(context, player);
  (void)btech_repair_technician_available_at_set(context, player, 0);
}

static int load_units(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement = nullptr;
  if (btech_special_prepare_v2(
          sqlite,
          "SELECT object_dbref, preferred_id, display_name, markings, "
          "assigned_pilot FROM btech_unit_configuration ORDER BY object_dbref;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  int result = 0;
  int step;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long object;
    long pilot = NOTHING;
    if (btech_special_column_long(statement, 0, &object) < 0 ||
        (sqlite3_column_type(statement, 4) != SQLITE_NULL &&
         btech_special_column_long(statement, 4, &pilot) < 0)) {
      result = -1;
      break;
    }
    if (btech_context_get_mech(context, object) == nullptr) {
      log_stale_configuration(context, "unit configuration", object);
      continue;
    }
    if (pilot != NOTHING && (!is_good_obj(context->database, pilot) ||
                             !is_player(context->database, pilot) ||
                             is_going(context->database, pilot))) {
      log_stale_configuration(context, "assigned-pilot reference", pilot);
      pilot = NOTHING;
    }
    if (!btech_unit_preferred_id_set(context, object,
                                     column_text(statement, 1)) ||
        !btech_unit_display_name_set(context, object,
                                     column_text(statement, 2)) ||
        !btech_unit_markings_set(context, object, column_text(statement, 3)) ||
        !btech_unit_assigned_pilot_set(context, object, pilot)) {
      clear_unit_configuration(context, object);
      log_invalid_configuration(context, "unit configuration", object);
      continue;
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

static int load_players(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement = nullptr;
  if (btech_special_prepare_v2(
          sqlite,
          "SELECT player_dbref, has_ui, tactical_height, tactical_width, "
          "lrs_height, include_dead, include_shutdown, include_enemies, "
          "include_allies, include_target, buildings, mechwarrior_template, "
          "has_loadout, armor_head, armor_torso, armor_hands, armor_feet, "
          "right_weapon, left_weapon, has_right_ammunition, "
          "has_left_ammunition, right_ammunition, left_ammunition, "
          "technician_available_at FROM btech_player_configuration "
          "ORDER BY player_dbref;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  int result = 0;
  int step;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long player;
    int values[22];
    if (btech_special_column_long(statement, 0, &player) < 0) {
      result = -1;
      break;
    }
    if (!is_good_obj(context->database, player) ||
        !is_player(context->database, player) ||
        is_going(context->database, player)) {
      log_stale_configuration(context, "player configuration", player);
      continue;
    }
    for (int column = 1; column <= 10; column++) {
      if (btech_special_column_int(
              statement, column,
              checked_storage_at(values, 22, sizeof(*values),
                                 (size_t)(column - 1))) < 0)
        result = -1;
    }
    static const int LOADOUT_COLUMNS[] = {12, 13, 14, 15, 16, 19, 20, 21, 22};
    for (size_t index = 0;
         index < sizeof(LOADOUT_COLUMNS) / sizeof(*LOADOUT_COLUMNS); index++) {
      const int COLUMN = *(const int *)checked_storage_at_const(
          LOADOUT_COLUMNS, sizeof(LOADOUT_COLUMNS) / sizeof(*LOADOUT_COLUMNS),
          sizeof(*LOADOUT_COLUMNS), index);
      if (btech_special_column_int(
              statement, COLUMN,
              checked_storage_at(values, 22, sizeof(*values), 10 + index)) < 0)
        result = -1;
    }
    long available_at;
    if (result < 0 ||
        btech_special_column_long(statement, 23, &available_at) < 0)
      break;
    bool valid = true;
    if (values[0] && !btech_player_ui_preferences_set(
                         context, player,
                         (BtechPlayerUiPreferences){
                             .tactical_height = values[1],
                             .tactical_width = values[2],
                             .lrs_height = values[3],
                             .include_dead = values[4] != 0,
                             .include_shutdown = values[5] != 0,
                             .include_enemies = values[6] != 0,
                             .include_allies = values[7] != 0,
                             .include_target = values[8] != 0,
                             .buildings = (BtechBuildingContactMode)values[9]}))
      valid = false;
    if (*column_text(statement, 11) &&
        !btech_player_mechwarrior_template_set(context, player,
                                               column_text(statement, 11)))
      valid = false;
    if (values[10]) {
      BtechPersonalCombatLoadout loadout = {
          .armor_head = values[11],
          .armor_torso = values[12],
          .armor_hands = values[13],
          .armor_feet = values[14],
          .has_right_ammunition = values[15] != 0,
          .has_left_ammunition = values[16] != 0,
          .right_ammunition = values[17],
          .left_ammunition = values[18],
      };
      (void)string_copy_bounded(loadout.right_weapon,
                                sizeof(loadout.right_weapon),
                                column_text(statement, 17));
      (void)string_copy_bounded(loadout.left_weapon,
                                sizeof(loadout.left_weapon),
                                column_text(statement, 18));
      if (!btech_player_loadout_set(context, player, &loadout))
        valid = false;
    }
    if (!btech_repair_technician_available_at_set(context, player,
                                                  available_at))
      valid = false;
    if (!valid) {
      clear_player_configuration(context, player);
      log_invalid_configuration(context, "player configuration", player);
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

static int load_maps(sqlite3 *sqlite, BtechContext *context) {
  sqlite3_stmt *statement = nullptr;
  int result = 0;
  if (btech_special_prepare_v2(
          sqlite,
          "SELECT map_dbref, x, y, reveal_hint FROM "
          "btech_map_cargo_configuration ORDER BY map_dbref;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  int step;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long map;
    BtechCargoTransferPoint point;
    int hint;
    if (btech_special_column_long(statement, 0, &map) < 0 ||
        btech_special_column_int(statement, 1, &point.x) < 0 ||
        btech_special_column_int(statement, 2, &point.y) < 0 ||
        btech_special_column_int(statement, 3, &hint) < 0) {
      result = -1;
      break;
    }
    if (btech_context_get_map(context, map) == nullptr) {
      log_stale_configuration(context, "map cargo configuration", map);
      continue;
    }
    if (!btech_map_cargo_transfer_point_set(
            context, map,
            &(BtechCargoTransferPoint){
                .x = point.x, .y = point.y, .reveal_hint = hint != 0})) {
      log_invalid_configuration(context, "map cargo configuration", map);
      continue;
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  if (result < 0 ||
      btech_special_prepare_v2(
          sqlite,
          "SELECT child_dbref, parent_dbref, x, y FROM btech_map_links "
          "ORDER BY child_dbref;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long child;
    long parent;
    BtechMapLink link = {0};
    if (btech_special_column_long(statement, 0, &child) < 0 ||
        btech_special_column_long(statement, 1, &parent) < 0 ||
        btech_special_column_int(statement, 2, &link.x) < 0 ||
        btech_special_column_int(statement, 3, &link.y) < 0) {
      result = -1;
      break;
    }
    if (btech_context_get_map(context, child) == nullptr ||
        btech_context_get_map(context, parent) == nullptr) {
      log_stale_configuration(context, "map link", child);
      continue;
    }
    link.parent = parent;
    if (!btech_map_link_set(context, child, &link)) {
      log_invalid_configuration(context, "map link", child);
      continue;
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  if (result < 0 ||
      btech_special_prepare_v2(
          sqlite,
          "SELECT child_dbref, direction, mode, x, y, offset FROM "
          "btech_map_entrances ORDER BY child_dbref, direction;",
          -1, &statement, nullptr) != SQLITE_OK)
    return -1;
  while ((step = sqlite3_step(statement)) == SQLITE_ROW) {
    long child;
    int direction;
    if (btech_special_column_long(statement, 0, &child) < 0 ||
        btech_special_column_int(statement, 1, &direction) < 0) {
      result = -1;
      break;
    }
    if (direction < 0 || direction >= 4) {
      log_invalid_configuration(context, "map entrance", child);
      continue;
    }
    int mode;
    int x;
    int y;
    int offset;
    if (btech_special_column_int(statement, 2, &mode) < 0 ||
        btech_special_column_int(statement, 3, &x) < 0 ||
        btech_special_column_int(statement, 4, &y) < 0 ||
        btech_special_column_int(statement, 5, &offset) < 0) {
      result = -1;
      break;
    }
    if (mode < BTECH_MAP_ENTRANCE_NONE || mode > BTECH_MAP_ENTRANCE_EXACT) {
      log_invalid_configuration(context, "map entrance", child);
      continue;
    }
    BtechMapLink link;
    if (!btech_map_link(context, child, &link)) {
      log_stale_configuration(context, "map entrance", child);
      continue;
    }
    BtechMapEntrance *entrance = checked_storage_at(
        link.entrances, 4, sizeof(*link.entrances), (size_t)direction);
    entrance->mode = (BtechMapEntranceMode)mode;
    entrance->x = x;
    entrance->y = y;
    entrance->offset = offset;
    if (!btech_map_link_set(context, child, &link)) {
      log_invalid_configuration(context, "map entrance", child);
      continue;
    }
  }
  if (step != SQLITE_DONE)
    result = -1;
  sqlite3_finalize(statement);
  return result;
}

int btech_special_load_configurations(sqlite3 *sqlite, BtechContext *context) {
  return load_units(sqlite, context) < 0 || load_players(sqlite, context) < 0 ||
                 load_maps(sqlite, context) < 0
             ? -1
             : 0;
}

int btech_persistence_load_configurations_path(BtechContext *context,
                                               const char *path) {
  sqlite3 *sqlite = nullptr;
  int result = -1;
  if (sqlite3_open_v2(path, &sqlite, SQLITE_OPEN_READONLY, nullptr) !=
      SQLITE_OK) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Cannot open BTech configuration from %s", path);
  } else if (btech_special_validate_metadata(sqlite) < 0 ||
             btech_special_load_configurations(sqlite, context) < 0) {
    log_error((LogEntry){.log = context->log,
                         .key = LOG_ALWAYS,
                         .primary = "BTP",
                         .secondary = "FAIL"},
              "Invalid BTech configuration in %s: %s", path,
              sqlite3_errmsg(sqlite));
  } else {
    result = 0;
  }
  if (sqlite != nullptr)
    sqlite3_close(sqlite);
  return result;
}
