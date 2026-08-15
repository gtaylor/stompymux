/* object_state.c -- Typed object-state and transaction tests. */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/server/server_config.h"

bool is_good_obj(GameDatabase *database, DbRef object);

bool is_good_obj(GameDatabase *database, DbRef object) {
  return object >= 0 && object < database->top &&
         game_object_type(database, object) != OBJECT_TYPE_GARBAGE;
}

static ObjectStateValue string_value(const char *value) {
  return (ObjectStateValue){
      .type = OBJECT_STATE_STRING,
      .as.string = {.data = value, .length = strlen(value)},
  };
}

static bool state_string_equals(GameDatabase *database, DbRef object,
                                const char *name_space, const char *key,
                                const char *expected) {
  const ObjectStateValue *value =
      object_state_get(database, object, name_space, key);
  const size_t EXPECTED_LENGTH = strlen(expected);
  return value && value->type == OBJECT_STATE_STRING &&
         value->as.string.length == EXPECTED_LENGTH &&
         memcmp(value->as.string.data, expected, EXPECTED_LENGTH) == 0;
}

static int check_committed_values(GameDatabase *database) {
  const ObjectStateValue *empty = object_state_get(database, 0, "bank", "memo");
  const ObjectStateValue *balance =
      object_state_get(database, 0, "bank", "balance");
  const ObjectStateValue *enabled =
      object_state_get(database, 0, "bank", "enabled");
  const ObjectStateValue *rate = object_state_get(database, 1, "bank", "rate");

  return empty && empty->type == OBJECT_STATE_STRING &&
         empty->as.string.length == 0 && balance &&
         balance->type == OBJECT_STATE_INTEGER && balance->as.integer == 1250 &&
         enabled && enabled->type == OBJECT_STATE_BOOLEAN &&
         enabled->as.boolean && rate && rate->type == OBJECT_STATE_NUMBER &&
         fabs(rate->as.number - 1.25) < 0.000001;
}

