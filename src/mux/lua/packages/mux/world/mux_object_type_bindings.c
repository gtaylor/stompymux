/* mux_object_type_bindings.c - Lua bindings for MUX object types. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"

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

const char *lua_mux_object_type_name(int type) {
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

void lua_mux_push_object_type(lua_State *state, LuaMuxPackage *package,
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

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the ObjectType class declaration.
 */
static int lua_mux_object_type_tostring(lua_State *state) {
  LuaMuxObjectType *object_type =
      luaL_checkudata(state, 1, LUA_MUX_OBJECT_TYPE_METATABLE);

  lua_pushstring(state, object_type->name);
  return 1;
}

/**
 * @par LuaLS ignore mux __eq -- LuaCATS has no equality-operator declaration; ObjectType equality semantics are documented on the class.
 */
static int lua_mux_object_type_equal(lua_State *state) {
  LuaMuxObjectType *left =
      luaL_checkudata(state, 1, LUA_MUX_OBJECT_TYPE_METATABLE);
  LuaMuxObjectType *right =
      luaL_checkudata(state, 2, LUA_MUX_OBJECT_TYPE_METATABLE);

  lua_pushboolean(state,
                  left->package == right->package && left->type == right->type);
  return 1;
}

/**
 * @par LuaLS ignore mux __newindex -- Immutability is represented by the ObjectType class and ObjectTypeNamespace table declarations.
 */
static int lua_mux_object_type_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "mux.world.types constants are immutable");
}

/**
 * @par LuaLS definition mux catalog mux.world.types
 * @code{.lua}
 * ---Immutable namespace of typed native object kinds.
 * ---
 * ---Raises [`mux.error.codes.arg.invalid`](lua://mux.error.codes.arg.invalid) for
 * ---unknown or non-string keys and attempted mutation.
 * ---@class (exact) ObjectTypeNamespace
 * ---@field ROOM RoomObjectType Detached room kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
 * ---@field THING ThingObjectType Contained thing kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
 * ---@field EXIT ExitObjectType Attached exit kind accepted by [`mux.world.create_object`](lua://mux.world.create_object).
 * ---@field PLAYER PlayerObjectType Player kind; existing players may have this type, but scripts cannot create them.
 * ---@see mux.error.codes.arg.invalid
 * @endcode
 *
 * @par LuaLS ignore mux __index -- Dynamic lookup is represented by the ObjectTypeNamespace table declaration.
 */
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

/**
 * Installs typed native object-kind constants.
 *
 * @par LuaLS definition mux type object.kinds
 * @code{.lua}
 * ---A typed native object kind obtained from [`mux.world.types`](lua://mux.world.types).
 * ---Its string form is its uppercase name, and equality compares native type
 * ---identity within the current runtime.
 * ---@class ObjectType
 *
 * ---The `ROOM` object-kind constant from the current runtime.
 * ---@class RoomObjectType: ObjectType
 *
 * ---The `THING` object-kind constant from the current runtime.
 * ---@class ThingObjectType: ObjectType
 *
 * ---The `EXIT` object-kind constant from the current runtime.
 * ---@class ExitObjectType: ObjectType
 *
 * ---The `PLAYER` object-kind constant from the current runtime.
 * ---@class PlayerObjectType: ObjectType
 * @endcode
 *
 * @param[in,out] state Lua state whose top value is the `mux.world` table.
 * @param[in,out] package Package owning the constants.
 */
void lua_mux_install_object_type_bindings(lua_State *state,
                                          LuaMuxPackage *package) {
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

  LuaMuxObjectTypeNamespace *name_space =
      lua_newuserdata(state, sizeof(*name_space));

  *name_space = (LuaMuxObjectTypeNamespace){.package = package};
  luaL_getmetatable(state, LUA_MUX_OBJECT_TYPE_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "types");
}
