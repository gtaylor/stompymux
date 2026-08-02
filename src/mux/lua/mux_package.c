/* mux_package.c - Built-in Lua mux package bindings. */

#include "mux/server/platform.h"

#include "mux/lua/mux_package.h"

#include <math.h>
#include <stdint.h>
#include <string.h>

#include <lauxlib.h>

#include "mux/lua/lua_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/objects/object_state.h"
#include "mux/server/mux_server.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/styled_text.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"

static LuaMuxPackage *lua_mux_package_get(lua_State *state) {
  return lua_touserdata(state, lua_upvalueindex(1));
}

constexpr char LUA_MUX_OBJECT_METATABLE[] = "btmux.object";
constexpr char LUA_MUX_STATE_METATABLE[] = "btmux.object_state";

typedef struct LuaMuxObject LuaMuxObject;
struct LuaMuxObject {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
};

typedef struct LuaMuxState LuaMuxState;
struct LuaMuxState {
  LuaMuxPackage *package;
  DbRef object;
  uint64_t generation;
  char name_space[128];
};

bool lua_mux_package_transaction_begin(LuaMuxPackage *package) {
  return object_state_transaction_begin(&package->state_transaction,
                                        package->services->database);
}

void lua_mux_package_transaction_finish(LuaMuxPackage *package, bool commit) {
  object_state_transaction_finish(&package->state_transaction, commit);
}

void lua_mux_package_destroy(LuaMuxPackage *package) {
  object_state_transaction_destroy(&package->state_transaction);
}

static int lua_mux_package_is_checking(LuaMuxPackage *package) {
  return package->is_checking && package->is_checking(package->context);
}

static void lua_mux_require_runtime(LuaMuxPackage *package, lua_State *state,
                                    const char *function) {
  if (lua_mux_package_is_checking(package))
    luaL_error(state, "mux.%s is unavailable during @lua/check", function);
}

static DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                                    int argument) {
  DbRef object;
  LuaMuxObject *handle =
      luaL_testudata(state, argument, LUA_MUX_OBJECT_METATABLE);

  if (handle) {
    if (handle->package != package)
      luaL_argerror(state, argument, "object belongs to another Lua runtime");
    if (!is_good_obj(package->services->database, handle->object) ||
        game_object_generation(package->services->database, handle->object) !=
            handle->generation)
      luaL_argerror(state, argument, "object no longer exists");
    object = handle->object;
  } else {
    object = (DbRef)luaL_checkinteger(state, argument);
  }

  if (!is_good_obj(package->services->database, object))
    luaL_argerror(state, argument, "invalid object");
  return object;
}

static LuaMuxObject *lua_mux_push_object(lua_State *state,
                                         LuaMuxPackage *package, DbRef object) {
  LuaMuxObject *handle = lua_newuserdata(state, sizeof(*handle));

  *handle = (LuaMuxObject){
      .package = package,
      .object = object,
      .generation = game_object_generation(package->services->database, object),
  };
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_setmetatable(state, -2);
  return handle;
}

