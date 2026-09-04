#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "context_internal.h"
#include "map.h"
#include "map_api.h"
#include "map_obj_api.h"
#include "mech_los_api.h"
#include "mech_position_api.h"
#include "mech_utils_api.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_helpers.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "script_functions_api.h"

static constexpr DbRef TEST_MAP = 2;
static constexpr DbRef TEST_RESOLVED_UNIT = 42;

static BattleMap map;
static bool map_loaded;
static bool map_units_cleared;
static bool map_objects_deleted;

static BtechScriptCall test_call(char *buffer, size_t capacity) {
  return (BtechScriptCall){
      .output = {.buffer = buffer, .cursor = buffer, .capacity = capacity}};
}

static char *buffer_at(char *buffer, size_t capacity, size_t offset) {
  return checked_storage_at(buffer, capacity, sizeof(char), offset);
}

static void check(bool condition) {
  if (!condition)
    abort();
}

DbRef match_thing(MatchContext *match [[maybe_unused]],
                  DbRef player [[maybe_unused]], char *name) {
  check(strcmp(name, "#2") == 0);
  return TEST_MAP;
}

bool is_good_obj(GameDatabase *database [[maybe_unused]], DbRef object) {
  return object == TEST_MAP;
}

bool btech_context_is_mech(BtechContext *context [[maybe_unused]],
                           DbRef key [[maybe_unused]]) {
  return false;
}

bool btech_context_is_map(BtechContext *context [[maybe_unused]], DbRef key) {
  return key == TEST_MAP;
}

BattleMap *btech_context_get_map(BtechContext *context [[maybe_unused]],
                                 DbRef object) {
  return object == TEST_MAP ? &map : nullptr;
}

Mech *btech_context_get_mech(BtechContext *context [[maybe_unused]],
                             DbRef object [[maybe_unused]]) {
  return nullptr;
}

DbRef find_target_dbref_from_map_number(Mech *mech [[maybe_unused]],
                                        const char *map_number
                                        [[maybe_unused]]) {
  abort();
}

DbRef find_mech_on_map(BattleMap *target_map, const char *id) {
  check(target_map == &map);
  check(strcmp(id, "AA") == 0);
  return TEST_RESOLVED_UNIT;
}

int mech_los_check_unblocked(Mech *mech [[maybe_unused]],
                             Mech *target [[maybe_unused]],
                             int x [[maybe_unused]], int y [[maybe_unused]],
                             float range [[maybe_unused]]) {
  abort();
}

int mech_position_x(const Mech *mech [[maybe_unused]]) { abort(); }

int mech_position_y(const Mech *mech [[maybe_unused]]) { abort(); }

float mech_range_to(const Mech *mech [[maybe_unused]],
                    const Mech *target [[maybe_unused]]) {
  abort();
}

int map_checkmapfile(BattleMap *target_map, char *map_name) {
  check(target_map == &map);
  check(strcmp(map_name, "fixture") == 0);
  return 1;
}

int map_load(BattleMap *target_map, char *map_name) {
  check(target_map == &map);
  check(strcmp(map_name, "fixture") == 0);
  map_loaded = true;
  return 0;
}

void map_clearmechs(DbRef player, void *data, const char *buffer) {
  check(player == GOD);
  check(data == &map);
  check(strcmp(buffer, "") == 0);
  map_units_cleared = true;
}

void del_mapobjs(BattleMap *target_map) {
  check(target_map == &map);
  map_objects_deleted = true;
}

static EvaluationContext test_evaluation(GameDatabase *database,
                                         BtechContext *btech,
                                         WorldContext *world,
                                         CommandContext *command) {
  GameObject *god = game_database_object(database, GOD);
  god->has_wizard_flag = true;
  game_database_object(database, TEST_MAP)->type = OBJECT_TYPE_THING;
  btech->database = database;
  world->database = database;
  return (EvaluationContext){
      .btech = btech,
      .world = world,
      .command = command,
  };
}

static void test_error_status_is_explicit(void) {
  char buffer[32] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 BAD INPUT");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-1 BAD INPUT") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), strlen(buffer)));
}

