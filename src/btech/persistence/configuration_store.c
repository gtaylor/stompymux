/* configuration_store.c - Store typed BTech configuration. */

#include <sqlite3.h>
#include <stddef.h>

#include "btech/configuration.h"
#include "configuration_internal.h"
#include "context_internal.h" // IWYU pragma: keep
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "sqlite_internal.h"

typedef struct ConfigurationStore {
  BtechContext *context;
  BtechSpecialWriteContext *fault;
  sqlite3_stmt *unit;
  sqlite3_stmt *player;
  sqlite3_stmt *cargo;
  sqlite3_stmt *link;
  sqlite3_stmt *entrance;
  int result;
} ConfigurationStore;

static int bind_text_or_null(sqlite3_stmt *statement, int index,
                             const char *text) {
  return text != nullptr && *text != '\0'
             ? sqlite3_bind_text(statement, index, text, -1, SQLITE_TRANSIENT)
             : sqlite3_bind_null(statement, index);
}

static bool store_configuration(const RedBlackTreeVisitCall *call) {
  ConfigurationStore *store = call->context;
  BtechConfigurationEntry *entry = call->data;
  if (store->result < 0)
    return false;
  if (!is_good_obj(store->context->database, entry->object) ||
      is_going(store->context->database, entry->object))
    return true;

  const DbRef ASSIGNED_PILOT =
      entry->assigned_pilot != NOTHING &&
              is_good_obj(store->context->database, entry->assigned_pilot) &&
              is_player(store->context->database, entry->assigned_pilot) &&
              !is_going(store->context->database, entry->assigned_pilot)
          ? entry->assigned_pilot
          : NOTHING;
  const bool IS_UNIT =
      btech_context_get_mech(store->context, entry->object) != nullptr;
  const bool IS_MAP =
      btech_context_get_map(store->context, entry->object) != nullptr;

  if (IS_UNIT && (*entry->preferred_id || entry->display_name ||
                  entry->markings || ASSIGNED_PILOT != NOTHING)) {
    if (btech_special_bind_int(store->unit, 1, entry->object) < 0 ||
        bind_text_or_null(store->unit, 2, entry->preferred_id) != SQLITE_OK ||
        bind_text_or_null(store->unit, 3, entry->display_name) != SQLITE_OK ||
        bind_text_or_null(store->unit, 4, entry->markings) != SQLITE_OK ||
        (ASSIGNED_PILOT == NOTHING
             ? sqlite3_bind_null(store->unit, 5)
             : btech_special_bind_int(store->unit, 5, ASSIGNED_PILOT)) !=
            SQLITE_OK ||
        btech_special_write_step(store->fault, store->unit) < 0)
      store->result = -1;
  }

  if (entry->has_ui_preferences || entry->mechwarrior_template ||
      entry->has_loadout || entry->technician_available_at != 0) {
    BtechPlayerUiPreferences *ui = &entry->ui_preferences;
    BtechPersonalCombatLoadout *loadout = &entry->loadout;
    if (btech_special_bind_int(store->player, 1, entry->object) < 0 ||
        btech_special_bind_int(store->player, 2, entry->has_ui_preferences) <
            0 ||
        btech_special_bind_int(store->player, 3, ui->tactical_height) < 0 ||
        btech_special_bind_int(store->player, 4, ui->tactical_width) < 0 ||
        btech_special_bind_int(store->player, 5, ui->lrs_height) < 0 ||
        btech_special_bind_int(store->player, 6, ui->include_dead) < 0 ||
        btech_special_bind_int(store->player, 7, ui->include_shutdown) < 0 ||
        btech_special_bind_int(store->player, 8, ui->include_enemies) < 0 ||
        btech_special_bind_int(store->player, 9, ui->include_allies) < 0 ||
        btech_special_bind_int(store->player, 10, ui->include_target) < 0 ||
        btech_special_bind_int(store->player, 11, ui->buildings) < 0 ||
        bind_text_or_null(store->player, 12, entry->mechwarrior_template) !=
            SQLITE_OK ||
        btech_special_bind_int(store->player, 13, entry->has_loadout) < 0 ||
        btech_special_bind_int(store->player, 14, loadout->armor_head) < 0 ||
        btech_special_bind_int(store->player, 15, loadout->armor_torso) < 0 ||
        btech_special_bind_int(store->player, 16, loadout->armor_hands) < 0 ||
        btech_special_bind_int(store->player, 17, loadout->armor_feet) < 0 ||
        bind_text_or_null(store->player, 18, loadout->right_weapon) !=
            SQLITE_OK ||
        bind_text_or_null(store->player, 19, loadout->left_weapon) !=
            SQLITE_OK ||
        btech_special_bind_int(store->player, 20,
                               loadout->has_right_ammunition) < 0 ||
        btech_special_bind_int(store->player, 21,
                               loadout->has_left_ammunition) < 0 ||
        btech_special_bind_int(store->player, 22, loadout->right_ammunition) <
            0 ||
        btech_special_bind_int(store->player, 23, loadout->left_ammunition) <
            0 ||
        btech_special_bind_int(store->player, 24,
                               entry->technician_available_at) < 0 ||
        btech_special_write_step(store->fault, store->player) < 0)
      store->result = -1;
  }

  if (IS_MAP && entry->has_cargo_transfer_point &&
      (btech_special_bind_int(store->cargo, 1, entry->object) < 0 ||
       btech_special_bind_int(store->cargo, 2, entry->cargo_transfer_point.x) <
           0 ||
       btech_special_bind_int(store->cargo, 3, entry->cargo_transfer_point.y) <
           0 ||
       btech_special_bind_int(store->cargo, 4,
                              entry->cargo_transfer_point.reveal_hint) < 0 ||
       btech_special_write_step(store->fault, store->cargo) < 0))
    store->result = -1;

  if (IS_MAP && entry->has_map_link &&
      is_good_obj(store->context->database, entry->map_link.parent) &&
      !is_going(store->context->database, entry->map_link.parent) &&
      btech_context_get_map(store->context, entry->map_link.parent) !=
          nullptr) {
    if (btech_special_bind_int(store->link, 1, entry->object) < 0 ||
        btech_special_bind_int(store->link, 2, entry->map_link.parent) < 0 ||
        btech_special_bind_int(store->link, 3, entry->map_link.x) < 0 ||
        btech_special_bind_int(store->link, 4, entry->map_link.y) < 0 ||
        btech_special_write_step(store->fault, store->link) < 0)
      store->result = -1;
    for (int direction = 0; direction < 4 && store->result == 0; direction++) {
      const BtechMapEntrance *entrance = checked_storage_at_const(
          entry->map_link.entrances, 4, sizeof(*entry->map_link.entrances),
          (size_t)direction);
      if (entrance->mode == BTECH_MAP_ENTRANCE_NONE)
        continue;
      if (btech_special_bind_int(store->entrance, 1, entry->object) < 0 ||
          btech_special_bind_int(store->entrance, 2, direction) < 0 ||
          btech_special_bind_int(store->entrance, 3, entrance->mode) < 0 ||
          btech_special_bind_int(store->entrance, 4, entrance->x) < 0 ||
          btech_special_bind_int(store->entrance, 5, entrance->y) < 0 ||
          btech_special_bind_int(store->entrance, 6, entrance->offset) < 0 ||
          btech_special_write_step(store->fault, store->entrance) < 0)
        store->result = -1;
    }
  }
  return store->result == 0;
}