static LuaMuxObject *lua_mux_check_object_handle(lua_State *state,
                                                 int argument) {
  LuaMuxObject *handle =
      luaL_checkudata(state, argument, LUA_MUX_OBJECT_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    luaL_argerror(state, argument, "object no longer exists");
  return handle;
}

static LuaMuxState *lua_mux_check_state(lua_State *state, int argument) {
  LuaMuxState *handle =
      luaL_checkudata(state, argument, LUA_MUX_STATE_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    luaL_argerror(state, argument, "object no longer exists");
  return handle;
}

static int lua_mux_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;

  lua_mux_require_runtime(package, state, "object");
  object = lua_mux_require_object(package, state, 1);
  lua_mux_push_object(state, package, object);
  return 1;
}

static bool lua_mux_list_contains(GameDatabase *database, DbRef first,
                                  DbRef member) {
  DbRef object;

  DOLIST(database, object, first) {
    if (object == member)
      return true;
  }
  return false;
}

static int lua_mux_contents(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef member;
  int index = 1;

  lua_mux_require_runtime(package, state, "contents");
  object = lua_mux_require_object(package, state, 1);
  if (!has_contents(package->services->database, object))
    return luaL_argerror(state, 1, "object cannot contain other objects");
  lua_newtable(state);
  DOLIST(package->services->database, member,
         game_object_contents(package->services->database, object)) {
    lua_mux_push_object(state, package, member);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_contents_visible(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  EvaluationContext *evaluation;
  DbRef container;
  DbRef viewer;
  DbRef member;
  bool can_see_location;

  lua_mux_require_runtime(package, state, "contents_visible");
  container = lua_mux_require_object(package, state, 1);
  viewer = lua_mux_require_object(package, state, 2);
  member = lua_mux_require_object(package, state, 3);
  if (!has_contents(package->services->database, container))
    return luaL_argerror(state, 1, "object cannot contain other objects");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_contents(package->services->database, container), member))
    return luaL_argerror(state, 3, "object is not directly contained");
  evaluation = &package->services->background_command->evaluation;
  can_see_location = !is_dark(package->services->database, container);
  lua_pushboolean(state, can_see(evaluation, viewer, member, can_see_location));
  return 1;
}

static int lua_mux_exits(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef exit;
  int index = 1;

  lua_mux_require_runtime(package, state, "exits");
  object = lua_mux_require_object(package, state, 1);
  if (!has_exits(package->services->database, object))
    return luaL_argerror(state, 1, "object cannot have exits");
  lua_newtable(state);
  DOLIST(package->services->database, exit,
         game_object_exits(package->services->database, object)) {
    lua_mux_push_object(state, package, exit);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_exits_visible(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef location;
  DbRef viewer;
  DbRef exit;
  int key = 0;

  lua_mux_require_runtime(package, state, "exits_visible");
  location = lua_mux_require_object(package, state, 1);
  viewer = lua_mux_require_object(package, state, 2);
  exit = lua_mux_require_object(package, state, 3);
  if (!has_exits(package->services->database, location))
    return luaL_argerror(state, 1, "object cannot have exits");
  if (!is_exit(package->services->database, exit))
    return luaL_argerror(state, 3, "object is not an exit");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_exits(package->services->database, location), exit))
    return luaL_argerror(state, 3, "exit is not directly attached");
  if (is_dark(package->services->database, location))
    key |= VE_LOC_DARK;
  lua_pushboolean(
      state, exit_displayable(package->services->database, exit, viewer, key));
  return 1;
}

static int lua_mux_exit_enter_lock_passes(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef exit;
  DbRef enactor;

  lua_mux_require_runtime(package, state, "exit_enter_lock_passes");
  exit = lua_mux_require_object(package, state, 1);
  enactor = lua_mux_require_object(package, state, 2);
  if (!is_exit(package->services->database, exit))
    return luaL_argerror(state, 1, "object is not an exit");
  if (!package->exit_enter_lock_passes)
    return luaL_error(state, "mux.exit_enter_lock_passes is unavailable");
  lua_pushboolean(
      state, package->exit_enter_lock_passes(package->context, exit, enactor));
  return 1;
}

static int lua_mux_markup(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *markup = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_markup");
  char error[256];

  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           output, LBUF_SIZE, error, sizeof(error))) {
    free_lbuf(output);
    return luaL_error(state, "invalid color markup: %s", error);
  }
  lua_pushstring(state, markup);
  free_lbuf(output);
  return 1;
}

