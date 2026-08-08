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
