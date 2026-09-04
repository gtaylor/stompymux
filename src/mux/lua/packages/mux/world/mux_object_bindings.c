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

/**
 * Creates a validated handle for a native database object.
 *
 * @par LuaLS definition mux callable mux.world.object
 * @code{.lua}
 * ---Creates a validated object handle from a dbref or existing handle.
 * ---@param dbref DbRef|Object
 * ---@return Object object
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * function mux_world.object(dbref) end
 * @endcode
 */
static int lua_mux_object(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;

  lua_mux_require_runtime(package, state, "world.object");
  object = lua_mux_require_object(package, state, 1);
  lua_mux_push_object(state, package, object);
  return 1;
}

/**
 * Returns objects directly contained by or attached to this object.
 *
 * @par LuaLS definition mux callable Object:contents
 * @code{.lua}
 * ---Returns matching objects directly contained by or attached to this object.
 * ---@param options? ContentsOptions Optional type and visibility filters.
 * ---@return Object[] contents
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), or [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * function Object:contents(options) end
 * @endcode
 */
static int lua_mux_contents(lua_State *state) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  GameDatabase *database = package->services->database;
  static const char *const FIELDS[] = {"types", "visible_to"};
  LuaMuxObjectTypeFilter filter = {
      .enabled = false,
      .rooms = false,
      .things = false,
      .exits = false,
      .players = false,
  };
  DbRef object;
  DbRef viewer = NOTHING;
  DbRef member;
  bool filter_visibility = false;
  int index = 1;

  lua_mux_require_runtime(package, state, "contents");
  object = lua_mux_require_object(package, state, 1);
  if (!has_contents(database, object) && !has_exits(database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_INVALID,
                         "object can hold neither contents nor exits");

  if (!lua_isnoneornil(state, 2)) {
    lua_mux_check_options(state, 2, FIELDS, sizeof(FIELDS) / sizeof(*FIELDS));
    lua_mux_object_type_filter_parse(package, state, 2, &filter);
    viewer = lua_mux_option_object(package, state, 2, "visible_to", false,
                                   &filter_visibility);
  }

  lua_newtable(state);
  if (has_contents(database, object)) {
    bool location_visible = is_dark(database, object) == 0;

    DOLIST(database, member, game_object_contents(database, object)) {
      if (!lua_mux_object_type_filter_matches(&filter,
                                              typeof_obj(database, member)))
        continue;
      if (filter_visibility &&
          !can_see(&(ObjectVisibilityRequest){
              .evaluation = &package->services->background_command->evaluation,
              .viewer = viewer,
              .object = member,
              .location_visible = location_visible}))
        continue;
      lua_mux_push_object(state, package, member);
      lua_rawseti(state, -2, index++);
    }
  }
  if (has_exits(database, object)) {
    int visibility_options = is_dark(database, object) ? VE_LOC_DARK : 0;

    DOLIST(database, member, game_object_exits(database, object)) {
      if (!lua_mux_object_type_filter_matches(&filter,
                                              typeof_obj(database, member)))
        continue;
      if (filter_visibility && !exit_displayable(&(ExitVisibilityRequest){
                                   .database = database,
                                   .exit = member,
                                   .viewer = viewer,
                                   .options = visibility_options}))
        continue;
      lua_mux_push_object(state, package, member);
      lua_rawseti(state, -2, index++);
    }
  }
  return 1;
}

/**
 * Returns this object's current name.
 *
 * @par LuaLS definition mux callable Object:name
 * @code{.lua}
 * ---Returns this object's current stored name.
 * ---@return string name
 * ---
 * ---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.object.invalid
 * function Object:name() end
 * @endcode
 */
static int lua_mux_object_name(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushstring(state, game_object_name(handle->package->services->database,
                                         handle->object));
  return 1;
}
static int lua_mux_object_push_description(lua_State *state, bool internal) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);
  const char *description =
      internal ? game_object_internal_description(
                     handle->package->services->database, handle->object)
               : game_object_description(handle->package->services->database,
                                         handle->object);

  if (description)
    lua_pushstring(state, description);
  else
    lua_pushnil(state);
  return 1;
}
/**
 * @par LuaLS definition mux callable Object:description
 * @code{.lua}
 * ---Returns this object's styled description, or nil when it is unset.
 * ---@return string? description
 * ---
 * ---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
 * ---for a stale Object.
 * ---@see mux.error.codes.object.invalid
 * function Object:description() end
 * @endcode
 */
static int lua_mux_object_description(lua_State *state) {
  return lua_mux_object_push_description(state, false);
}
/**
 * @par LuaLS definition mux callable Object:internal_description
 * @code{.lua}
 * ---Returns this object's styled internal description, or nil when it is unset.
 * ---@return string? description
 * ---
 * ---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
 * ---for a stale Object.
 * ---@see mux.error.codes.object.invalid
 * function Object:internal_description() end
 * @endcode
 */