static bool lua_mux_style_open_string(lua_State *state, int table,
                                      const char *field, const char *tag,
                                      char *markup, char **cursor,
                                      size_t *open_count) {
  const char *value;

  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (!lua_isstring(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  value = lua_tostring(state, -1);
  if (strchr(value, '[') || strchr(value, ']')) {
    lua_pop(state, 1);
    return false;
  }
  safe_str("[", markup, cursor);
  safe_str(tag, markup, cursor);
  safe_str(value, markup, cursor);
  safe_str("]", markup, cursor);
  (*open_count)++;
  lua_pop(state, 1);
  return true;
}

static bool lua_mux_style_open_bool(lua_State *state, int table,
                                    const char *field, const char *tag,
                                    char *markup, char **cursor,
                                    size_t *open_count) {
  bool enabled;

  lua_getfield(state, table, field);
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return true;
  }
  if (!lua_isboolean(state, -1)) {
    lua_pop(state, 1);
    return false;
  }
  enabled = lua_toboolean(state, -1);
  lua_pop(state, 1);
  if (!enabled)
    return true;
  safe_str("[", markup, cursor);
  safe_str(tag, markup, cursor);
  safe_str("]", markup, cursor);
  (*open_count)++;
  return true;
}

static int lua_mux_style(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  size_t text_length;
  const char *value = luaL_checklstring(state, 1, &text_length);
  char *markup;
  char *cursor;
  char *validated;
  char error[256];
  size_t open_count = 0;

  if (strlen(value) != text_length)
    return luaL_argerror(state, 1, "value contains an embedded NUL byte");
  luaL_checktype(state, 2, LUA_TTABLE);
  markup = alloc_lbuf("lua_mux_style.markup");
  cursor = markup;
  *cursor = '\0';
  if (!lua_mux_style_open_string(state, 2, "foreground", "fg=", markup, &cursor,
                                 &open_count) ||
      !lua_mux_style_open_string(state, 2, "background", "bg=", markup, &cursor,
                                 &open_count) ||
      !lua_mux_style_open_bool(state, 2, "bold", "bold", markup, &cursor,
                               &open_count) ||
      !lua_mux_style_open_bool(state, 2, "underline", "underline", markup,
                               &cursor, &open_count) ||
      !lua_mux_style_open_bool(state, 2, "inverse", "inverse", markup, &cursor,
                               &open_count)) {
    free_lbuf(markup);
    return luaL_error(state, "style fields have invalid types");
  }
  safe_str(value, markup, &cursor);
  for (size_t index = 0; index < open_count; index++)
    safe_str("[/]", markup, &cursor);
  *cursor = '\0';

  validated = alloc_lbuf("lua_mux_style.validated");
  if (!styled_text_compile(package->services->styled_text_palette, markup,
                           validated, LBUF_SIZE, error, sizeof(error))) {
    free_lbuf(markup);
    free_lbuf(validated);
    return luaL_error(state, "invalid style: %s", error);
  }
  lua_pushstring(state, markup);
  free_lbuf(markup);
  free_lbuf(validated);
  return 1;
}

static int lua_mux_strip_style(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  char *output = alloc_lbuf("lua_mux_strip_style");

  styled_text_strip(package->services->styled_text_palette, value, output,
                    LBUF_SIZE);
  lua_pushstring(state, output);
  free_lbuf(output);
  return 1;
}

static int lua_mux_text_width(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_pushinteger(state, (lua_Integer)styled_text_width(
                             package->services->styled_text_palette,
                             luaL_checkstring(state, 1)));
  return 1;
}

static int lua_mux_truncate_text(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  const char *value = luaL_checkstring(state, 1);
  lua_Integer width = luaL_checkinteger(state, 2);
  char *output;

  if (width < 0)
    return luaL_argerror(state, 2, "width must not be negative");
  output = alloc_lbuf("lua_mux_truncate_text");
  styled_text_truncate(package->services->styled_text_palette, value,
                       (size_t)width, output, LBUF_SIZE);
  lua_pushstring(state, output);
  free_lbuf(output);
  return 1;
}

static int lua_mux_object_index(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);
  LuaMuxPackage *package = handle->package;
  const char *key = luaL_checkstring(state, 2);
  GameDatabase *database = package->services->database;

  if (!strcmp(key, "dbref")) {
    lua_pushinteger(state, handle->object);
    return 1;
  }
  if (!strcmp(key, "name")) {
    lua_pushstring(state, game_object_name(database, handle->object));
    return 1;
  }
  if (!strcmp(key, "type")) {
    switch (typeof_obj(database, handle->object)) {
    case OBJECT_TYPE_ROOM:
      lua_pushliteral(state, "room");
      break;
    case OBJECT_TYPE_THING:
      lua_pushliteral(state, "thing");
      break;
    case OBJECT_TYPE_EXIT:
      lua_pushliteral(state, "exit");
      break;
    case OBJECT_TYPE_PLAYER:
      lua_pushliteral(state, "player");
      break;
    default:
      lua_pushnil(state);
      break;
    }
    return 1;
  }
  if (!strcmp(key, "description") || !strcmp(key, "inside_description")) {
    int attribute = !strcmp(key, "description") ? A_DESC : A_IDESC;
    const char *description =
        attribute_get_raw(database, handle->object, attribute);
    if (description)
      lua_pushstring(state, description);
    else
      lua_pushnil(state);
    return 1;
  }
  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_getfield(state, -1, key);
  lua_remove(state, -2);
  return 1;
}

