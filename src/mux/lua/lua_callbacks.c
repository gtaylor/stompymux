/* lua.c - Lua runtime initialization and MUX integration. */

#include "mux/server/platform.h"

#include "mux/lua/btech_package.h"
#include "mux/lua/command_access.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/mux_package.h"

#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#include <lauxlib.h>
#include <lua.h>
#include <luajit.h>
#include <lualib.h>

#include "mux/commands/command.h"
#include "mux/commands/command_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/network/input_flow.h"
#include "mux/objects/attrs.h"
#include "mux/server/log.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/world/match.h"
#include "mux/world/world_context.h"

#include "mux/lua/lua_internal.h"

const char *const LUA_EVENT_NAMES[LUA_EVENT_COUNT] = {
    [LUA_EVENT_NONE] = nullptr,
    [LUA_EVENT_SUCCESS] = "on_success",
    [LUA_EVENT_FAIL] = "on_fail",
    [LUA_EVENT_DROP] = "on_drop",
    [LUA_EVENT_GIVE_FAIL] = "on_give_fail",
    [LUA_EVENT_GIVE_RECEIVE_FAIL] = "on_give_receive_fail",
    [LUA_EVENT_DROP_FAIL] = "on_drop_fail",
    [LUA_EVENT_USE] = "on_use",
    [LUA_EVENT_USE_FAIL] = "on_use_fail",
    [LUA_EVENT_DESCRIBE] = "on_describe",
    [LUA_EVENT_ENTER] = "on_enter",
    [LUA_EVENT_LEAVE] = "on_leave",
    [LUA_EVENT_MOVE] = "on_move",
    [LUA_EVENT_ENTER_FAIL] = "on_enter_fail",
    [LUA_EVENT_LEAVE_FAIL] = "on_leave_fail",
    [LUA_EVENT_TELEPORT] = "on_teleport",
    [LUA_EVENT_TELEPORT_DESTINATION_FAIL] = "on_teleport_destination_fail",
    [LUA_EVENT_TELEPORT_OUT_FAIL] = "on_teleport_out_fail",
    [LUA_EVENT_CLONE] = "on_clone",
    [LUA_EVENT_SERVER_STARTUP] = "on_server_startup",
    [LUA_EVENT_CONNECT] = "on_connect",
    [LUA_EVENT_DISCONNECT] = "on_disconnect",
    [LUA_EVENT_MECH_DESTROYED] = "on_mech_destroyed",
    [LUA_EVENT_MECH_MINE_TRIGGER] = "on_mech_mine_trigger",
    [LUA_EVENT_AERO_LAND] = "on_aero_land",
    [LUA_EVENT_OOD_LAND] = "on_ood_land",
};

const char *const LUA_LOCK_NAMES[LUA_LOCK_COUNT] = {
    [LUA_LOCK_DEFAULT] = "default",   [LUA_LOCK_DROP] = "drop",
    [LUA_LOCK_ENTER] = "enter",       [LUA_LOCK_GIVE] = "give",
    [LUA_LOCK_LEAVE] = "leave",       [LUA_LOCK_LINK] = "link",
    [LUA_LOCK_RECEIVE] = "receive",   [LUA_LOCK_SPEECH] = "speech",
    [LUA_LOCK_TELEPORT] = "teleport", [LUA_LOCK_TELEPORT_OUT] = "teleport_out",
    [LUA_LOCK_USE] = "use",
};

