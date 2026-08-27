#include <sqlite3.h>

#include <stdbool.h>
#include <stdint.h>

#include "btech/context.h"
#include "btech/persistence/sqlite_internal.h"
#include "btech_event.h"
#include "context_internal.h"
#include "equipment_types.h"
#include "mech_classification_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_specification_api.h"
#include "mech_tech_commands_api.h"
#include "mech_tech_events_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "registry_api.h"
#include "repair_job.h"
#include "special_object.h"

static MuxEventScheduler test_scheduler;
static BtechSpecialObject test_mech_object = {.type = GTYPE_MECH};
static BtechContext test_context_storage = {
    .events = &test_scheduler,
    .special_objects = (RedBlackTree)(uintptr_t)1,
};
static BtechContext *const test_context = &test_context_storage;
static Mech *const test_mech = (Mech *)&test_mech_object;

DbRef mech_dbref(const Mech *mech [[maybe_unused]]) { return 3; }
typedef struct ScheduledRequest {
  MuxEventCallback callback;
  void *data;
  intptr_t event_data;
  int type;
  int delay;
} ScheduledRequest;

static ScheduledRequest scheduled[16];
static int scheduled_count;
static MuxEventCallback scheduled_callback;
static int critical_count;
static int critical_type;
static bool critical_destroyed;
static bool section_destroyed;
static bool structural_placeholder;
static bool gun_position_valid;
static bool critical_nonfunctional;
static bool critical_damaged;
static bool critical_disabled;
static bool section_flooded;
static int critical_data_value;
static int ammunition_capacity = 10;
static int armor_value;
static int rear_armor_value;
static int internal_value;
static int original_armor_value = 10;
static int original_rear_armor_value = 10;
static int original_internal_value = 10;

static ScheduledRequest *scheduled_request_at(int index) {
  return checked_storage_at(scheduled, 16, sizeof(*scheduled), (size_t)index);
}

