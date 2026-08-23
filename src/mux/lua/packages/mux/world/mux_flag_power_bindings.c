/* mux_flag_power_bindings.c - Lua bindings for object flags and powers. */

#include <lauxlib.h>
#include <lua.h>
#include <string.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"

static const char LUA_MUX_FLAG_NAMESPACE_METATABLE[] = "btmux.flag_namespace";
static const char LUA_MUX_POWER_NAMESPACE_METATABLE[] = "btmux.power_namespace";

typedef struct LuaMuxConstantNamespace LuaMuxConstantNamespace;
struct LuaMuxConstantNamespace {
  LuaMuxPackage *package;
  bool powers;
};

static LuaMuxObjectSet *lua_mux_check_object_set(lua_State *state, int argument,
                                                 const char *metatable) {
  LuaMuxObjectSet *handle = luaL_checkudata(state, argument, metatable);

  if (!is_good_obj(handle->package->services->database, handle->object) ||
      game_object_generation(handle->package->services->database,
                             handle->object) != handle->generation)
    lua_error_arg(state, argument, LUA_ERROR_CODE_OBJECT_INVALID,
                  "object no longer exists");
  return handle;
}

static void lua_mux_push_upper_name(lua_State *state, const char *name) {
  char canonical[SBUF_SIZE];
  size_t length = strlen(name);

  if (length >= sizeof(canonical))
    lua_error_raise(state, LUA_ERROR_CODE_INTERNAL,
                    "native flag or power name is too long");
  for (size_t index = 0; index < length; index++) {
    char character = *(const char *)checked_storage_at_const(
        name, length, sizeof(char), index);
    char *output =
        checked_storage_at(canonical, sizeof(canonical), sizeof(char), index);

    *output = character >= 'a' && character <= 'z'
                  ? (char)(character - ('a' - 'A'))
                  : character;
  }
  *(char *)checked_storage_at(canonical, sizeof(canonical), sizeof(char),
                              length) = '\0';
  lua_pushlstring(state, canonical, length);
}

static bool lua_mux_name_matches_constant(const char *requested,
                                          const char *native) {
  size_t requested_length = strlen(requested);
  size_t native_length = strlen(native);

  if (requested_length != native_length)
    return false;
  for (size_t index = 0; index < native_length; index++) {
    char character = *(const char *)checked_storage_at_const(
        native, native_length, sizeof(char), index);
    char canonical = character >= 'a' && character <= 'z'
                         ? (char)(character - ('a' - 'A'))
                         : character;

    if (*(const char *)checked_storage_at_const(
            requested, requested_length, sizeof(char), index) != canonical)
      return false;
  }
  return true;
}

static void lua_mux_push_named_constant(lua_State *state,
                                        LuaMuxPackage *package, int id,
                                        const char *name, bool powers) {
  LuaMuxNamedConstant *constant = lua_newuserdata(state, sizeof(*constant));

  *constant = (LuaMuxNamedConstant){
      .package = package,
      .id = id,
      .name = name,
  };
  luaL_getmetatable(state,
                    powers ? LUA_MUX_POWER_METATABLE : LUA_MUX_FLAG_METATABLE);
  lua_setmetatable(state, -2);
}

static LuaMuxNamedConstant *lua_mux_check_named_constant(lua_State *state,
                                                         int argument,
                                                         LuaMuxPackage *package,
                                                         bool powers) {
  LuaErrorCode code =
      powers ? LUA_ERROR_CODE_POWER_INVALID : LUA_ERROR_CODE_FLAG_INVALID;
  const char *metatable =
      powers ? LUA_MUX_POWER_METATABLE : LUA_MUX_FLAG_METATABLE;
  LuaMuxNamedConstant *constant = luaL_testudata(state, argument, metatable);

  if (!constant || constant->package != package)
    lua_error_arg(state, argument, code, "expected a mux.world.%s constant",
                  powers ? "powers" : "flags");
  return constant;
}