const char *const LUA_LOCK_OPERATION_NAMES[LUA_LOCK_OPERATION_COUNT] = {
    [LUA_LOCK_OPERATION_MATCH] = "match",
    [LUA_LOCK_OPERATION_TRAVERSE] = "traverse",
    [LUA_LOCK_OPERATION_TAKE] = "take",
    [LUA_LOCK_OPERATION_LOOK] = "look",
    [LUA_LOCK_OPERATION_COMMAND_MATCH] = "command_match",
    [LUA_LOCK_OPERATION_USE] = "use",
    [LUA_LOCK_OPERATION_DROP] = "drop",
    [LUA_LOCK_OPERATION_GIVE] = "give",
    [LUA_LOCK_OPERATION_RECEIVE] = "receive",
    [LUA_LOCK_OPERATION_ENTER] = "enter",
    [LUA_LOCK_OPERATION_LEAVE] = "leave",
    [LUA_LOCK_OPERATION_TELEPORT] = "teleport",
    [LUA_LOCK_OPERATION_TELEPORT_OUT] = "teleport_out",
    [LUA_LOCK_OPERATION_LINK] = "link",
    [LUA_LOCK_OPERATION_SET_HOME] = "set_home",
    [LUA_LOCK_OPERATION_SPEAK] = "speak",
    [LUA_LOCK_OPERATION_CHANNEL_JOIN] = "channel_join",
    [LUA_LOCK_OPERATION_CHANNEL_TRANSMIT] = "channel_transmit",
    [LUA_LOCK_OPERATION_CHANNEL_RECEIVE] = "channel_receive",
    [LUA_LOCK_OPERATION_BTECH_ENTER] = "btech_enter",
    [LUA_LOCK_OPERATION_BTECH_CONTACT] = "btech_contact",
};

const char *const LUA_MESSAGE_NAMES[LUA_MESSAGE_COUNT] = {
    [LUA_MESSAGE_NONE] = nullptr,
    [LUA_MESSAGE_SUCCESS] = "success",
    [LUA_MESSAGE_DROP] = "drop",
    [LUA_MESSAGE_DESCRIBE] = "describe",
    [LUA_MESSAGE_USE] = "use",
    [LUA_MESSAGE_LEAVE] = "leave",
    [LUA_MESSAGE_ENTER] = "enter",
    [LUA_MESSAGE_MOVE] = "move",
    [LUA_MESSAGE_TELEPORT] = "teleport",
    [LUA_MESSAGE_ENTER_SOURCE] = "enter_source",
    [LUA_MESSAGE_LEAVE_DESTINATION] = "leave_destination",
    [LUA_MESSAGE_TELEPORT_SOURCE] = "teleport_source",
};

const char *const LUA_MESSAGE_OPERATION_NAMES[LUA_MESSAGE_OPERATION_COUNT] = {
    [LUA_MESSAGE_OPERATION_NONE] = "none",
    [LUA_MESSAGE_OPERATION_LOOK] = "look",
    [LUA_MESSAGE_OPERATION_TAKE] = "take",
    [LUA_MESSAGE_OPERATION_TRAVERSE] = "traverse",
    [LUA_MESSAGE_OPERATION_RECEIVE] = "receive",
    [LUA_MESSAGE_OPERATION_DROP] = "drop",
    [LUA_MESSAGE_OPERATION_GIVE] = "give",
    [LUA_MESSAGE_OPERATION_DESCRIBE] = "describe",
    [LUA_MESSAGE_OPERATION_INSIDE_DESCRIBE] = "inside_describe",
    [LUA_MESSAGE_OPERATION_USE] = "use",
    [LUA_MESSAGE_OPERATION_MOVE] = "move",
    [LUA_MESSAGE_OPERATION_TELEPORT] = "teleport",
};

const char *lua_event_name(LuaEventType event) {
  if ((unsigned int)event >= LUA_EVENT_COUNT)
    return nullptr;
  return LUA_EVENT_NAMES[event];
}

bool lua_event_name_is_known(const char *name) {
  LuaEventType event;

  if (!name)
    return false;
  for (event = LUA_EVENT_SUCCESS; event < LUA_EVENT_COUNT; event++) {
    if (!strcmp(name, LUA_EVENT_NAMES[event]))
      return true;
  }
  return false;
}

const char *lua_lock_name(LuaLockType lock) {
  if ((unsigned int)lock >= LUA_LOCK_COUNT)
    return nullptr;
  return LUA_LOCK_NAMES[lock];
}