Mech *btech_context_get_mech(BtechContext *context, DbRef dbref) {
  return context == test_context && dbref == 3 ? test_mech : nullptr;
}
UnitClass mech_class(const Mech *mech [[maybe_unused]]) { return CLASS_BSUIT; }
int mech_maximum_battle_suits(const Mech *mech [[maybe_unused]]) { return 9; }
int mech_section_critical_count(Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return critical_count;
}
int mech_critical_part_type(const Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]],
                            int position [[maybe_unused]]) {
  return critical_type;
}
int mech_critical_data(const Mech *mech [[maybe_unused]],
                       int section [[maybe_unused]],
                       int position [[maybe_unused]]) {
  return critical_data_value;
}
bool mech_critical_is_destroyed(const Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]],
                                int position [[maybe_unused]]) {
  return critical_destroyed;
}
bool mech_critical_is_nonfunctional(const Mech *mech [[maybe_unused]],
                                    int section [[maybe_unused]],
                                    int position [[maybe_unused]]) {
  return critical_nonfunctional;
}
bool mech_critical_is_damaged(const Mech *mech [[maybe_unused]],
                              int section [[maybe_unused]],
                              int position [[maybe_unused]]) {
  return critical_damaged;
}
bool mech_section_is_destroyed(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]]) {
  return section_destroyed;
}
bool mech_section_is_flooded(const Mech *mech [[maybe_unused]],
                             int section [[maybe_unused]]) {
  return section_flooded;
}
bool mech_critical_is_disabled(const Mech *mech [[maybe_unused]],
                               int section [[maybe_unused]],
                               int position [[maybe_unused]]) {
  return critical_disabled;
}
int mech_section_armor(const Mech *mech [[maybe_unused]],
                       int section [[maybe_unused]]) {
  return armor_value;
}
int mech_section_rear_armor(const Mech *mech [[maybe_unused]],
                            int section [[maybe_unused]]) {
  return rear_armor_value;
}
int mech_section_internal(const Mech *mech [[maybe_unused]],
                          int section [[maybe_unused]]) {
  return internal_value;
}
int mech_section_original_armor(const Mech *mech [[maybe_unused]],
                                int section [[maybe_unused]]) {
  return original_armor_value;
}
int mech_section_original_rear_armor(const Mech *mech [[maybe_unused]],
                                     int section [[maybe_unused]]) {
  return original_rear_armor_value;
}
int mech_section_original_internal(const Mech *mech [[maybe_unused]],
                                   int section [[maybe_unused]]) {
  return original_internal_value;
}
int full_ammo(const Mech *mech [[maybe_unused]], int section [[maybe_unused]],
              int position [[maybe_unused]]) {
  return ammunition_capacity;
}
bool mech_part_is_structural_placeholder(int part_type [[maybe_unused]]) {
  return structural_placeholder;
}
bool valid_gun_pos(const RepairCriticalSelection *selection [[maybe_unused]]) {
  return gun_position_valid;
}
int get_weapon_crits(Mech *mech [[maybe_unused]],
                     int weapon_index [[maybe_unused]]) {
  return 1;
}
int mech_weapon_first_critical(const WeaponCriticalSearch *search) {
  return search->weapon.critical;
}
SplitCriticalLookup split_critical_find(Mech *mech [[maybe_unused]],
                                        CriticalSlotReference source
                                        [[maybe_unused]]) {
  return (SplitCriticalLookup){0};
}
int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail);
int btech_special_column_int(sqlite3_stmt *statement, int column, int *value);
int btech_special_column_long(sqlite3_stmt *statement, int column, long *value);
int btech_special_prepare_v2(sqlite3 *sqlite, const char *sql, int byte_count,
                             sqlite3_stmt **statement, const char **tail) {
  return sqlite3_prepare_v2(sqlite, sql, byte_count, statement, tail);
}
int btech_special_column_int(sqlite3_stmt *statement, int column, int *value) {
  *value = sqlite3_column_int(statement, column);
  return SQLITE_OK;
}
int btech_special_column_long(sqlite3_stmt *statement, int column,
                              long *value) {
  *value = sqlite3_column_int64(statement, column);
  return SQLITE_OK;
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
bool red_black_tree_walk(RedBlackTree tree [[maybe_unused]],
                         int how [[maybe_unused]], RedBlackTreeVisitor visitor,
                         void *context) {
  intptr_t key = 3;
  return visitor(&(RedBlackTreeVisitCall){
      .key = (void *)&key, .data = test_mech, .context = context});
}
void mux_event_add(const MuxEventRequest *request) {
  if (scheduled_count >= 16)
    return;
  *scheduled_request_at(scheduled_count) = (ScheduledRequest){
      .callback = request->callback,
      .data = request->data,
      .event_data = request->secondary.integer,
      .type = request->type,
      .delay = request->delay,
  };
  scheduled_count++;
  scheduled_callback = request->callback;
}
int mux_event_count_type(MuxEventScheduler *scheduler [[maybe_unused]],
                         int type) {
  int count = 0;
  for (int index = 0; index < scheduled_count; index++)
    if (scheduled_request_at(index)->type == type)
      count++;
  return count;
}
void mech_event_failure_marker(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_mountbomb(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_umountbomb(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairpart(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_replacegun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairenhcrit(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairgun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reattach(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reload(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairarmor(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_repairinternal(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_removesection(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_removegun(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_removepart(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_reseal(MuxEvent *event [[maybe_unused]]) {}
void mux_event_tickmech_replacesuit(MuxEvent *event [[maybe_unused]]) {}

int btech_special_load_repair_events(sqlite3 *sqlite, BtechContext *context);

static void reset_scheduled(void) {
  scheduled_count = 0;
  scheduled_callback = nullptr;
}

static bool insert_event(sqlite3 *sqlite, int type, int delay,
                         RepairEventPayload payload, bool fake) {
  sqlite3_stmt *statement = nullptr;
  bool result =
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (3, ?, ?, ?, ?);",
          -1, &statement, nullptr) == SQLITE_OK &&
      sqlite3_bind_int(statement, 1, type) == SQLITE_OK &&
      sqlite3_bind_int(statement, 2, delay) == SQLITE_OK &&
      sqlite3_bind_int64(statement, 3, repair_event_payload_pack(payload)) ==
          SQLITE_OK &&
      sqlite3_bind_int(statement, 4, fake) == SQLITE_OK &&
      sqlite3_step(statement) == SQLITE_DONE;
  sqlite3_finalize(statement);
  return result;
}

static bool clear_events(sqlite3 *sqlite) {
  reset_scheduled();
  return sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr,
                      nullptr, nullptr) == SQLITE_OK;
}

static bool load_repsuit_event(sqlite3 *sqlite, int location, int expected) {
  reset_scheduled();
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK)
    return false;
  sqlite3_stmt *statement = nullptr;
  if (sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (3, ?, 1, ?, 0);",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_bind_int(statement, 1, EVENT_REPAIR_REPSUIT) != SQLITE_OK ||
      sqlite3_bind_int(statement, 2, location) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_DONE) {
    sqlite3_finalize(statement);
    return false;
  }
  sqlite3_finalize(statement);
  int result = btech_special_load_repair_events(sqlite, test_context);
  return result == expected && scheduled_count == (expected == 0 ? 1 : 0);
}

static bool load_scrap_event(sqlite3 *sqlite, int type,
                             RepairEventPayload payload, int fake, int expected,
                             int expected_scheduled) {
  reset_scheduled();
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK)
    return false;
  sqlite3_stmt *statement = nullptr;
  if (sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (3, ?, 1, ?, ?);",
          -1, &statement, nullptr) != SQLITE_OK ||
      sqlite3_bind_int(statement, 1, type) != SQLITE_OK ||
      sqlite3_bind_int64(statement, 2, repair_event_payload_pack(payload)) !=
          SQLITE_OK ||
      sqlite3_bind_int(statement, 3, fake) != SQLITE_OK ||
      sqlite3_step(statement) != SQLITE_DONE) {
    sqlite3_finalize(statement);
    return false;
  }
  sqlite3_finalize(statement);
  int result = btech_special_load_repair_events(sqlite, test_context);
  return result == expected && scheduled_count == expected_scheduled;
}

static bool test_restore_lifecycle(sqlite3 *sqlite) {
  RepairEventPayload fix = {.location = 1, .player = 1073741824};
  RepairEventPayload fixi = {.location = 2, .player = 9};
  if (!repair_fix_event_payload_with_amount(&fix, 2) || !clear_events(sqlite))
    return false;
  armor_value = rear_armor_value = internal_value = 0;
  original_armor_value = original_rear_armor_value = original_internal_value =
      10;
  section_destroyed = section_flooded = false;
  if (!insert_event(sqlite, EVENT_REPAIR_FIX, 17, fix, false) ||
      !insert_event(sqlite, EVENT_REPAIR_FIXI, 9, fixi, true) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 2)
    return false;
  ScheduledRequest *first = scheduled_request_at(0);
  ScheduledRequest *second = scheduled_request_at(1);
  if (first->type != EVENT_REPAIR_FIX || first->delay != 17 ||
      first->callback != mux_event_tickmech_repairarmor ||
      first->data != test_mech ||
      first->event_data != repair_event_payload_pack(fix) ||
      second->type != EVENT_REPAIR_FIXI || second->delay != 9 ||
      second->callback != mech_event_failure_marker ||
      second->data != test_mech ||
      second->event_data != repair_event_payload_pack(fixi))
    return false;

  /* Loading an already-restored queue must fail without adding copies. */
  if (btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 2)
    return false;

  /* Identical work fingerprints are corrupt even if their delays differ. */
  RepairEventPayload duplicate_fix = {.location = 1, .player = 55};
  if (!repair_fix_event_payload_with_amount(&duplicate_fix, 1))
    return false;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 4, fix, false) ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 8, duplicate_fix, false) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;

  critical_count = 4;
  critical_type = special_equipment_index(HEAT_SINK);
  critical_destroyed = section_destroyed = section_flooded = false;
  critical_nonfunctional = true;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_REPAP, 4,
                    (RepairEventPayload){.location = 1, .position = 2},
                    false) ||
      !insert_event(sqlite, EVENT_REPAIR_SCRP, 8,
                    (RepairEventPayload){
                        .location = 1, .position = 2, .extra = 2, .player = 55},
                    false) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;

  /* Current topology changes make old safe coordinates stale, not corrupt. */
  critical_type = EMPTY;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_REPAP, 4,
                    (RepairEventPayload){.location = 1, .position = 2},
                    false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  critical_count = 2;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  critical_count = 4;

  /* Fake rows still require structurally and semantically valid payloads. */
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_REAT, 1,
                    (RepairEventPayload){.location = NUM_SECTIONS}, true) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 3, fix, false) ||
      !insert_event(sqlite, EVENT_REPAIR_REAT, 1,
                    (RepairEventPayload){.location = NUM_SECTIONS}, false) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 1,
                    (RepairEventPayload){.location = 1}, true) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1 ||
      scheduled_request_at(0)->callback != mech_event_failure_marker)
    return false;
  RepairEventPayload invalid_fake_fix = {.location = 1};
  if (!repair_fix_event_payload_with_amount(&invalid_fake_fix, 1) ||
      !clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 1, invalid_fake_fix, true) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;

  /* Section work must still be applicable when restored. */
  section_destroyed = false;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_REAT, 2,
                    (RepairEventPayload){.location = 1}, false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  section_destroyed = true;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1 ||
      scheduled_request_at(0)->callback != mux_event_tickmech_reattach)
    return false;
  reset_scheduled();
  section_destroyed = false;
  section_flooded = false;
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK ||
      !insert_event(sqlite, EVENT_REPAIR_RESE, 2,
                    (RepairEventPayload){.location = 1}, false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  section_flooded = true;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1 ||
      scheduled_request_at(0)->callback != mux_event_tickmech_reseal)
    return false;

  /* A partially completed stream remains actionable and stops at full. */
  reset_scheduled();
  armor_value = 9;
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK ||
      !insert_event(sqlite, EVENT_REPAIR_FIX, 1, fix, false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1)
    return false;
  reset_scheduled();
  armor_value = original_armor_value;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;

  /* Reload/unload rows are state-sensitive and reject stale credit work. */
  critical_count = 4;
  critical_type = ammunition_equipment_index(1);
  critical_destroyed = critical_disabled = false;
  critical_nonfunctional = false;
  section_destroyed = section_flooded = false;
  critical_data_value = 5;
  if (!clear_events(sqlite) ||
      !insert_event(
          sqlite, EVENT_REPAIR_RELO, 3,
          (RepairEventPayload){.location = 1, .position = 2, .extra = 2},
          false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1 ||
      scheduled_request_at(0)->callback != mux_event_tickmech_reload)
    return false;
  reset_scheduled();
  critical_type = special_equipment_index(HEAT_SINK);
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  critical_type = ammunition_equipment_index(1);
  critical_nonfunctional = true;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;

  /* Destroyed guns remain repairable, but cannot be scrapped twice. */
  critical_type = weapon_equipment_index(1);
  critical_destroyed = true;
  critical_nonfunctional = true;
  gun_position_valid = true;
  if (!clear_events(sqlite) ||
      !insert_event(sqlite, EVENT_REPAIR_REPAG, 3,
                    (RepairEventPayload){.location = 1, .position = 2},
                    false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 1 ||
      scheduled_request_at(0)->callback != mux_event_tickmech_repairgun)
    return false;
  reset_scheduled();
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK ||
      !insert_event(
          sqlite, EVENT_REPAIR_SCRG, 3,
          (RepairEventPayload){.location = 1, .position = 2, .extra = 2},
          false) ||
      btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  critical_nonfunctional = false;
  critical_data_value = 0;
  if (btech_special_load_repair_events(sqlite, test_context) != 0 ||
      scheduled_count != 0)
    return false;
  if (!clear_events(sqlite) ||
      !insert_event(
          sqlite, EVENT_REPAIR_RELO, 3,
          (RepairEventPayload){.location = 1, .position = 2, .extra = 2},
          true) ||
      btech_special_load_repair_events(sqlite, test_context) != -1 ||
      scheduled_count != 0)
    return false;
  return true;
}

static int repair_row_count(sqlite3 *sqlite) {
  sqlite3_stmt *statement = nullptr;
  int count = -1;
  if (sqlite3_prepare_v2(sqlite, "SELECT count(*) FROM btech_repair_events;",
                         -1, &statement, nullptr) == SQLITE_OK &&
      sqlite3_step(statement) == SQLITE_ROW)
    count = sqlite3_column_int(statement, 0);
  sqlite3_finalize(statement);
  return count;
}

static bool test_store_load_roundtrip(sqlite3 *sqlite) {
  if (!clear_events(sqlite))
    return false;
  armor_value = 0;
  original_armor_value = 10;
  RepairEventPayload payload = {.location = 1, .player = 99};
  if (!repair_fix_event_payload_with_amount(&payload, 2))
    return false;
  intptr_t event_data = repair_event_payload_pack(payload);
  test_scheduler.tick = 100;
  MuxEvent event = {
      .function = mux_event_tickmech_repairarmor,
      .data = test_mech,
      .secondary = {.kind = MUX_EVENT_PAYLOAD_INTEGER, .integer = event_data},
      .tick = 117,
      .type = EVENT_REPAIR_FIX,
      .scheduler = &test_scheduler};
  test_scheduler.events = &event;
  sqlite3_stmt *insert = nullptr;
  if (sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (?, ?, ?, ?, ?);",
          -1, &insert, nullptr) != SQLITE_OK)
    return false;
  BtechSpecialWriteContext fault = {0};
  bool passed =
      btech_special_store_repair_events(&fault, insert, test_context) == 0 &&
      repair_row_count(sqlite) == 1;
  sqlite3_finalize(insert);
  test_scheduler.events = nullptr;
  reset_scheduled();
  passed =
      passed && btech_special_load_repair_events(sqlite, test_context) == 0 &&
      scheduled_count == 1 && scheduled_request_at(0)->delay == 17 &&
      scheduled_request_at(0)->type == EVENT_REPAIR_FIX &&
      scheduled_request_at(0)->callback == mux_event_tickmech_repairarmor &&
      scheduled_request_at(0)->event_data == event_data;

  /* A completed target is stale, not corrupt: omit it from the snapshot. */
  armor_value = original_armor_value;
  reset_scheduled();
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK ||
      sqlite3_prepare_v2(
          sqlite,
          "INSERT INTO btech_repair_events "
          "(mech_dbref, event_type, remaining_ticks, event_data, is_fake) "
          "VALUES (?, ?, ?, ?, ?);",
          -1, &insert, nullptr) != SQLITE_OK)
    return false;
  test_scheduler.events = &event;
  passed =
      passed &&
      btech_special_store_repair_events(&fault, insert, test_context) == 0 &&
      repair_row_count(sqlite) == 0;

  /* Damage that makes queued gun work inapplicable is stale, not malformed. */
  critical_count = 4;
  critical_type = weapon_equipment_index(1);
  gun_position_valid = true;
  critical_nonfunctional = true;
  section_flooded = true;
  event.type = EVENT_REPAIR_REPAG;
  event.function = mux_event_tickmech_repairgun;
  event.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 2});
  passed =
      passed &&
      btech_special_store_repair_events(&fault, insert, test_context) == 0 &&
      repair_row_count(sqlite) == 0;
  section_flooded = false;

  critical_type = EMPTY;
  event.type = EVENT_REPAIR_REPAP;
  event.function = mux_event_tickmech_repairpart;
  event.secondary.integer = repair_event_payload_pack(
      (RepairEventPayload){.location = 1, .position = 2});
  passed =
      passed &&
      btech_special_store_repair_events(&fault, insert, test_context) == 0 &&
      repair_row_count(sqlite) == 0;

  critical_count = 2;
  critical_type = special_equipment_index(HEAT_SINK);
  passed =
      passed &&
      btech_special_store_repair_events(&fault, insert, test_context) == 0 &&
      repair_row_count(sqlite) == 0;
  critical_count = 4;

  /* The serializer must reject queues its loader would reject as duplicate. */
  armor_value = 0;
  event.type = EVENT_REPAIR_FIX;
  event.function = mux_event_tickmech_repairarmor;
  event.secondary.integer = event_data;
  RepairEventPayload duplicate_payload = {.location = 1, .player = 100};
  if (!repair_fix_event_payload_with_amount(&duplicate_payload, 1))
    return false;
  MuxEvent duplicate = event;
  duplicate.secondary.integer = repair_event_payload_pack(duplicate_payload);
  duplicate.tick = 120;
  duplicate.next_in_main = &event;
  event.prev_in_main = &duplicate;
  test_scheduler.events = &duplicate;
  passed = passed && btech_special_store_repair_events(&fault, insert,
                                                       test_context) == -1;
  if (sqlite3_exec(sqlite, "DELETE FROM btech_repair_events;", nullptr, nullptr,
                   nullptr) != SQLITE_OK)
    return false;
  event.prev_in_main = nullptr;
  test_scheduler.events = &event;

  /* Bad payload encoding still fails the snapshot rather than disappearing. */
  event.type = EVENT_REPAIR_FIX;
  event.function = mux_event_tickmech_repairarmor;
  event.secondary.integer = -1;
  int malformed_result =
      btech_special_store_repair_events(&fault, insert, test_context);
  passed = passed && malformed_result == -1 && repair_row_count(sqlite) == 0;
  test_scheduler.events = nullptr;
  sqlite3_finalize(insert);
  return passed;
}