int btech_special_store_configurations(BtechSpecialWriteContext *fault,
                                       sqlite3 *sqlite, BtechContext *context) {
  ConfigurationStore store = {.context = context, .fault = fault, .result = 0};
  if (btech_special_write_prepare(
          fault, sqlite,
          "INSERT INTO btech_unit_configuration VALUES (?, ?, ?, ?, ?);", -1,
          &store.unit, nullptr) != SQLITE_OK ||
      btech_special_write_prepare(
          fault, sqlite,
          "INSERT INTO btech_player_configuration VALUES (?, ?, ?, ?, ?, ?, "
          "?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);",
          -1, &store.player, nullptr) != SQLITE_OK ||
      btech_special_write_prepare(
          fault, sqlite,
          "INSERT INTO btech_map_cargo_configuration VALUES (?, ?, ?, ?);", -1,
          &store.cargo, nullptr) != SQLITE_OK ||
      btech_special_write_prepare(
          fault, sqlite, "INSERT INTO btech_map_links VALUES (?, ?, ?, ?);", -1,
          &store.link, nullptr) != SQLITE_OK ||
      btech_special_write_prepare(
          fault, sqlite,
          "INSERT INTO btech_map_entrances VALUES (?, ?, ?, ?, ?, ?);", -1,
          &store.entrance, nullptr) != SQLITE_OK)
    store.result = -1;
  if (store.result == 0)
    red_black_tree_walk(context->configurations, WALK_INORDER,
                        store_configuration, &store);
  sqlite3_finalize(store.unit);
  sqlite3_finalize(store.player);
  sqlite3_finalize(store.cargo);
  sqlite3_finalize(store.link);
  sqlite3_finalize(store.entrance);
  return store.result;
}