const char *lua_lock_operation_name(LuaLockOperation operation) {
  if ((unsigned int)operation >= LUA_LOCK_OPERATION_COUNT)
    return nullptr;
  return LUA_LOCK_OPERATION_NAMES[operation];
}

bool lua_lock_name_is_known(const char *name) {
  LuaLockType lock;

  if (!name)
    return false;
  for (lock = LUA_LOCK_DEFAULT; lock < LUA_LOCK_COUNT; lock++) {
    if (!strcmp(name, LUA_LOCK_NAMES[lock]))
      return true;
  }
  return false;
}

const char *lua_message_name(LuaMessageType message) {
  if ((unsigned int)message >= LUA_MESSAGE_COUNT)
    return nullptr;
  return LUA_MESSAGE_NAMES[message];
}

const char *lua_message_operation_name(LuaMessageOperation operation) {
  if ((unsigned int)operation >= LUA_MESSAGE_OPERATION_COUNT)
    return nullptr;
  return LUA_MESSAGE_OPERATION_NAMES[operation];
}

bool lua_message_name_is_known(const char *name) {
  LuaMessageType message;

  if (!name)
    return false;
  for (message = LUA_MESSAGE_SUCCESS; message < LUA_MESSAGE_COUNT; message++) {
    if (!strcmp(name, LUA_MESSAGE_NAMES[message]))
      return true;
  }
  return false;
}

void lua_push_context(GameDatabase *database, Descriptor *descriptor,
                      lua_State *state, DbRef object, DbRef player, DbRef cause,
                      const char *command, const char *event, const char *scope,
                      char *args[], int nargs) {
  int index;

  lua_newtable(state);
  if (is_good_obj(database, object)) {
    lua_pushinteger(state, object);
    lua_setfield(state, -2, "object");
  }
  lua_pushinteger(state, player);
  lua_setfield(state, -2, "enactor");
  lua_pushinteger(state, cause);
  lua_setfield(state, -2, "cause");
  if (command) {
    lua_pushstring(state, command);
    lua_setfield(state, -2, "command");
  }
  if (event) {
    lua_pushstring(state, event);
    lua_setfield(state, -2, "event");
  }
  if (scope) {
    lua_pushstring(state, scope);
    lua_setfield(state, -2, "scope");
  }
  if (descriptor != nullptr) {
    lua_pushinteger(state, descriptor->descriptor);
    lua_setfield(state, -2, "descriptor");
  }
  lua_newtable(state);
  for (index = 0; index < nargs; index++) {
    if (args[index]) {
      lua_pushstring(state, args[index]);
      lua_rawseti(state, -2, index + 1);
    }
  }
  lua_setfield(state, -2, "args");
}

void lua_appearance_evaluate(LuaRuntime *runtime,
                             const LuaAppearanceInvocation *invocation,
                             LuaAppearanceResult *result) {
  lua_State *state;
  const char *function;
  const char *rendered;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  size_t length;
  int status;
  int top;

  memset(result, 0, sizeof(*result));
  if (!runtime || !invocation)
    return;
  function = invocation->type == LUA_APPEARANCE_INTERNAL
                 ? "internal_appearance"
                 : "external_appearance";
  if (!lua_attached_path(runtime, invocation->object, path, sizeof(path),
                         nullptr))
    return;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, function);
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return;
  }
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, nullptr, nullptr, nullptr, 0);
  lua_pushstring(state, function);
  lua_setfield(state, -2, "appearance");
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_callback_pcall_checked(runtime, 1, 1);
    runtime->current_root = previous_root;
  }
  if (status) {
    lua_log_error(runtime, invocation->object, "APPEARANCE",
                  lua_tostring(state, -1));
  } else if (!lua_isnil(state, -1)) {
    if (lua_type(state, -1) != LUA_TSTRING) {
      lua_log_error(runtime, invocation->object, "APPEARANCE",
                    "appearance function must return a string or nil");
    } else {
      rendered = lua_tolstring(state, -1, &length);
      if (length >= sizeof(result->rendered)) {
        lua_log_error(runtime, invocation->object, "APPEARANCE",
                      "appearance string exceeds the MUX buffer limit");
      } else if (memchr(rendered, '\0', length)) {
        lua_log_error(runtime, invocation->object, "APPEARANCE",
                      "appearance string contains an embedded NUL");
      } else {
        memcpy(result->rendered, rendered, length);
        result->rendered[length] = '\0';
        result->defined = true;
      }
    }
  }
  lua_settop(state, top);
}

