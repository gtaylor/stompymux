#include "mech_movement_validation_api.h"

#include <assert.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "map_terrain.h"
#include "mech_crew_api.h"
#include "mech_identity_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_startup_api.h"
#include "mech_utils_api.h"
#include "registry_api.h"

struct BattleMap {
  int width;
  int height;
};

struct Mech {
  BtechContext *context;
  DbRef map_dbref;
  DbRef pilot_dbref;
  DbRef dbref;
  int x;
  int y;
  int previous_x;
  int previous_y;
  bool jumping;
  int cocoon_integrity;
};

static BtechContext *context = (BtechContext *)(uintptr_t)1;
static BattleMap current_map;
static BattleMap fallback_map;
static bool current_map_available;
static bool fallback_map_available;
static int valid_map_calls;
static bool land_called;
static bool shutdown_called;
static DbRef last_channel_map;
static char notification[128];

BtechContext *mech_context(const Mech *mech) { return mech->context; }

DbRef mech_map_dbref(const Mech *mech) { return mech->map_dbref; }

DbRef mech_pilot_dbref(const Mech *mech) { return mech->pilot_dbref; }

DbRef mech_dbref(const Mech *mech) { return mech->dbref; }

void mech_map_dbref_set(Mech *mech, DbRef map_dbref) {
  mech->map_dbref = map_dbref;
}

BattleMap *btech_context_get_map(BtechContext *value [[maybe_unused]],
                                 DbRef dbref) {
  if (current_map_available && dbref == 7)
    return &current_map;
  return nullptr;
}

BattleMap *valid_map(const MapValidationRequest *request) {
  valid_map_calls++;
  return fallback_map_available && request->map == 7 ? &fallback_map : nullptr;
}

bool battle_map_coordinate_is_valid(const BattleMap *map, int x, int y) {
  return x >= 0 && y >= 0 && x < map->width && y < map->height;
}

int mech_position_x(const Mech *mech) { return mech->x; }

int mech_position_y(const Mech *mech) { return mech->y; }

int mech_position_previous_x(const Mech *mech) { return mech->previous_x; }

int mech_position_previous_y(const Mech *mech) { return mech->previous_y; }

bool mech_is_jumping(const Mech *mech) { return mech->jumping; }

void mech_cocoon_integrity_set(Mech *mech, int integrity) {
  mech->cocoon_integrity = integrity;
}

int mech_cocoon_integrity(const Mech *mech) { return mech->cocoon_integrity; }

void mech_notify(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *message) {
  (void)snprintf(notification, sizeof(notification), "%s", message);
}

void mech_land(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
               char *buffer [[maybe_unused]]) {
  land_called = true;
}

void mech_shutdown(DbRef player [[maybe_unused]], Mech *mech [[maybe_unused]],
                   const char *buffer [[maybe_unused]]) {
  shutdown_called = true;
}

void btech_channel_send(BtechContext *value [[maybe_unused]],
                        BtechChannel channel [[maybe_unused]],
                        const char *format, ...) {
  va_list arguments;
  va_start(arguments, format);
  (void)vsnprintf(notification, sizeof(notification), format, arguments);
  va_end(arguments);
  last_channel_map = 7;
}

static Mech fresh_mech(void) {
  return (Mech){.context = context,
                .map_dbref = 7,
                .pilot_dbref = 12,
                .dbref = 42,
                .x = 1,
                .y = 1,
                .previous_x = 0,
                .previous_y = 0,
                .cocoon_integrity = 3};
}

static void reset_fixture(void) {
  current_map = (BattleMap){.width = 5, .height = 5};
  fallback_map = (BattleMap){.width = 8, .height = 8};
  current_map_available = true;
  fallback_map_available = true;
  valid_map_calls = 0;
  land_called = false;
  shutdown_called = false;
  last_channel_map = -1;
  notification[0] = '\0';
}

static void test_valid_current_map(void) {
  reset_fixture();
  Mech mech = fresh_mech();
  assert(mech_movement_map_validate(&mech) == &current_map);
  assert(valid_map_calls == 0);
  assert(mech.map_dbref == 7);
  assert(mech.cocoon_integrity == 3);
  assert(!shutdown_called);
}

static void test_fallback_map(void) {
  reset_fixture();
  current_map_available = false;
  Mech mech = fresh_mech();
  assert(mech_movement_map_validate(&mech) == &fallback_map);
  assert(valid_map_calls == 1);
  assert(!shutdown_called);
}

static void test_invalid_position_resets_unit(void) {
  reset_fixture();
  Mech mech = fresh_mech();
  mech.x = current_map.width;
  assert(mech_movement_map_validate(&mech) == nullptr);
  assert(valid_map_calls == 0);
  assert(mech.map_dbref == -1);
  assert(mech.cocoon_integrity == 0);
  assert(shutdown_called);
  assert(!land_called);
  assert(strcmp(notification, "move_mech:invalid map:Mech: 42 Index: 7") == 0);
}

static void test_invalid_jump_lands_before_shutdown(void) {
  reset_fixture();
  Mech mech = fresh_mech();
  mech.jumping = true;
  mech.previous_y = current_map.height;
  assert(mech_movement_map_validate(&mech) == nullptr);
  assert(land_called);
  assert(shutdown_called);
  assert(mech.map_dbref == -1);
}

static void test_missing_map_without_pilot(void) {
  reset_fixture();
  current_map_available = false;
  fallback_map_available = false;
  Mech mech = fresh_mech();
  mech.pilot_dbref = -1;
  assert(mech_movement_map_validate(&mech) == nullptr);
  assert(valid_map_calls == 0);
  assert(mech.map_dbref == -1);
  assert(shutdown_called);
}

int main(void) {
  test_valid_current_map();
  test_fallback_map();
  test_invalid_position_resets_unit();
  test_invalid_jump_lands_before_shutdown();
  test_missing_map_without_pilot();
  return 0;
}