int main(void) {
  sqlite3 *sqlite = nullptr;
  if (sqlite3_open(":memory:", &sqlite) != SQLITE_OK ||
      sqlite3_exec(sqlite,
                   "CREATE TABLE btech_repair_events (mech_dbref INTEGER, "
                   "event_type INTEGER, remaining_ticks INTEGER, "
                   "event_data INTEGER, is_fake INTEGER, event_id INTEGER "
                   "PRIMARY KEY);",
                   nullptr, nullptr, nullptr) != SQLITE_OK) {
    sqlite3_close(sqlite);
    return 1;
  }
  section_destroyed = true;
  bool passed = load_repsuit_event(sqlite, NUM_SECTIONS - 1, 0) &&
                load_repsuit_event(sqlite, NUM_SECTIONS, -1);
  section_destroyed = false;
  critical_count = 4;
  critical_type = special_equipment_index(HEAT_SINK);
  critical_destroyed = section_destroyed = structural_placeholder = false;
  critical_nonfunctional = critical_damaged = true;
  gun_position_valid = true;
  passed = passed &&
           load_scrap_event(
               sqlite, EVENT_REPAIR_SCRP,
               (RepairEventPayload){.location = 1, .position = 2, .extra = 3},
               1, 0, 1) &&
           load_scrap_event(
               sqlite, EVENT_REPAIR_SCRP,
               (RepairEventPayload){.location = 1, .position = 2, .extra = 1},
               1, -1, 0);
  critical_type = weapon_equipment_index(1);
  passed = passed &&
           load_scrap_event(
               sqlite, EVENT_REPAIR_SCRG,
               (RepairEventPayload){.location = 1, .position = 2, .extra = 2},
               0, 0, 1);
  passed = passed &&
           load_scrap_event(sqlite, EVENT_REPAIR_REPAG,
                            (RepairEventPayload){.location = 1, .position = 2},
                            0, 0, 1) &&
           scheduled_callback == mux_event_tickmech_repairgun;
  critical_type = special_equipment_index(HEAT_SINK);
  passed = passed &&
           load_scrap_event(sqlite, EVENT_REPAIR_REPAP,
                            (RepairEventPayload){.location = 1, .position = 2},
                            0, 0, 1) &&
           scheduled_callback == mux_event_tickmech_repairpart;
  section_destroyed = true;
  passed = passed &&
           load_scrap_event(sqlite, EVENT_REPAIR_REPAP,
                            (RepairEventPayload){.location = 1, .position = 2},
                            0, 0, 0);
  section_destroyed = false;
  critical_type = weapon_equipment_index(1);
  gun_position_valid = false;
  passed = passed &&
           load_scrap_event(
               sqlite, EVENT_REPAIR_SCRG,
               (RepairEventPayload){.location = 1, .position = 2, .extra = 2},
               0, 0, 0);
  bool lifecycle_passed = test_restore_lifecycle(sqlite);
  bool roundtrip_passed = test_store_load_roundtrip(sqlite);
  passed = passed && lifecycle_passed && roundtrip_passed;
  sqlite3_close(sqlite);
  return passed ? 0 : 1;
}