void lua_mech_status_evaluate(LuaRuntime *runtime,
                              const LuaMechStatusInvocation *invocation,
                              LuaMechStatusResult *result) {
  memset(result, 0, sizeof(*result));
  if (!runtime || !invocation)
    return;

  char path[PATH_MAX];
  char error[LBUF_SIZE];
  if (!lua_attached_path(runtime, invocation->object, path, sizeof(path),
                         nullptr))
    return;

  lua_State *state = runtime->state;
  int top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "mech_status");
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return;
  }

  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, nullptr, nullptr, nullptr, 0);
  LUA_MODULE_ROOT previous_root = runtime->current_root;
  runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
  int status = lua_callback_pcall_checked(runtime, 1, 1);
  runtime->current_root = previous_root;
  if (status) {
    lua_log_error(runtime, invocation->object, "MECH_STATUS",
                  lua_tostring(state, -1));
  } else if (!lua_isnil(state, -1)) {
    if (lua_type(state, -1) != LUA_TSTRING) {
      lua_log_error(runtime, invocation->object, "MECH_STATUS",
                    "mech_status must return a string or nil");
    } else {
      size_t length;
      const char *rendered = lua_tolstring(state, -1, &length);
      if (length >= sizeof(result->rendered)) {
        lua_log_error(runtime, invocation->object, "MECH_STATUS",
                      "mech_status exceeds the MUX buffer limit");
      } else if (memchr(rendered, '\0', length)) {
        lua_log_error(runtime, invocation->object, "MECH_STATUS",
                      "mech_status contains an embedded NUL");
      } else {
        memcpy(result->rendered, rendered, length);
        result->rendered[length] = '\0';
        result->defined = true;
      }
    }
  }
  lua_settop(state, top);
}

bool lua_event_defined(LuaRuntime *runtime, DbRef object, LuaEventType event) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  bool defined;

  if (!runtime || !lua_event_name(event) ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "events");
  if (lua_istable(state, -1))
    lua_getfield(state, -1, lua_event_name(event));
  defined = lua_isfunction(state, -1);
  lua_settop(state, top);
  return defined;
}

bool lua_lock_defined(LuaRuntime *runtime, DbRef object, LuaLockType lock) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  bool defined;

  if (!runtime || !lua_lock_name(lock) ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "locks");
  if (lua_istable(state, -1))
    lua_getfield(state, -1, lua_lock_name(lock));
  defined = lua_isfunction(state, -1);
  lua_settop(state, top);
  return defined;
}

bool lua_message_defined(LuaRuntime *runtime, DbRef object,
                         LuaMessageType message) {
  lua_State *state;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  bool defined;

  if (!runtime || !lua_message_name(message) ||
      !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "messages");
  if (lua_istable(state, -1))
    lua_getfield(state, -1, lua_message_name(message));
  defined = lua_isfunction(state, -1);
  lua_settop(state, top);
  return defined;
}