static int lua_mux_constant_tostring(lua_State *state) {
  LuaMuxNamedConstant *constant = lua_touserdata(state, 1);

  lua_mux_push_upper_name(state, constant->name);
  return 1;
}

static int lua_mux_constant_equal(lua_State *state) {
  LuaMuxNamedConstant *left = lua_touserdata(state, 1);
  LuaMuxNamedConstant *right = lua_touserdata(state, 2);

  lua_pushboolean(state,
                  left->package == right->package && left->id == right->id);
  return 1;
}

static int lua_mux_constant_newindex(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_ARG_INVALID,
                         "flag and power constants are immutable");
}

static int lua_mux_constant_namespace_index(lua_State *state) {
  LuaMuxConstantNamespace *name_space = lua_touserdata(state, 1);
  LuaErrorCode code = name_space->powers ? LUA_ERROR_CODE_POWER_INVALID
                                         : LUA_ERROR_CODE_FLAG_INVALID;
  const char *name = lua_tostring(state, 2);

  if (!name)
    return lua_error_arg(state, 2, code, "constant name must be a string");
  if (name_space->powers) {
    for (size_t index = 0; index < object_power_entry_count(); index++) {
      const POWERENT *entry = object_power_entry_at(index);

      if (lua_mux_name_matches_constant(name, entry->powername)) {
        lua_mux_push_named_constant(state, name_space->package, entry->id,
                                    entry->powername, true);
        return 1;
      }
    }
  } else {
    for (size_t index = 0; index < object_flag_entry_count(); index++) {
      const FlagEntry *entry = object_flag_entry_at(index);

      if (lua_mux_name_matches_constant(name, entry->flagname)) {
        lua_mux_push_named_constant(state, name_space->package, entry->id,
                                    entry->flagname, false);
        return 1;
      }
    }
  }
  return lua_error_arg(state, 2, code, "unknown %s constant '%s'",
                       name_space->powers ? "power" : "flag", name);
}

static int lua_mux_constant_namespace_newindex(lua_State *state) {
  LuaMuxConstantNamespace *name_space = lua_touserdata(state, 1);

  return lua_error_raise(state,
                         name_space->powers ? LUA_ERROR_CODE_POWER_INVALID
                                            : LUA_ERROR_CODE_FLAG_INVALID,
                         "mux.world.%s constants are immutable",
                         name_space->powers ? "powers" : "flags");
}

static void lua_mux_push_object_set(lua_State *state, LuaMuxObject *object,
                                    const char *metatable) {
  LuaMuxObjectSet *handle = lua_newuserdata(state, sizeof(*handle));

  *handle = (LuaMuxObjectSet){
      .package = object->package,
      .object = object->object,
      .generation = object->generation,
  };
  luaL_getmetatable(state, metatable);
  lua_setmetatable(state, -2);
}