static int lua_mux_object_tostring(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushfstring(state, "object(#%d)", (int)handle->object);
  return 1;
}

static int lua_mux_object_equal(lua_State *state) {
  LuaMuxObject *left = luaL_checkudata(state, 1, LUA_MUX_OBJECT_METATABLE);
  LuaMuxObject *right = luaL_checkudata(state, 2, LUA_MUX_OBJECT_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->object == right->object &&
                             left->generation == right->generation);
  return 1;
}

static int lua_mux_object_state(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);
  size_t length;
  const char *name_space = luaL_checklstring(state, 2, &length);
  LuaMuxState *handle;

  lua_mux_require_runtime(object->package, state, "object:state");
  if (length >= sizeof(handle->name_space) ||
      memchr(name_space, '\0', length) ||
      !object_state_name_is_valid(name_space))
    return luaL_argerror(state, 2, "invalid state namespace");
  handle = lua_newuserdata(state, sizeof(*handle));
  *handle = (LuaMuxState){
      .package = object->package,
      .object = object->object,
      .generation = object->generation,
  };
  memcpy(handle->name_space, name_space, length);
  handle->name_space[length] = '\0';
  luaL_getmetatable(state, LUA_MUX_STATE_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

static void lua_mux_push_state_value(lua_State *state,
                                     const ObjectStateValue *value) {
  switch (value->type) {
  case OBJECT_STATE_STRING:
    lua_pushlstring(state, value->as.string.data, value->as.string.length);
    break;
  case OBJECT_STATE_BOOLEAN:
    lua_pushboolean(state, value->as.boolean);
    break;
  case OBJECT_STATE_INTEGER:
    lua_pushinteger(state, (lua_Integer)value->as.integer);
    break;
  case OBJECT_STATE_NUMBER:
    lua_pushnumber(state, (lua_Number)value->as.number);
    break;
  }
}

static const char *lua_mux_state_key(lua_State *state, int argument) {
  size_t length;
  const char *key = luaL_checklstring(state, argument, &length);

  if (length > 255 || memchr(key, '\0', length) ||
      !object_state_name_is_valid(key))
    luaL_argerror(state, argument, "invalid state key");
  return key;
}

static bool lua_mux_read_state_value(lua_State *state, int argument,
                                     ObjectStateValue *value) {
  memset(value, 0, sizeof(*value));
  switch (lua_type(state, argument)) {
  case LUA_TSTRING:
    value->type = OBJECT_STATE_STRING;
    value->as.string.data =
        lua_tolstring(state, argument, &value->as.string.length);
    return true;
  case LUA_TBOOLEAN:
    value->type = OBJECT_STATE_BOOLEAN;
    value->as.boolean = lua_toboolean(state, argument);
    return true;
  case LUA_TNUMBER: {
    lua_Number number = lua_tonumber(state, argument);
    lua_Integer integer = lua_tointeger(state, argument);

    if (!isfinite((double)number))
      return false;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wfloat-equal"
    if ((lua_Number)integer == number) {
#pragma clang diagnostic pop
      value->type = OBJECT_STATE_INTEGER;
      value->as.integer = (int64_t)integer;
    } else {
      value->type = OBJECT_STATE_NUMBER;
      value->as.number = (double)number;
    }
    return true;
  }
  default:
    return false;
  }
}

static int lua_mux_state_get(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  const ObjectStateValue *value =
      transaction->depth
          ? object_state_transaction_get(transaction, handle->object,
                                         handle->name_space, key)
          : object_state_get(handle->package->services->database,
                             handle->object, handle->name_space, key);

  if (value)
    lua_mux_push_state_value(state, value);
  else if (lua_gettop(state) >= 3)
    lua_pushvalue(state, 3);
  else
    lua_pushnil(state);
  return 1;
}

static int lua_mux_state_has(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;

  lua_pushboolean(
      state, (transaction->depth
                  ? object_state_transaction_get(transaction, handle->object,
                                                 handle->name_space, key)
                  : object_state_get(handle->package->services->database,
                                     handle->object, handle->name_space,
                                     key)) != nullptr);
  return 1;
}

static int lua_mux_state_set(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateValue value;
  char error[256];

  if (lua_isnil(state, 3)) {
    object_state_transaction_delete(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key);
    return 0;
  }
  if (!lua_mux_read_state_value(state, 3, &value))
    return luaL_argerror(
        state, 3, "state values must be strings, booleans, or finite numbers");
  if (!object_state_transaction_set(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key,
                                    &value, error, sizeof(error)))
    return luaL_error(state, "%s", error);
  return 0;
}

static int lua_mux_state_delete(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  const char *key = lua_mux_state_key(state, 2);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  bool existed =
      (transaction->depth
           ? object_state_transaction_get(transaction, handle->object,
                                          handle->name_space, key)
           : object_state_get(handle->package->services->database,
                              handle->object, handle->name_space, key)) !=
      nullptr;

  if (existed)
    object_state_transaction_delete(&handle->package->state_transaction,
                                    handle->object, handle->name_space, key);
  lua_pushboolean(state, existed);
  return 1;
}

static int lua_mux_state_keys(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  size_t count;

  if (!transaction->depth)
    return luaL_error(state, "state enumeration requires an active callback");
  count = object_state_transaction_count(transaction, handle->object,
                                         handle->name_space);

  lua_createtable(state, (int)count, 0);
  for (size_t index = 0; index < count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_transaction_entry(transaction, handle->object,
                                        handle->name_space, index, &entry))
      return luaL_error(state, "state changed during enumeration");
    lua_pushstring(state, entry.key);
    lua_rawseti(state, -2, (int)index + 1);
  }
  return 1;
}