bool lua_event_dispatch(LuaRuntime *runtime,
                        const LuaEventInvocation *invocation) {
  lua_State *state;
  const char *event;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int status;

  if (!runtime || !invocation || !(event = lua_event_name(invocation->type)) ||
      !lua_attached_path(runtime, invocation->object, path, sizeof(path),
                         nullptr))
    return false;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return true;
  }
  lua_getfield(state, -1, "events");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return false;
  }
  lua_getfield(state, -1, event);
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return false;
  }
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, event, nullptr, invocation->arguments,
                   invocation->argument_count);
  if (invocation->type == LUA_EVENT_CONNECT) {
    lua_pushboolean(state, invocation->reconnect);
    lua_setfield(state, -2, "reconnect");
  } else if (invocation->type == LUA_EVENT_DISCONNECT && invocation->reason) {
    lua_pushstring(state, invocation->reason);
    lua_setfield(state, -2, "reason");
  }
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_callback_pcall_checked(runtime, 1, 0);
    runtime->current_root = previous_root;
  }
  if (status)
    lua_log_error(runtime, invocation->object, "EVENT",
                  lua_tostring(state, -1));
  lua_settop(state, top);
  return true;
}

static bool lua_result_copy_message(lua_State *state, int table,
                                    const char *field, bool *present,
                                    char destination[LBUF_SIZE]) {
  size_t length;
  const char *message;

  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (lua_type(state, -1) != LUA_TSTRING) {
    lua_pop(state, 1);
    return false;
  }
  message = lua_tolstring(state, -1, &length);
  if (length >= LBUF_SIZE) {
    lua_pop(state, 1);
    return false;
  }
  memcpy(destination, message, length);
  destination[length] = '\0';
  *present = true;
  lua_pop(state, 1);
  return true;
}

static bool lua_lock_parse_result(lua_State *state, LuaLockResult *result) {
  int table;

  if (lua_isboolean(state, -1)) {
    result->passes = lua_toboolean(state, -1);
    return true;
  }
  if (!lua_istable(state, -1))
    return false;
  table = lua_gettop(state);
  lua_pushnil(state);
  while (lua_next(state, table) != 0) {
    const char *key = lua_tostring(state, -2);
    bool valid = lua_type(state, -2) == LUA_TSTRING && key &&
                 (!strcmp(key, "passes") || !strcmp(key, "enactor_message") ||
                  !strcmp(key, "other_message"));

    lua_pop(state, 1);
    if (!valid) {
      lua_pop(state, 1);
      return false;
    }
  }
  lua_getfield(state, table, "passes");
  if (!lua_isboolean(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  result->passes = lua_toboolean(state, -1);
  lua_pop(state, 1);
  return lua_result_copy_message(state, table, "enactor_message",
                                 &result->has_enactor_message,
                                 result->enactor_message) &&
         lua_result_copy_message(state, table, "other_message",
                                 &result->has_other_message,
                                 result->other_message);
}

void lua_lock_evaluate(LuaRuntime *runtime, const LuaLockInvocation *invocation,
                       LuaLockResult *result) {
  lua_State *state;
  const char *lock;
  const char *operation;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int status;

  memset(result, 0, sizeof(*result));
  result->passes = false;
  if (!runtime || !invocation || !(lock = lua_lock_name(invocation->type)) ||
      !(operation = lua_lock_operation_name(invocation->operation)))
    return;
  if (!lua_attached_path(runtime, invocation->object, path, sizeof(path),
                         nullptr)) {
    result->passes = true;
    return;
  }
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    result->defined = true;
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "locks");
  if (!lua_istable(state, -1)) {
    result->passes = true;
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, lock);
  if (!lua_isfunction(state, -1)) {
    result->passes = true;
    lua_settop(state, top);
    return;
  }
  result->defined = true;
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, nullptr, nullptr, nullptr, 0);
  lua_pushinteger(state, invocation->subject);
  lua_setfield(state, -2, "subject");
  lua_pushstring(state, lock);
  lua_setfield(state, -2, "lock");
  lua_pushstring(state, operation);
  lua_setfield(state, -2, "operation");
  lua_pushboolean(state, invocation->silent);
  lua_setfield(state, -2, "silent");
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_callback_pcall_checked(runtime, 1, 1);
    runtime->current_root = previous_root;
  }
  if (status) {
    lua_log_error(runtime, invocation->object, "LOCK", lua_tostring(state, -1));
  } else if (!lua_lock_parse_result(state, result)) {
    lua_log_error(runtime, invocation->object, "LOCK",
                  "lock handler must return a boolean or a valid result table");
    result->passes = false;
    result->has_enactor_message = false;
    result->has_other_message = false;
  }
  lua_settop(state, top);
}