/**
 * Creates a flag collection handle for this object.
 *
 * @par Lua name `object:flags`
 * @par Lua signature `object:flags( )`
 * @par Lua parameters - None.
 * @par Lua returns - `flags` (`Flags`): The object's flag collection.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_flags(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);

  lua_mux_require_runtime(object->package, state, "object:flags");
  lua_mux_push_object_set(state, object, LUA_MUX_FLAGS_METATABLE);
  return 1;
}

/**
 * Creates a power collection handle for this object.
 *
 * @par Lua name `object:powers`
 * @par Lua signature `object:powers( )`
 * @par Lua parameters - None.
 * @par Lua returns - `powers` (`Powers`): The object's power collection.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Object;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`.
 * @par Lua availability Available only at runtime; unavailable during
 * `@lua/check`.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_object_powers(lua_State *state) {
  LuaMuxObject *object = lua_mux_check_object_handle(state, 1);

  lua_mux_require_runtime(object->package, state, "object:powers");
  lua_mux_push_object_set(state, object, LUA_MUX_POWERS_METATABLE);
  return 1;
}

/**
 * Lists the flags currently set on an object.
 *
 * @par Lua name `flags:list`
 * @par Lua signature `flags:list( )`
 * @par Lua parameters - None.
 * @par Lua returns - `values` (`table`): An array of `Flag` constants in
 * native registry order.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Flags handle.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_flags_list(lua_State *state) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_FLAGS_METATABLE);
  GameDatabase *database = handle->package->services->database;
  int output = 1;

  lua_newtable(state);
  for (size_t index = 0; index < object_flag_entry_count(); index++) {
    const FlagEntry *entry = object_flag_entry_at(index);

    if (!game_object_has_flag(&(ObjectFlagRequest){
            .database = database, .object = handle->object, .flag = entry->id}))
      continue;
    lua_mux_push_named_constant(state, handle->package, entry->id,
                                entry->flagname, false);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

/**
 * Lists the powers currently granted to an object.
 *
 * @par Lua name `powers:list`
 * @par Lua signature `powers:list( )`
 * @par Lua parameters - None.
 * @par Lua returns - `values` (`table`): An array of `Power` constants in
 * native registry order.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Powers handle.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_powers_list(lua_State *state) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_POWERS_METATABLE);
  GameDatabase *database = handle->package->services->database;
  int output = 1;

  lua_newtable(state);
  for (size_t index = 0; index < object_power_entry_count(); index++) {
    const POWERENT *entry = object_power_entry_at(index);

    if (!game_object_has_power(&(ObjectPowerRequest){.database = database,
                                                     .object = handle->object,
                                                     .power = entry->id}))
      continue;
    lua_mux_push_named_constant(state, handle->package, entry->id,
                                entry->powername, true);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

/**
 * Tests whether an object has a flag.
 *
 * @par Lua name `flags:has`
 * @par Lua signature `flags:has( flag )`
 * @par Lua parameters - `flag` (`Flag`) A `mux.world.flags` constant.
 * @par Lua returns - `present` (`boolean`): Whether the flag is set.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Flags handle;
 * `LUA_ERROR_CODE_FLAG_INVALID` for a value that is not a Flag constant.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_flags_has(lua_State *state) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_FLAGS_METATABLE);
  LuaMuxNamedConstant *constant =
      lua_mux_check_named_constant(state, 2, handle->package, false);

  lua_pushboolean(state, game_object_has_flag(&(ObjectFlagRequest){
                             .database = handle->package->services->database,
                             .object = handle->object,
                             .flag = (ObjectFlag)constant->id}));
  return 1;
}

/**
 * Tests whether an object has a power.
 *
 * @par Lua name `powers:has`
 * @par Lua signature `powers:has( power )`
 * @par Lua parameters - `power` (`Power`) A `mux.world.powers` constant.
 * @par Lua returns - `present` (`boolean`): Whether the power is granted.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Powers handle;
 * `LUA_ERROR_CODE_POWER_INVALID` for a value that is not a Power constant.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_powers_has(lua_State *state) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_POWERS_METATABLE);
  LuaMuxNamedConstant *constant =
      lua_mux_check_named_constant(state, 2, handle->package, true);

  lua_pushboolean(state, game_object_has_power(&(ObjectPowerRequest){
                             .database = handle->package->services->database,
                             .object = handle->object,
                             .power = (PowerId)constant->id}));
  return 1;
}

static int lua_mux_flags_change(lua_State *state, bool value) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_FLAGS_METATABLE);
  LuaMuxNamedConstant *constant =
      lua_mux_check_named_constant(state, 2, handle->package, false);
  GameDatabase *database = handle->package->services->database;
  const FlagEntry *entry = nullptr;
  bool current;

  lua_mux_require_runtime(handle->package, state,
                          value ? "flags:add" : "flags:remove");
  for (size_t index = 0; index < object_flag_entry_count(); index++) {
    const FlagEntry *candidate = object_flag_entry_at(index);

    if (candidate->id == (ObjectFlag)constant->id) {
      entry = candidate;
      break;
    }
  }
  if (!entry)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_FLAG_INVALID,
                         "flag constant is no longer registered");
  current = game_object_has_flag(&(ObjectFlagRequest){
      .database = database, .object = handle->object, .flag = entry->id});
  if (current == value) {
    lua_pushboolean(state, false);
    return 1;
  }
  if (!entry->handler(&(FlagChangeRequest){
          .evaluation =
              &handle->package->services->background_command->evaluation,
          .target = handle->object,
          .player = GOD,
          .flag = entry->id,
          .clear = (bool)!value}))
    return lua_error_raise(state, LUA_ERROR_CODE_OBJECT_UNAVAILABLE,
                           "native flag policy rejected the change");
  lua_pushboolean(state, game_object_has_flag(&(ObjectFlagRequest){
                             .database = database,
                             .object = handle->object,
                             .flag = entry->id}) != current);
  return 1;
}

/**
 * Adds a flag and reports whether the object changed.
 *
 * @par Lua name `flags:add`
 * @par Lua signature `flags:add( flag )`
 * @par Lua parameters - `flag` (`Flag`) A `mux.world.flags` constant.
 * @par Lua returns - `changed` (`boolean`): True when the flag was newly set.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Flags handle;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_FLAG_INVALID` for a value that is not a Flag constant;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` when native policy rejects the change.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_flags_add(lua_State *state) {
  return lua_mux_flags_change(state, true);
}

/**
 * Removes a flag and reports whether the object changed.
 *
 * @par Lua name `flags:remove`
 * @par Lua signature `flags:remove( flag )`
 * @par Lua parameters - `flag` (`Flag`) A `mux.world.flags` constant.
 * @par Lua returns - `changed` (`boolean`): True when the flag was removed.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Flags handle;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_FLAG_INVALID` for a value that is not a Flag constant;
 * `LUA_ERROR_CODE_OBJECT_UNAVAILABLE` when native policy rejects the change.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_flags_remove(lua_State *state) {
  return lua_mux_flags_change(state, false);
}

static int lua_mux_powers_change(lua_State *state, bool value) {
  LuaMuxObjectSet *handle =
      lua_mux_check_object_set(state, 1, LUA_MUX_POWERS_METATABLE);
  LuaMuxNamedConstant *constant =
      lua_mux_check_named_constant(state, 2, handle->package, true);
  GameDatabase *database = handle->package->services->database;
  PowerId power = (PowerId)constant->id;
  bool current;

  lua_mux_require_runtime(handle->package, state,
                          value ? "powers:add" : "powers:remove");
  current = game_object_has_power(&(ObjectPowerRequest){
      .database = database, .object = handle->object, .power = power});
  if (current == value) {
    lua_pushboolean(state, false);
    return 1;
  }
  game_object_set_power(
      &(ObjectPowerChange){.target = {.database = database,
                                      .object = handle->object,
                                      .power = power},
                           .value = value});
  lua_pushboolean(state, true);
  return 1;
}

/**
 * Grants a power and reports whether the object changed.
 *
 * @par Lua name `powers:add`
 * @par Lua signature `powers:add( power )`
 * @par Lua parameters - `power` (`Power`) A `mux.world.powers` constant.
 * @par Lua returns - `changed` (`boolean`): True when newly granted.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Powers handle;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_POWER_INVALID` for a value that is not a Power constant.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_powers_add(lua_State *state) {
  return lua_mux_powers_change(state, true);
}

/**
 * Removes a power and reports whether the object changed.
 *
 * @par Lua name `powers:remove`
 * @par Lua signature `powers:remove( power )`
 * @par Lua parameters - `power` (`Power`) A `mux.world.powers` constant.
 * @par Lua returns - `changed` (`boolean`): True when removed.
 * @par Lua errors - `LUA_ERROR_CODE_OBJECT_INVALID` for a stale Powers handle;
 * `LUA_ERROR_CODE_CHECKING_UNAVAILABLE` during `@lua/check`;
 * `LUA_ERROR_CODE_POWER_INVALID` for a value that is not a Power constant.
 * @param[in,out] state Lua state.
 * @return The number of Lua values pushed.
 */