static int lua_mux_state_entries(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  ObjectStateTransaction *transaction = &handle->package->state_transaction;
  size_t count;

  if (!transaction->depth)
    return luaL_error(state, "state enumeration requires an active callback");
  count = object_state_transaction_count(transaction, handle->object,
                                         handle->name_space);

  lua_createtable(state, (int)count, 0);
  for (size_t index = 0; index < count; index++) {
    ObjectStateEntryView entry;

    if (!object_state_transaction_entry(transaction, handle->object,
                                        handle->name_space, index, &entry))
      return luaL_error(state, "state changed during enumeration");
    lua_createtable(state, 0, 2);
    lua_pushstring(state, entry.key);
    lua_setfield(state, -2, "key");
    lua_mux_push_state_value(state, entry.value);
    lua_setfield(state, -2, "value");
    lua_rawseti(state, -2, (int)index + 1);
  }
  return 1;
}

static int lua_mux_state_get_many(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);
  size_t count;

  luaL_checktype(state, 2, LUA_TTABLE);
  count = lua_objlen(state, 2);
  lua_createtable(state, 0, (int)count);
  for (size_t index = 1; index <= count; index++) {
    const char *key;
    const ObjectStateValue *value;

    lua_rawgeti(state, 2, (int)index);
    key = lua_mux_state_key(state, -1);
    ObjectStateTransaction *transaction = &handle->package->state_transaction;
    value = transaction->depth
                ? object_state_transaction_get(transaction, handle->object,
                                               handle->name_space, key)
                : object_state_get(handle->package->services->database,
                                   handle->object, handle->name_space, key);
    if (value) {
      lua_mux_push_state_value(state, value);
      lua_setfield(state, -3, key);
    }
    lua_pop(state, 1);
  }
  return 1;
}

static int lua_mux_state_set_many(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);

  luaL_checktype(state, 2, LUA_TTABLE);
  lua_pushnil(state);
  while (lua_next(state, 2) != 0) {
    ObjectStateValue value;
    const char *key;
    char error[256];

    if (lua_type(state, -2) != LUA_TSTRING)
      return luaL_error(state, "state update keys must be strings");
    key = lua_mux_state_key(state, -2);
    if (lua_isnil(state, -1)) {
      object_state_transaction_delete(&handle->package->state_transaction,
                                      handle->object, handle->name_space, key);
    } else {
      if (!lua_mux_read_state_value(state, -1, &value))
        return luaL_error(
            state, "state values must be strings, booleans, or finite numbers");
      if (!object_state_transaction_set(&handle->package->state_transaction,
                                        handle->object, handle->name_space, key,
                                        &value, error, sizeof(error)))
        return luaL_error(state, "%s", error);
    }
    lua_pop(state, 1);
  }
  return 0;
}

static int lua_mux_state_tostring(lua_State *state) {
  LuaMuxState *handle = lua_mux_check_state(state, 1);

  lua_pushfstring(state, "state(#%d, %s)", (int)handle->object,
                  handle->name_space);
  return 1;
}

