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

static const char LUA_MUX_OBJECT_TYPE_NAMESPACE_METATABLE[] =
    "btmux.object_type_namespace";
static const char LUA_MUX_OBJECT_TYPE_METATABLE[] = "btmux.object_type";

typedef struct LuaMuxObjectType LuaMuxObjectType;
struct LuaMuxObjectType {
  LuaMuxPackage *package;
  int type;
  const char *name;
};

typedef struct LuaMuxObjectTypeNamespace LuaMuxObjectTypeNamespace;
struct LuaMuxObjectTypeNamespace {
  LuaMuxPackage *package;
};

static const char *lua_mux_object_type_name(int type) {
  switch (type) {
  case OBJECT_TYPE_ROOM:
    return "ROOM";
  case OBJECT_TYPE_THING:
    return "THING";
  case OBJECT_TYPE_EXIT:
    return "EXIT";
  case OBJECT_TYPE_PLAYER:
    return "PLAYER";
  default:
    return nullptr;
  }
}

static void lua_mux_push_object_type(lua_State *state, LuaMuxPackage *package,
                                     int type) {
  LuaMuxObjectType *object_type = lua_newuserdata(state, sizeof(*object_type));

  *object_type = (LuaMuxObjectType){
      .package = package,
      .type = type,
      .name = lua_mux_object_type_name(type),
  };
  luaL_getmetatable(state, LUA_MUX_OBJECT_TYPE_METATABLE);
  lua_setmetatable(state, -2);
}

static int lua_mux_object_type_tostring(lua_State *state) {
  LuaMuxObjectType *object_type =
      luaL_checkudata(state, 1, LUA_MUX_OBJECT_TYPE_METATABLE);

  lua_pushstring(state, object_type->name);
  return 1;
}

static int lua_mux_object_type_equal(lua_State *state) {
  LuaMuxObjectType *left =
      luaL_checkudata(state, 1, LUA_MUX_OBJECT_TYPE_METATABLE);
  LuaMuxObjectType *right =
      luaL_checkudata(state, 2, LUA_MUX_OBJECT_TYPE_METATABLE);

  lua_pushboolean(state,
                  left->package == right->package && left->type == right->type);
  return 1;
}

static int lua_mux_object_type_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "mux.world.types constants are immutable");
}

static int lua_mux_object_type_namespace_index(lua_State *state) {
  LuaMuxObjectTypeNamespace *name_space =
      luaL_checkudata(state, 1, LUA_MUX_OBJECT_TYPE_NAMESPACE_METATABLE);

  if (lua_type(state, 2) != LUA_TSTRING)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "object type name must be a string");
  const char *name = lua_tostring(state, 2);
  if (!strcmp(name, "ROOM"))
    lua_mux_push_object_type(state, name_space->package, OBJECT_TYPE_ROOM);
  else if (!strcmp(name, "THING"))
    lua_mux_push_object_type(state, name_space->package, OBJECT_TYPE_THING);
  else if (!strcmp(name, "EXIT"))
    lua_mux_push_object_type(state, name_space->package, OBJECT_TYPE_EXIT);
  else if (!strcmp(name, "PLAYER"))
    lua_mux_push_object_type(state, name_space->package, OBJECT_TYPE_PLAYER);
  else
    return lua_error_arg(state, 2, LUA_ERROR_CODE_ARG_INVALID,
                         "unknown object type '%s'", name);
  return 1;
}

// Stack index and public argument number have distinct error-reporting roles.
// NOLINTBEGIN(bugprone-easily-swappable-parameters)
int lua_mux_require_object_type_at(LuaMuxPackage *package, lua_State *state,
                                   int index, int argument, const char *label) {
  LuaMuxObjectType *object_type =
      luaL_testudata(state, index, LUA_MUX_OBJECT_TYPE_METATABLE);

  if (!object_type || object_type->package != package)
    return lua_error_arg(
        state, argument, LUA_ERROR_CODE_ARG_INVALID,
        "%s must be a mux.world.types constant from this runtime", label);
  return object_type->type;
}
// NOLINTEND(bugprone-easily-swappable-parameters)

bool lua_mux_object_type_filter_matches(const LuaMuxObjectTypeFilter *filter,
                                        int type) {
  if (!filter->enabled)
    return true;
  switch (type) {
  case OBJECT_TYPE_ROOM:
    return filter->rooms;
  case OBJECT_TYPE_THING:
    return filter->things;
  case OBJECT_TYPE_EXIT:
    return filter->exits;
  case OBJECT_TYPE_PLAYER:
    return filter->players;
  default:
    return false;
  }
}

static void lua_mux_object_type_filter_add(LuaMuxObjectTypeFilter *filter,
                                           int type) {
  switch (type) {
  case OBJECT_TYPE_ROOM:
    filter->rooms = true;
    break;
  case OBJECT_TYPE_THING:
    filter->things = true;
    break;
  case OBJECT_TYPE_EXIT:
    filter->exits = true;
    break;
  case OBJECT_TYPE_PLAYER:
    filter->players = true;
    break;
  default:
    break;
  }
}

static bool lua_mux_object_type_array_key(lua_State *state, size_t count) {
  if (lua_type(state, -2) != LUA_TNUMBER)
    return false;
  lua_Integer key = lua_tointeger(state, -2);
  lua_Number number = lua_tonumber(state, -2);

  // Type filters are deliberately strict arrays: reject holes and hash keys.
  // The ordered comparisons test integrality without float equality.
  return (key >= 1 && (size_t)key <= count && number >= (lua_Number)key &&
          number <= (lua_Number)key) != 0;
}