static int lua_mux_powers_remove(lua_State *state) {
  return lua_mux_powers_change(state, false);
}

static int lua_mux_object_set_tostring(lua_State *state) {
  bool powers = luaL_testudata(state, 1, LUA_MUX_POWERS_METATABLE) != nullptr;
  LuaMuxObjectSet *handle = lua_mux_check_object_set(
      state, 1, powers ? LUA_MUX_POWERS_METATABLE : LUA_MUX_FLAGS_METATABLE);

  lua_pushfstring(state, powers ? "powers(#%d)" : "flags(#%d)",
                  (int)handle->object);
  return 1;
}

static void lua_mux_install_constant_metatable(lua_State *state,
                                               const char *metatable) {
  luaL_newmetatable(state, metatable);
  lua_pushcfunction(state, lua_mux_constant_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_constant_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_constant_newindex);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);
}

static void lua_mux_install_namespace_metatable(lua_State *state,
                                                const char *metatable) {
  luaL_newmetatable(state, metatable);
  lua_pushcfunction(state, lua_mux_constant_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_constant_namespace_newindex);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);
}

static void
lua_mux_install_object_set_metatable(lua_State *state, const char *metatable,
                                     lua_CFunction list, lua_CFunction has,
                                     lua_CFunction add, lua_CFunction remove) {
  luaL_newmetatable(state, metatable);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_object_set_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, list);
  lua_setfield(state, -2, "list");
  lua_pushcfunction(state, has);
  lua_setfield(state, -2, "has");
  lua_pushcfunction(state, add);
  lua_setfield(state, -2, "add");
  lua_pushcfunction(state, remove);
  lua_setfield(state, -2, "remove");
  lua_pop(state, 1);
}