static bool lua_message_parse_result(lua_State *state, LuaMessageType type,
                                     LuaMessageResult *result) {
  const bool allow_enactor = type != LUA_MESSAGE_DESCRIBE &&
                             type != LUA_MESSAGE_ENTER_SOURCE &&
                             type != LUA_MESSAGE_LEAVE_DESTINATION &&
                             type != LUA_MESSAGE_TELEPORT_SOURCE;
  int table;

  if (!lua_istable(state, -1))
    return false;
  table = lua_gettop(state);
  lua_pushnil(state);
  while (lua_next(state, table) != 0) {
    const char *key = lua_tostring(state, -2);
    const bool valid = lua_type(state, -2) == LUA_TSTRING && key &&
                       ((!strcmp(key, "enactor_message") && allow_enactor) ||
                        !strcmp(key, "other_message"));

    lua_pop(state, 1);
    if (!valid) {
      lua_pop(state, 1);
      return false;
    }
  }
  return (!allow_enactor ||
          lua_result_copy_message(state, table, "enactor_message",
                                  &result->has_enactor_message,
                                  result->enactor_message)) &&
         lua_result_copy_message(state, table, "other_message",
                                 &result->has_other_message,
                                 result->other_message);
}

void lua_message_evaluate(LuaRuntime *runtime,
                          const LuaMessageInvocation *invocation,
                          LuaMessageResult *result) {
  lua_State *state;
  const char *message;
  const char *operation;
  char path[PATH_MAX];
  char error[LBUF_SIZE];
  int top;
  int status;

  memset(result, 0, sizeof(*result));
  if (!runtime || !invocation ||
      !(message = lua_message_name(invocation->type)) ||
      !(operation = lua_message_operation_name(invocation->operation)) ||
      !lua_attached_path(runtime, invocation->object, path, sizeof(path),
                         nullptr))
    return;
  state = runtime->state;
  top = lua_gettop(state);
  if (!lua_load_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, error,
                       sizeof(error))) {
    lua_log_load_error(runtime, invocation->object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "messages");
  if (!lua_istable(state, -1)) {
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, message);
  if (!lua_isfunction(state, -1)) {
    lua_settop(state, top);
    return;
  }
  result->defined = true;
  lua_push_context(runtime->services->database, invocation->descriptor, state,
                   invocation->object, invocation->enactor, invocation->cause,
                   nullptr, nullptr, nullptr, nullptr, 0);
  lua_pushstring(state, message);
  lua_setfield(state, -2, "message");
  lua_pushstring(state, operation);
  lua_setfield(state, -2, "operation");
  lua_pushboolean(state, invocation->silent);
  lua_setfield(state, -2, "silent");
  if (invocation->source == NOTHING)
    lua_pushnil(state);
  else
    lua_pushinteger(state, invocation->source);
  lua_setfield(state, -2, "source");
  if (invocation->destination == NOTHING)
    lua_pushnil(state);
  else
    lua_pushinteger(state, invocation->destination);
  lua_setfield(state, -2, "destination");
  {
    LUA_MODULE_ROOT previous_root = runtime->current_root;

    runtime->current_root = LUA_ROOT_OBJECT_LOGIC;
    status = lua_callback_pcall_checked(runtime, 1, 1);
    runtime->current_root = previous_root;
  }
  if (status) {
    lua_log_error(runtime, invocation->object, "MESSAGE",
                  lua_tostring(state, -1));
  } else if (!lua_message_parse_result(state, invocation->type, result)) {
    lua_log_error(runtime, invocation->object, "MESSAGE",
                  "message provider must return a valid result table");
    result->has_enactor_message = false;
    result->has_other_message = false;
  }
  lua_settop(state, top);
}