static int lua_mux_object_internal_description(lua_State *state) {
  return lua_mux_object_push_description(state, true);
}
static int lua_mux_object_set_description_value(lua_State *state,
                                                bool internal) {
  LuaMuxPackage *package = lua_mux_package_get(state);
  DbRef object;
  const char *description = nullptr;
  size_t length = 0;
  char compiled[LBUF_SIZE];
  char error[256];

  lua_mux_require_runtime(package, state,
                          internal ? "object:set_internal_description"
                                   : "object:set_description");
  object = lua_mux_require_object(package, state, 1);
  if (is_going(package->services->database, object))
    return lua_error_arg(state, 1, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                         "object is being destroyed");
  if (lua_gettop(state) < 2)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "description is required; pass nil to clear it");
  if (!lua_isnil(state, 2)) {
    description = luaL_checklstring(state, 2, &length);
    if (length >= LBUF_SIZE || memchr(description, '\0', length) ||
        !utf8_validate(description, length))
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "description must be valid UTF-8 without embedded "
                           "NUL bytes and shorter than LBUF_SIZE");
    if (!styled_text_compile(package->services->styled_text_palette,
                             description, compiled, sizeof(compiled), error,
                             sizeof(error)))
      return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                           "description has invalid styled-text markup: %s",
                           error);
  }
  if (internal)
    game_object_internal_description_set(package->services->database, object,
                                         description);
  else
    game_object_description_set(package->services->database, object,
                                description);
  return 0;
}
/**
 * @par LuaLS definition mux callable Object:set_description
 * @code{.lua}
 * ---Sets this object's styled description. Nil or an empty string clears it.
 * ---@param description string|nil Valid UTF-8 styled-text markup without embedded NUL bytes, or nil to clear the description. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
 * ---for a stale Object,
 * ---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
 * ---when the object is being destroyed, or
 * ---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for text
 * ---that is too long or has invalid UTF-8 or styled-text markup.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * ---@see mux.error.codes.arg.invalid
 * function Object:set_description(description) end
 * @endcode
 */
static int lua_mux_object_set_description(lua_State *state) {
  return lua_mux_object_set_description_value(state, false);
}
/**
 * @par LuaLS definition mux callable Object:set_internal_description
 * @code{.lua}
 * ---Sets this object's styled internal description. Nil or an empty string clears it.
 * ---@param description string|nil Valid UTF-8 styled-text markup without embedded NUL bytes, or nil to clear the internal description. This argument must be supplied explicitly.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---during `@lua/check`, [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
 * ---for a stale Object,
 * ---[`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable)
 * ---when the object is being destroyed, or
 * ---[`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for text
 * ---that is too long or has invalid UTF-8 or styled-text markup.
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * ---@see mux.error.codes.arg.invalid
 * function Object:set_internal_description(description) end
 * @endcode
 */
static int lua_mux_object_set_internal_description(lua_State *state) {
  return lua_mux_object_set_description_value(state, true);
}

/**
 * Returns this object's native database reference.
 *
 * @par LuaLS definition mux callable Object:dbref
 * @code{.lua}
 * ---Returns this object's native database reference.
 * ---@return DbRef dbref
 * ---
 * ---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.object.invalid
 * function Object:dbref() end
 * @endcode
 */
static int lua_mux_object_dbref(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);

  lua_pushinteger(state, handle->object);
  return 1;
}

/**
 * Returns this object's native object type.
 *
 * @par LuaLS definition mux callable Object:type
 * @code{.lua}
 * ---Returns this object's native object type.
 * ---@return ObjectType? type
 * ---
 * ---Raises [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid).
 * ---@see mux.error.codes.object.invalid
 * function Object:type() end
 * @endcode
 */
static int lua_mux_object_type(lua_State *state) {
  LuaMuxObject *handle = lua_mux_check_object_handle(state, 1);
  int type = typeof_obj(handle->package->services->database, handle->object);

  if (!lua_mux_object_type_name(type))
    lua_pushnil(state);
  else
    lua_mux_push_object_type(state, handle->package, type);
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
 * @par LuaLS definition mux callable Object:set_name
 * @code{.lua}
 * ---Changes this object's name using native object-name validation.
 * ---@param name string New UTF-8 name, optionally containing styled-text markup.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid), [`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid), or [`mux.error.codes.object.unavailable`](lua://mux.error.codes.object.unavailable).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.arg.invalid
 * ---@see mux.error.codes.object.invalid
 * ---@see mux.error.codes.object.unavailable
 * function Object:set_name(name) end
 * @endcode
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
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the Object class declaration.
 *
 * Raises `LUA_ERROR_CODE_OBJECT_INVALID` when the handle is stale.
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
 * @par LuaLS ignore mux __eq -- LuaCATS has no equality-operator declaration; Object equality semantics are documented on the class.
 *
 * Raises a Lua type error when either operand is not an Object.
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

/**
 * Installs Object methods.
 *
 * @par LuaLS definition mux type object
 * @code{.lua}
 * ---A generation-checked native database object handle. Native equality compares
 * ---object identity. `tostring` returns `object(#<dbref>)` and raises
 * ---[`mux.error.codes.object.invalid`](lua://mux.error.codes.object.invalid)
 * ---when the handle is stale.
 * ---@class Object
 * local Object = {}
 * @endcode
 *
 * @param[in,out] state Lua state whose top value is the `mux.world` table.
 * @param[in,out] package Package owning the constants.
 */
void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package) {
  lua_mux_install_object_type_bindings(state, package);

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
  lua_pushcfunction(state, lua_mux_object_description);
  lua_setfield(state, -2, "description");
  lua_pushcfunction(state, lua_mux_object_internal_description);
  lua_setfield(state, -2, "internal_description");
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
  lua_pushcclosure(state, lua_mux_object_set_description, 1);
  lua_setfield(state, -2, "set_description");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object_set_internal_description, 1);
  lua_setfield(state, -2, "set_internal_description");
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_contents, 1);
  lua_setfield(state, -2, "contents");
  lua_mux_install_object_relationship_bindings(state, package);
  lua_pop(state, 1);

  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_object, 1);
  lua_setfield(state, -2, "object");
}
