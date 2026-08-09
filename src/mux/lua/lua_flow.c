/* lua.c - Lua runtime initialization and MUX integration. */

#include <lauxlib.h>
#include <limits.h>
#include <linux/limits.h>
#include <lua.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mux/lua/command_access.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/objects/db.h"
#include "mux/server/log.h"
#include "mux/server/maintenance.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

constexpr int LUA_FLOW_MAX_FIELDS = 16;
constexpr int LUA_FLOW_KEY_SIZE = 32;

typedef struct LuaFlowField {
  char key[LUA_FLOW_KEY_SIZE];
  char *value;
} LuaFlowField;

typedef struct LuaFlowData {
  LuaOwner *runtime_owner;
  LUA_MODULE_ROOT root;
  char path[PATH_MAX];
  LuaFlowField fields[LUA_FLOW_MAX_FIELDS];
  int field_count;
} LuaFlowData;

static LuaFlowField *lua_flow_field_at(LuaFlowData *data, size_t index) {
  return checked_storage_at(data->fields, LUA_FLOW_MAX_FIELDS,
                            sizeof(*data->fields), index);
}

static void lua_flow_data_clear_fields(LuaFlowData *data) {
  int index;

  for (index = 0; index < data->field_count; index++) {
    LuaFlowField *field = lua_flow_field_at(data, (size_t)index);

    free_lbuf(field->value);
    field->value = nullptr;
  }
  data->field_count = 0;
}

static void lua_flow_data_free(void *flow_data) {
  LuaFlowData *data = flow_data;

  lua_flow_data_clear_fields(data);
  free(data);
}

/* Decode the flat, C-owned scratch store into a fresh ctx.flow table. This
 * (rather than a Lua registry reference) is what lets a flow survive
 * @lua/reload rebuilding the whole lua_State out from under it. */
static void lua_flow_decode(lua_State *state, LuaFlowData *data) {
  int index;

  lua_newtable(state);
  for (index = 0; index < data->field_count; index++) {
    LuaFlowField *field = lua_flow_field_at(data, (size_t)index);

    lua_pushstring(state, field->value);
    lua_setfield(state, -2, field->key);
  }
  lua_setfield(state, -2, "flow");
}

/* Harvest ctx.flow (at the given stack index) back into the scratch store.
 * Only string/number values keyed by strings round-trip; anything else is
 * dropped with a log message. */
static void lua_flow_encode(LuaRuntime *runtime, lua_State *state,
                            int flow_table_index, LuaFlowData *data) {
  lua_flow_data_clear_fields(data);
  lua_pushnil(state);
  while (lua_next(state, flow_table_index) != 0) {
    if (lua_type(state, -2) == LUA_TSTRING &&
        (lua_isstring(state, -1) || lua_isnumber(state, -1)) &&
        data->field_count < LUA_FLOW_MAX_FIELDS) {
      LuaFlowField *field = lua_flow_field_at(data, (size_t)data->field_count);

      StringCopyTrunc(field->key, lua_tostring(state, -2),
                      LUA_FLOW_KEY_SIZE - 1);
      field->value = alloc_lbuf("lua_flow_field");
      StringCopyTrunc(field->value, lua_tostring(state, -1), LBUF_SIZE - 1);
      data->field_count++;
    } else if (lua_type(state, -2) == LUA_TSTRING) {
      log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
                "Dropping unsupported ctx.flow.%s (must be a string or "
                "number).",
                lua_tostring(state, -2));
    }
    lua_pop(state, 1);
  }
}