void lua_mux_object_type_filter_parse(LuaMuxPackage *package, lua_State *state,
                                      int options,
                                      LuaMuxObjectTypeFilter *filter) {
  if (options < 0 && options > LUA_REGISTRYINDEX)
    options = lua_gettop(state) + options + 1;
  lua_getfield(state, options, "types");
  if (lua_isnil(state, -1)) {
    lua_pop(state, 1);
    return;
  }

  if (!lua_istable(state, -1)) {
    (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                        "options.types must be an array");
    return;
  }
  filter->enabled = true;
  size_t count = lua_objlen(state, -1);
  size_t entries = 0;
  lua_pushnil(state);
  while (lua_next(state, -2) != 0) {
    if (!lua_mux_object_type_array_key(state, count)) {
      (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                          "options.types must be a dense array");
      return;
    }
    int type = lua_mux_require_object_type_at(package, state, -1, options,
                                              "options.types entries");
    lua_mux_object_type_filter_add(filter, type);
    entries++;
    lua_pop(state, 1);
  }
  if (entries != count) {
    (void)lua_error_arg(state, options, LUA_ERROR_CODE_ARG_INVALID,
                        "options.types must be a dense array");
    return;
  }
  lua_pop(state, 1);
}

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

/**
 * Returns objects directly contained by or attached to this object.
 *
 * @par Lua name `object:contents`
 * @par Lua signature `object:contents( options? )`
 * @par Lua parameters - `options` (`ContentsOptions|nil`) Optional filters.
 * `types` is an array of typed constants from `mux.world.types`, and
 * `visible_to` is a dbref or Object whose native visibility rules are applied.
 * @par Lua returns - `contents` (`table`): Matching Object handles. Ordinary
 * contents precede attached exits; each group retains native database order.
 * @par Lua errors - `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * - `LUA_ERROR_CODE_ARG_INVALID` for malformed options or object type
 * constants not owned by this runtime.
 * - `LUA_ERROR_CODE_OBJECT_INVALID` for an invalid receiver/viewer or an
 * object that can hold neither contents nor exits.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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
/** Lua `object:description()`: returns styled text or nil when unset.
 * @param[in,out] state Lua state. @return One Lua value.
 * @exception LUA_ERROR_CODE_OBJECT_INVALID The Object is stale. */
static int lua_mux_object_description(lua_State *state) {
  return lua_mux_object_push_description(state, false);
}
/** Lua `object:internal_description()`: returns styled text or nil when unset.
 * @param[in,out] state Lua state. @return One Lua value.
 * @exception LUA_ERROR_CODE_OBJECT_INVALID The Object is stale. */
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
/** Lua `object:set_description(value)`: sets styled text; nil/empty clears.
 * @param[in,out] state Lua state. @return No Lua values. @exception
 * LUA_ERROR_CODE_OBJECT_INVALID Stale Object. @exception
 * LUA_ERROR_CODE_OBJECT_UNAVAILABLE Object being destroyed. @exception
 * LUA_ERROR_CODE_ARG_INVALID Invalid text or markup. @exception
 * LUA_ERROR_CODE_CHECKING_UNAVAILABLE Called during `@lua/check`. */
static int lua_mux_object_set_description(lua_State *state) {
  return lua_mux_object_set_description_value(state, false);
}
/** Lua `object:set_internal_description(value)`: sets styled text; nil clears.
 * @param[in,out] state Lua state. @return No Lua values. @exception
 * LUA_ERROR_CODE_OBJECT_INVALID Stale Object. @exception
 * LUA_ERROR_CODE_OBJECT_UNAVAILABLE Object being destroyed. @exception
 * LUA_ERROR_CODE_ARG_INVALID Invalid text or markup. @exception
 * LUA_ERROR_CODE_CHECKING_UNAVAILABLE Called during `@lua/check`. */
static int lua_mux_object_set_internal_description(lua_State *state) {
  return lua_mux_object_set_description_value(state, true);
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
 * @par Lua returns - `type` (`ObjectType|nil`): The corresponding typed
 * constant from `mux.world.types`, or nil for an unrecognized native type.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object.
 * @param[in,out] state The Lua state whose arguments are read and results are
 * pushed.
 * @return The number of Lua values pushed onto the stack.
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

/**
 * Installs Object methods and typed native object-kind constants.
 *
 * @par Lua name `mux.world.types`
 * @par Lua constants Immutable `ObjectType` constants `ROOM`, `THING`, `EXIT`,
 * and `PLAYER`. Their string forms are their uppercase names, and equality
 * compares native type identity within the current runtime.
 * @par Lua errors - `LUA_ERROR_CODE_ARG_INVALID` for unknown or non-string
 * lookups and attempted mutation.
 * @param[in,out] state Lua state whose top value is the `mux.world` table.
 * @param[in,out] package Package owning the constants.
 */
void lua_mux_install_object_bindings(lua_State *state, LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_OBJECT_TYPE_METATABLE);
  lua_pushcfunction(state, lua_mux_object_type_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_object_type_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_object_type_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_OBJECT_TYPE_NAMESPACE_METATABLE);
  lua_pushcfunction(state, lua_mux_object_type_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_object_type_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

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

  LuaMuxObjectTypeNamespace *name_space =
      lua_newuserdata(state, sizeof(*name_space));

  *name_space = (LuaMuxObjectTypeNamespace){.package = package};
  luaL_getmetatable(state, LUA_MUX_OBJECT_TYPE_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "types");
}