static void test_sentinel_text_can_be_a_success(void) {
  char buffer[32] = "#-1";
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  call.output.cursor = buffer_at(buffer, sizeof(buffer), strlen(buffer));
  BtechScriptResult result =
      btech_script_result_finish(&call, BTECH_SCRIPT_TEXT);

  check(result.status == BTECH_SCRIPT_OK);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-1") == 0);
}

static void test_error_respects_output_capacity(void) {
  char buffer[6] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 TOO LONG");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(strcmp(buffer, "#-1 T") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), sizeof(buffer) - 1));
}

static void test_error_handles_one_byte_output(void) {
  char buffer[1] = {};
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  BtechScriptResult result = btech_script_error(&call, "#-1 TOO LONG");

  check(result.status == BTECH_SCRIPT_ERROR);
  check(strcmp(buffer, "") == 0);
  check(call.output.cursor == buffer);
}

static void test_preformatted_error_status_is_explicit(void) {
  char buffer[32] = "#-7 FORMATTED";
  BtechScriptCall call = test_call(buffer, sizeof(buffer));
  call.output.cursor = buffer_at(buffer, sizeof(buffer), strlen(buffer));
  BtechScriptResult result = btech_script_error_output(&call);

  check(result.status == BTECH_SCRIPT_ERROR);
  check(result.kind == BTECH_SCRIPT_TEXT);
  check(strcmp(result.value.text, "#-7 FORMATTED") == 0);
  check(call.output.cursor ==
        buffer_at(buffer, sizeof(buffer), strlen(buffer)));
}

static void test_handlers_preserve_observed_numeric_anomalies(void) {
  GameObject objects[4] = {};
  GameDatabase database = {
      .object_storage = objects,
      .top = 3,
      .size = 3,
  };
  BtechContext btech = {};
  WorldContext world = {};
  CommandContext command = {};
  EvaluationContext evaluation =
      test_evaluation(&database, &btech, &world, &command);

  char load_buffer[32] = {};
  char load_map[] = "#2";
  char load_name[] = "fixture";
  char *load_arguments[] = {load_map, load_name};
  BtechScriptCall load_call = test_call(load_buffer, sizeof(load_buffer));
  load_call.evaluation = &evaluation;
  load_call.player = GOD;
  load_call.arguments = (BtechScriptArguments){
      .values = load_arguments,
      .count = sizeof(load_arguments) / sizeof(load_arguments[0]),
  };
  BtechScriptResult load_result = fun_btloadmap(&load_call);

  // btech.map.load deliberately documents this adapter result as literal 1.
  check(load_result.status == BTECH_SCRIPT_OK);
  check(load_result.kind == BTECH_SCRIPT_NUMBER);
  check(fabs(load_result.value.number - 1.0) < 0.000001);
  check(map_loaded);
  check(map_units_cleared);
  check(map_objects_deleted);

  char dbref_buffer[32] = {};
  char dbref_map[] = "#2";
  char dbref_id[] = "AA";
  char *dbref_arguments[] = {dbref_map, dbref_id};
  BtechScriptCall dbref_call = test_call(dbref_buffer, sizeof(dbref_buffer));
  dbref_call.evaluation = &evaluation;
  dbref_call.player = GOD;
  dbref_call.arguments = (BtechScriptArguments){
      .values = dbref_arguments,
      .count = sizeof(dbref_arguments) / sizeof(dbref_arguments[0]),
  };
  BtechScriptResult dbref_result = fun_btid2db(&dbref_call);

  // btech.map.id_to_dbref emits leading-# text, which strtod converts to 0.
  check(dbref_result.status == BTECH_SCRIPT_OK);
  check(dbref_result.kind == BTECH_SCRIPT_NUMBER);
  check(fabs(dbref_result.value.number) < 0.000001);
}

int main(void) {
  test_error_status_is_explicit();
  test_sentinel_text_can_be_a_success();
  test_error_respects_output_capacity();
  test_error_handles_one_byte_output();
  test_preformatted_error_status_is_explicit();
  test_handlers_preserve_observed_numeric_anomalies();
  return 0;
}