static void lua_mux_install_constant_namespace(lua_State *state,
                                               LuaMuxPackage *package,
                                               bool powers) {
  LuaMuxConstantNamespace *name_space =
      lua_newuserdata(state, sizeof(*name_space));

  *name_space = (LuaMuxConstantNamespace){
      .package = package,
      .powers = powers,
  };
  luaL_getmetatable(state, powers ? LUA_MUX_POWER_NAMESPACE_METATABLE
                                  : LUA_MUX_FLAG_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, powers ? "powers" : "flags");
}

void lua_mux_install_flag_power_bindings(lua_State *state,
                                         LuaMuxPackage *package) {
  lua_mux_install_constant_metatable(state, LUA_MUX_FLAG_METATABLE);
  lua_mux_install_constant_metatable(state, LUA_MUX_POWER_METATABLE);
  lua_mux_install_namespace_metatable(state, LUA_MUX_FLAG_NAMESPACE_METATABLE);
  lua_mux_install_namespace_metatable(state, LUA_MUX_POWER_NAMESPACE_METATABLE);
  lua_mux_install_object_set_metatable(state, LUA_MUX_FLAGS_METATABLE,
                                       lua_mux_flags_list, lua_mux_flags_has,
                                       lua_mux_flags_add, lua_mux_flags_remove);
  lua_mux_install_object_set_metatable(
      state, LUA_MUX_POWERS_METATABLE, lua_mux_powers_list, lua_mux_powers_has,
      lua_mux_powers_add, lua_mux_powers_remove);

  luaL_getmetatable(state, LUA_MUX_OBJECT_METATABLE);
  lua_pushcfunction(state, lua_mux_object_flags);
  lua_setfield(state, -2, "flags");
  lua_pushcfunction(state, lua_mux_object_powers);
  lua_setfield(state, -2, "powers");
  lua_pop(state, 1);

  lua_mux_install_constant_namespace(state, package, false);
  lua_mux_install_constant_namespace(state, package, true);
}
