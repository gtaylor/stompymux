/* mux_object_bindings.c - Lua bindings for MUX objects. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/owned_text.h"
#include "mux/support/stringutil.h"
#include "mux/support/styled_text/markup.h"
#include "mux/support/utf8.h"
#include "mux/support/validation.h"
#include "mux/world/access.h"
#include "mux/world/object_spatial.h"
#include "mux/world/player.h"

// Stack index and public argument number have distinct error-reporting roles.
// NOLINTBEGIN(bugprone-easily-swappable-parameters)
DbRef lua_mux_require_object_at(LuaMuxPackage *package, lua_State *state,
                                int index, int argument, const char *label) {
  DbRef object;
  LuaMuxObject *handle = luaL_testudata(state, index, LUA_MUX_OBJECT_METATABLE);

  if (handle) {
    if (handle->package != package)
      lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                    "%s belongs to another Lua runtime", label);
    if (!is_good_obj(package->services->database, handle->object) ||
        game_object_generation(package->services->database, handle->object) !=
            handle->generation)
      lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                    "%s no longer exists", label);
    object = handle->object;
  } else {
    if (!lua_isnumber(state, index))
      lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                    "%s must be a dbref or Object", label);
    object = (DbRef)lua_tointeger(state, index);
  }

  if (!is_good_obj(package->services->database, object))
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "%s is invalid", label);
  return object;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

DbRef lua_mux_require_object(LuaMuxPackage *package, lua_State *state,
                             int argument) {
  return lua_mux_require_object_at(package, state, argument, argument,
                                   "object");
}

LuaMuxObject *lua_mux_push_object(lua_State *state, LuaMuxPackage *package,
                                  DbRef object) {
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

LuaMuxObject *lua_mux_check_object_handle(lua_State *state, int argument) {
  LuaMuxObject *handle =
      luaL_checkudata(state, argument, LUA_MUX_OBJECT_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

LuaMuxState *lua_mux_check_state(lua_State *state, int argument) {
  LuaMuxState *handle =
      luaL_checkudata(state, argument, LUA_MUX_STATE_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

LuaMuxAttribute *lua_mux_check_attribute(lua_State *state, int argument) {
  LuaMuxAttribute *handle =
      luaL_checkudata(state, argument, LUA_MUX_ATTRIBUTE_METATABLE);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

/**
 * Creates a validated handle for a native database object.
 *
 * @par Lua name `mux.world.object`
 * @par Lua signature `mux.world.object( dbref )`
 * @par Lua parameters - `dbref` (`integer|Object`) A live database reference or
 * an existing object handle.
 * @par Lua returns - `object` (`Object`): A handle for the referenced object.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale or invalid dbref/handle.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;

  lua_mux_require_runtime(package, state, "world.object");
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

/**
 * Returns the objects directly contained by this object.
 *
 * @par Lua name `object:contents`
 * @par Lua signature `object:contents( )`
 * @par Lua parameters - None.
 * @par Lua returns - `contents` (`table`): An array of Object handles in native
 * database order.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid handle or an object that
 * cannot contain objects.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_contents(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef member;
  int index = 1;

  lua_mux_require_runtime(package, state, "contents");
  object = lua_mux_require_object(package, state, 1);
  if (!has_contents(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot contain other objects");
  lua_newtable(state);
  DOLIST(package->services->database, member,
         game_object_contents(package->services->database, object)) {
    lua_mux_push_object(state, package, member);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

/**
 * Tests whether a directly contained object is visible to a viewer.
 *
 * @par Lua name `object:contents_visible`
 * @par Lua signature `object:contents_visible( viewer, member )`
 * @par Lua parameters - `viewer` (`number|Object`) The object viewing the
 * container.
 * - `member` (`number|Object`) An object directly contained by the receiver.
 * @par Lua returns - `visible` (`boolean`): Whether native look rules expose
 * the member.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for invalid handles, a non-container, or a
 * member not directly contained.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
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
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot contain other objects");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_contents(package->services->database, container), member))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not directly contained");
  evaluation = &package->services->background_command->evaluation;
  can_see_location = ((!is_dark(package->services->database, container)) != 0);
  lua_pushboolean(state, can_see(&(ObjectVisibilityRequest){
                             .evaluation = evaluation,
                             .viewer = viewer,
                             .object = member,
                             .location_visible = can_see_location}));
  return 1;
}

/**
 * Returns the exits directly attached to this object.
 *
 * @par Lua name `object:exits`
 * @par Lua signature `object:exits( )`
 * @par Lua parameters - None.
 * @par Lua returns - `exits` (`table`): An array of Object handles in native
 * database order.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid handle or an object that
 * cannot have exits.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_exits(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  DbRef exit;
  int index = 1;

  lua_mux_require_runtime(package, state, "exits");
  object = lua_mux_require_object(package, state, 1);
  if (!has_exits(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot have exits");
  lua_newtable(state);
  DOLIST(package->services->database, exit,
         game_object_exits(package->services->database, object)) {
    lua_mux_push_object(state, package, exit);
    lua_rawseti(state, -2, index++);
  }
  return 1;
}

/**
 * Tests whether a directly attached exit is visible to a viewer.
 *
 * @par Lua name `object:exits_visible`
 * @par Lua signature `object:exits_visible( viewer, exit )`
 * @par Lua parameters - `viewer` (`number|Object`) The object viewing the
 * location.
 * - `exit` (`number|Object`) An exit directly attached to the receiver.
 * @par Lua returns - `visible` (`boolean`): Whether native exit-display rules
 * expose the exit.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for invalid handles, an invalid
 * location/exit, or an exit not directly attached.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
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
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object cannot have exits");
  if (!is_exit(package->services->database, exit))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object is not an exit");
  if (!lua_mux_list_contains(
          package->services->database,
          game_object_exits(package->services->database, location), exit))
    return lua_error_arg(state, 3, LUA_ERROR_CODE_OBJECT_INVALID,
                         "exit is not directly attached");
  if (is_dark(package->services->database, location))
    key |= VE_LOC_DARK;
  lua_pushboolean(state, exit_displayable(&(ExitVisibilityRequest){
                             .database = package->services->database,
                             .exit = exit,
                             .viewer = viewer,
                             .options = key}));
  return 1;
}

/**
 * Returns this object's current name.
 *
 * @par Lua name `object:name`
 * @par Lua signature `object:name( )`
 * @par Lua parameters - None.
 * @par Lua returns - `name` (`string`): The current stored object name.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_name(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushstring(state, game_object_name(handle->package->services->database,
                                         handle->object));
  return 1;
}

/**
 * Returns this object's native database reference.
 *
 * @par Lua name `object:dbref`
 * @par Lua signature `object:dbref( )`
 * @par Lua parameters - None.
 * @par Lua returns - `dbref` (`integer`): The native database reference.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_dbref(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushinteger(state, handle->object);
  return 1;
}

/**
 * Returns this object's native object type.
 *
 * @par Lua name `object:type`
 * @par Lua signature `object:type( )`
 * @par Lua parameters - None.
 * @par Lua returns - `type` (`string|nil`): `room`, `thing`, `exit`, or
 * `player`, or nil for an unrecognized native object type.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_type(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);
  GameDatabase *database = handle->package->services->database;

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

static char *lua_mux_object_compile_name(lua_State *state,
                                         LuaMuxPackage *package, int argument) {
  size_t length;
  const char *name;
  char error[256];
  char *compiled;

  if (lua_type(state, argument) != LUA_TSTRING)
    lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                  "name must be a string");
  name = lua_tolstring(state, argument, &length);
  if (strlen(name) != length)
    lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                  "name contains an embedded NUL byte");
  if (!utf8_validate(name, length))
    lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                  "name is not valid UTF-8");

  compiled = alloc_lbuf("lua_mux_object_compile_name");
  if (!styled_text_compile(package->services->styled_text_palette, name,
                           compiled, LBUF_SIZE, error, sizeof(error))) {
    free_buf(compiled);
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "name has invalid styled-text markup: %s", error);
    return nullptr;
  }
  if (!string_copy_bounded(compiled, LBUF_SIZE, name)) {
    free_buf(compiled);
    (void)lua_error_arg(state, argument, LUA_ERROR_CODE_ARG_INVALID,
                        "name is too long");
    return nullptr;
  }
  return compiled;
}

/**
 * Changes this object's name using native object-name validation.
 *
 * @par Lua name `object:set_name`
 * @par Lua signature `object:set_name( name )`
 * @par Lua parameters - `name` (`string`) The new UTF-8 object name,
 * optionally containing valid styled-text markup.
 * @par Lua returns - No values.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for an invalid, unavailable, or duplicate
 * name.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object.
 * - `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` when the object is being destroyed.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_set_name(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;
  WorldContext *world = package->services->background_command->world;
  char clear[LBUF_SIZE];

  lua_mux_require_runtime(package, state, "object:set_name");
  DbRef object = lua_mux_require_object(package, state, 1);
  if (is_going(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "name is required");
  char *compiled = lua_mux_object_compile_name(state, package, 2);

  if (!compiled)
    return 0;

  styled_text_strip(package->services->styled_text_palette, compiled, clear,
                    sizeof(clear));
  if (is_player(database, object)) {
    OwnedText trimmed = trim_spaces(clear);
    const char *current = game_object_pure_name(database, object);

    if (!ok_player_name(package->services->configuration, trimmed.text) ||
        !badname_check(world, trimmed.text)) {
      owned_text_release(&trimmed);
      free_buf(compiled);
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "name is not a valid player name");
    }
    if (string_compare(package->services->configuration, trimmed.text,
                       current) &&
        lookup_player(world, NOTHING, trimmed.text, 0) != NOTHING) {
      owned_text_release(&trimmed);
      free_buf(compiled);
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "player name is already in use");
    }
    (void)delete_player_name(world, object, current);
    object_name_set(database, object, compiled);
    (void)add_player_name(world, object,
                          game_object_pure_name(database, object));
    owned_text_release(&trimmed);
  } else {
    if (!ok_name(package->services->configuration, clear)) {
      free_buf(compiled);
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "name is not a valid object name");
    }
    object_name_set(database, object, compiled);
  }
  free_buf(compiled);
  return 0;
}

/**
 * Formats the Object userdata.
 *
 * @par Lua name `Object.__tostring`
 * @par Lua signature `tostring(object)`
 * @par Lua parameters - `object` (`Object`): The validated object handle.
 * @par Lua returns - `text` (`string`): `object(#dbref)`.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` when the handle is stale.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_tostring(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushfstring(state, "object(#%d)", (int)handle->object);
  return 1;
}

/**
 * Compares Object identity without revalidating object lifetime.
 *
 * @par Lua name `Object.__eq`
 * @par Lua signature `left == right`
 * @par Lua parameters - `left` (`Object`): The left handle.
 * - `right` (`Object`): The right handle.
 * @par Lua returns - `equal` (`boolean`): True when runtime, dbref, and
 * generation all match.
 * @par Lua errors - A Lua type error when either operand is not an Object.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
 */
static int lua_mux_object_equal(lua_State *state) {
  LuaMuxObject *left = luaL_checkudata(state, 1, LUA_MUX_OBJECT_METATABLE);
  LuaMuxObject *right = luaL_checkudata(state, 2, LUA_MUX_OBJECT_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->object == right->object &&
                             left->generation == right->generation);
  return 1;
}

void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_object_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_object_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_name, 1);
  lua_setfield(state, -2, "name");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_dbref, 1);
  lua_setfield(state, -2, "dbref");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_type, 1);
  lua_setfield(state, -2, "type");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_name, 1);
  lua_setfield(state, -2, "set_name");
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
  lua_mux_install_object_relationship_bindings(state, package);
  lua_pop(state, 1);

  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object, 1);
  lua_setfield(state, -2, "object");
}