int main(void) {
  GameObject objects[3] = {0};
  GameDatabase database = {
      .object_storage = objects,
      .top = 2,
      .size = 2,
  };
  ServerConfiguration configuration = {0};
  ObjectStateTransaction transaction;
  char error[256];

  configuration.lua.state_value_limit = 16;
  configuration.lua.state_entry_limit = 8;
  configuration.lua.state_object_limit = 256;
  database.configuration = &configuration;
  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_PLAYER);

  if (!object_state_name_is_valid("bank.account") ||
      object_state_name_is_valid("9bank") ||
      object_state_name_is_valid("bank account"))
    return 1;

  object_state_transaction_initialize(&transaction);
  if (!object_state_transaction_begin(&transaction, &database))
    return 1;
  ObjectStateValue discarded = {
      .type = OBJECT_STATE_INTEGER,
      .as.integer = 999,
  };
  if (!object_state_transaction_set(&transaction, 0, "bank", "balance",
                                    &discarded, error, sizeof(error)))
    return 1;
  object_state_transaction_finish(&transaction, false);
  if (object_state_get(&database, 0, "bank", "balance"))
    return 1;

  if (!object_state_transaction_begin(&transaction, &database))
    return 1;
  ObjectStateValue empty = string_value("");
  ObjectStateValue balance = {
      .type = OBJECT_STATE_INTEGER,
      .as.integer = 1250,
  };
  ObjectStateValue enabled = {
      .type = OBJECT_STATE_BOOLEAN,
      .as.boolean = true,
  };
  ObjectStateValue rate = {
      .type = OBJECT_STATE_NUMBER,
      .as.number = 1.25,
  };
  if (!object_state_transaction_set(&transaction, 0, "bank", "memo", &empty,
                                    error, sizeof(error)) ||
      !object_state_transaction_set(&transaction, 0, "bank", "balance",
                                    &balance, error, sizeof(error)) ||
      !object_state_transaction_set(&transaction, 0, "bank", "enabled",
                                    &enabled, error, sizeof(error)) ||
      !object_state_transaction_set(&transaction, 1, "bank", "rate", &rate,
                                    error, sizeof(error)))
    return 1;
  if (object_state_transaction_count(&transaction, 0, "bank") != 3)
    return 1;
  object_state_transaction_finish(&transaction, true);
  if (!check_committed_values(&database))
    return 1;

  if (!object_state_transaction_begin(&transaction, &database))
    return 1;
  if (!object_state_transaction_delete(&transaction, 0, "bank", "balance"))
    return 1;
  object_state_transaction_finish(&transaction, false);
  if (!check_committed_values(&database))
    return 1;

  char middle_text[] = "middle";
  ObjectStateValue middle = string_value(middle_text);
  ObjectStateValue alpha = string_value("alpha");
  ObjectStateValue omega = string_value("omega");
  if (!object_state_set(&database, 0, "owned", "middle", &middle, error,
                        sizeof(error)) ||
      !object_state_set(&database, 0, "owned", "alpha", &alpha, error,
                        sizeof(error)) ||
      !object_state_set(&database, 0, "owned", "omega", &omega, error,
                        sizeof(error)))
    return 1;
  middle_text[0] = 'X';
  if (!state_string_equals(&database, 0, "owned", "middle", "middle") ||
      !state_string_equals(&database, 0, "owned", "alpha", "alpha") ||
      !state_string_equals(&database, 0, "owned", "omega", "omega"))
    return 1;

  if (!object_state_copy(&database, 1, 0))
    return 1;
  ObjectStateValue updated = string_value("updated");
  if (!object_state_set(&database, 0, "owned", "middle", &updated, error,
                        sizeof(error)) ||
      !object_state_delete(&database, 0, "owned", "alpha"))
    return 1;
  if (!state_string_equals(&database, 0, "owned", "middle", "updated") ||
      object_state_get(&database, 0, "owned", "alpha") != nullptr ||
      !state_string_equals(&database, 1, "owned", "middle", "middle") ||
      !state_string_equals(&database, 1, "owned", "alpha", "alpha"))
    return 1;

  if (!object_state_transaction_begin(&transaction, &database))
    return 1;
  ObjectStateValue rolled_back = string_value("rolledback");
  if (!object_state_transaction_set(&transaction, 0, "owned", "omega",
                                    &rolled_back, error, sizeof(error)))
    return 1;
  object_state_transaction_finish(&transaction, false);
  if (!state_string_equals(&database, 0, "owned", "omega", "omega"))
    return 1;

  object_state_clear(&database, 1);
  configuration.lua.state_object_limit = 32;
  ObjectStateValue six_bytes = string_value("123456");
  ObjectStateValue one_byte = string_value("x");
  if (!object_state_set(&database, 1, "quota", "first", &six_bytes, error,
                        sizeof(error)) ||
      object_state_set(&database, 1, "quota", "second", &six_bytes, error,
                       sizeof(error)) ||
      !strstr(error, "exceeds 32 bytes"))
    return 1;
  if (!object_state_set(&database, 1, "quota", "first", &one_byte, error,
                        sizeof(error)) ||
      !object_state_set(&database, 1, "quota", "second", &six_bytes, error,
                        sizeof(error)) ||
      !object_state_delete(&database, 1, "quota", "first") ||
      !object_state_set(&database, 1, "quota", "third", &one_byte, error,
                        sizeof(error)))
    return 1;

  ObjectStateValue too_large = string_value("0123456789abcdefg");
  if (object_state_set(&database, 0, "bank", "oversize", &too_large, error,
                       sizeof(error)) ||
      !strstr(error, "exceeds 16 bytes"))
    return 1;

  configuration.lua.state_entry_limit = 3;
  if (object_state_set(&database, 0, "bank", "extra", &enabled, error,
                       sizeof(error)) ||
      !strstr(error, "exceeds 3 entries"))
    return 1;

  configuration.lua.state_entry_limit = 8;
  configuration.lua.state_object_limit = 29;
  if (object_state_set(&database, 1, "bank", "other", &enabled, error,
                       sizeof(error)) ||
      !strstr(error, "exceeds 29 bytes"))
    return 1;

  object_state_transaction_destroy(&transaction);
  object_state_clear(&database, 0);
  object_state_clear(&database, 1);
  return 0;
}