static FlowOutcome lua_flow_step(Descriptor *d, void *flow_data,
                                 const char *step, const char *input) {
  static char prompt_buffer[LBUF_SIZE];
  LuaFlowData *data = flow_data;
  LuaRuntime *runtime =
      data->runtime_owner != nullptr ? data->runtime_owner->runtime : nullptr;
  lua_State *state;
  FlowOutcome outcome = {.action = FLOW_ACTION_CANCEL};
  LUA_MODULE_ROOT previous_root;
  char error[LBUF_SIZE];
  const char *field;
  int top;
  int ctx_index;
  int result_index;
  int status;

  if (!runtime) {
    outcome.prompt = "The Lua runtime is unavailable.\r\n";
    return outcome;
  }
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, data->root, data->path, error, sizeof(error))) {
    lua_log_load_error(runtime, d->player, data->path, error);
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, -1, "flows");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, -1, step);
  if (!lua_isfunction(state, -1)) {
    log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
              "Unknown flow step '%s' in %s.", step, data->path);
    lua_settop(state, top);
    return outcome;
  }

  lua_push_context(runtime->services->database, d, state, NOTHING, d->player,
                   d->player, nullptr, nullptr, "flow", nullptr, 0);
  if (input != nullptr) {
    lua_pushstring(state, input);
    lua_setfield(state, -2, "input");
  }
  lua_flow_decode(state, data);
  ctx_index = lua_gettop(state);
  lua_pushvalue(state, ctx_index);
  lua_insert(state, ctx_index - 1);
  previous_root = runtime->current_root;
  runtime->current_root = data->root;
  status = lua_callback_pcall_checked(runtime, 1, 1);
  runtime->current_root = previous_root;
  if (status) {
    lua_log_error(runtime, d->player, "FLOW", lua_tostring(state, -1));
    lua_settop(state, top);
    outcome.prompt = "A script error interrupted this flow.\r\n";
    return outcome;
  }

  result_index = lua_gettop(state);
  ctx_index = result_index - 1;
  lua_getfield(state, ctx_index, "flow");
  lua_flow_encode(runtime, state, lua_gettop(state), data);
  lua_pop(state, 1);

  if (!lua_istable(state, result_index)) {
    lua_settop(state, top);
    return outcome;
  }
  lua_getfield(state, result_index, "action");
  field = lua_tostring(state, -1);
  if (!field || strcmp(field, "repeat") == 0)
    outcome.action = FLOW_ACTION_WAIT;
  else if (strcmp(field, "goto") == 0)
    outcome.action = FLOW_ACTION_GOTO;
  else if (strcmp(field, "done") == 0)
    outcome.action = FLOW_ACTION_DONE;
  else {
    if (strcmp(field, "cancel") != 0)
      log_error(runtime->services->log, LOG_BUGS, "LUA", "FLOW",
                "Unknown flow action '%s' from step '%s' in %s; cancelling.",
                field, step, data->path);
    outcome.action = FLOW_ACTION_CANCEL;
  }
  lua_pop(state, 1);

  lua_getfield(state, result_index, "step");
  if (lua_isstring(state, -1))
    StringCopyTrunc(outcome.next_step, lua_tostring(state, -1),
                    FLOW_STEP_NAME_SIZE - 1);
  lua_pop(state, 1);

  lua_getfield(state, result_index, "prompt");
  if (!lua_isstring(state, -1)) {
    lua_pop(state, 1);
    lua_getfield(state, result_index, "message");
  }
  if (lua_isstring(state, -1)) {
    snprintf(prompt_buffer, sizeof(prompt_buffer), "%s",
             lua_tostring(state, -1));
    outcome.prompt = prompt_buffer;
  }
  lua_pop(state, 1);

  lua_settop(state, top);
  return outcome;
}

static int lua_verify_module_has_flow(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                                      const char *path, const char *first_step,
                                      char *error, size_t error_size) {
  int top = lua_gettop(runtime->state);
  int ok = 0;

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(runtime->state, top);
    return 0;
  }
  lua_getfield(runtime->state, -1, "flows");
  if (lua_istable(runtime->state, -1)) {
    lua_getfield(runtime->state, -1, first_step);
    ok = lua_isfunction(runtime->state, -1);
  }
  lua_settop(runtime->state, top);
  if (!ok)
    lua_set_error(error, error_size, "%s has no flow step '%s'", path,
                  first_step);
  return ok;
}

int lua_runtime_is_checking(void *context) {
  LuaRuntime *runtime = context;

  return runtime->checking;
}

int lua_runtime_flow_start(void *context, lua_State *state, int descriptor_id,
                           const char *module, const char *first_step) {
  LuaRuntime *runtime = context;
  Descriptor *d;
  LUA_MODULE_ROOT root;
  char error[LBUF_SIZE];
  LuaFlowData *data;

  d = descriptor_find_by_fd(runtime->services->descriptors, descriptor_id);
  if (!d)
    return luaL_error(state, "no such descriptor");
  if (d->flow != nullptr)
    return luaL_error(state, "descriptor already has an active flow");

  root = lua_require_root(state, runtime);
  if (!lua_verify_module_has_flow(runtime, root, module, first_step, error,
                                  sizeof(error)))
    return luaL_error(state, "%s", error);

  data = malloc(sizeof(LuaFlowData));
  data->runtime_owner = runtime->owner;
  data->root = root;
  snprintf(data->path, sizeof(data->path), "%s", module);
  data->field_count = 0;

  descriptor_flow_start(d, first_step, lua_flow_step, data, lua_flow_data_free);
  return 0;
}

int lua_runtime_exit_enter_lock_passes(void *context, DbRef exit,
                                       DbRef enactor) {
  LuaRuntime *runtime = context;
  LuaLockResult result;

  lua_lock_evaluate(runtime,
                    &(LuaLockInvocation){
                        .type = LUA_LOCK_DEFAULT,
                        .operation = LUA_LOCK_OPERATION_TRAVERSE,
                        .object = exit,
                        .enactor = enactor,
                        .cause = enactor,
                        .subject = enactor,
                        .silent = true,
                    },
                    &result);
  return result.passes;
}