static int lua_mux_notify(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  const char *message = luaL_checkstring(state, 2);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.notify is unavailable during @lua/check");
  object = lua_mux_require_object(package, state, 1);
  notify(&package->services->background_command->evaluation, object, message);
  return 0;
}

static int lua_mux_connected_players(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  Descriptor *descriptor;
  DescriptorIterator iterator =
      descriptor_iterator_connected(package->services->descriptors);
  int index = 1;

  lua_newtable(state);
  while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr) {
    lua_newtable(state);
    lua_mux_push_object(state, package, descriptor->player);
    lua_setfield(state, -2, "object");
    lua_pushstring(state, game_object_name(package->services->database,
                                           descriptor->player));
    lua_setfield(state, -2, "name");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->connected_at));
    lua_setfield(state, -2, "connected_for");
    lua_pushinteger(state, (lua_Integer)(package->services->clock->now -
                                         descriptor->last_time));
    lua_setfield(state, -2, "idle_for");
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

static int lua_mux_who_summary(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);

  lua_newtable(state);
  lua_pushinteger(state, 0);
  lua_setfield(state, -2, "hidden");
  lua_pushinteger(state, *package->services->record_players);
  lua_setfield(state, -2, "record");
  if (package->services->configuration->max_players == -1)
    lua_pushnil(state);
  else
    lua_pushinteger(state, package->services->configuration->max_players);
  lua_setfield(state, -2, "maximum");
  return 1;
}

static int lua_mux_flow_start(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  int descriptor_id = (int)luaL_checkinteger(state, 1);
  const char *module = luaL_checkstring(state, 2);
  const char *first_step = luaL_checkstring(state, 3);

  if (lua_mux_package_is_checking(package))
    return luaL_error(state, "mux.flow_start is unavailable during @lua/check");
  if (!package->flow_start)
    return luaL_error(state, "mux.flow_start is unavailable");
  return package->flow_start(package->context, state, descriptor_id, module,
                             first_step);
}

void lua_mux_package_install(lua_State *state, LuaMuxPackage *package) {
  object_state_transaction_initialize(&package->state_transaction);

  luaL_newmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_object_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_object_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_object_state);
  lua_setfield(state, -2, "state");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_contents, 1);
  lua_setfield(state, -2, "contents");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_contents_visible, 1);
  lua_setfield(state, -2, "contents_visible");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exits, 1);
  lua_setfield(state, -2, "exits");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exits_visible, 1);
  lua_setfield(state, -2, "exits_visible");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_exit_enter_lock_passes, 1);
  lua_setfield(state, -2, "enter_lock_passes");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_STATE_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_state_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_state_get);
  lua_setfield(state, -2, "get");
  lua_pushcfunction(state, lua_mux_state_has);
  lua_setfield(state, -2, "has");
  lua_pushcfunction(state, lua_mux_state_set);
  lua_setfield(state, -2, "set");
  lua_pushcfunction(state, lua_mux_state_delete);
  lua_setfield(state, -2, "delete");
  lua_pushcfunction(state, lua_mux_state_keys);
  lua_setfield(state, -2, "keys");
  lua_pushcfunction(state, lua_mux_state_entries);
  lua_setfield(state, -2, "entries");
  lua_pushcfunction(state, lua_mux_state_get_many);
  lua_setfield(state, -2, "get_many");
  lua_pushcfunction(state, lua_mux_state_set_many);
  lua_setfield(state, -2, "set_many");
  lua_pop(state, 1);

  lua_newtable(state);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object, 1);
  lua_setfield(state, -2, "object");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_markup, 1);
  lua_setfield(state, -2, "markup");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_style, 1);
  lua_setfield(state, -2, "style");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_strip_style, 1);
  lua_setfield(state, -2, "strip_style");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_text_width, 1);
  lua_setfield(state, -2, "text_width");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_truncate_text, 1);
  lua_setfield(state, -2, "truncate_text");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_notify, 1);
  lua_setfield(state, -2, "notify");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_connected_players, 1);
  lua_setfield(state, -2, "connected_players");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_who_summary, 1);
  lua_setfield(state, -2, "who_summary");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_flow_start, 1);
  lua_setfield(state, -2, "flow_start");
  lua_setglobal(state, "mux");
}
