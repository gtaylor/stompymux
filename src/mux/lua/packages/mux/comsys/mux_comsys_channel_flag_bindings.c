/* mux_comsys_channel_flag_bindings.c - Lua channel flag bindings. */

#include <lauxlib.h>
#include <lua.h>
#include <stdint.h>
#include <string.h>

#include "mux/communication/comsys.h"
#include "mux/lua/lua_error.h"
#include "mux/lua/lua_error_codes.h"
#include "mux/lua/packages/mux/comsys/mux_comsys_bindings_internal.h"
#include "mux/lua/packages/mux/mux_package.h"
#include "mux/lua/packages/mux/mux_package_internal.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"

static const char LUA_MUX_CHANNEL_FLAGS_METATABLE[] = "btmux.channel_flags";
static const char LUA_MUX_CHANNEL_FLAG_METATABLE[] = "btmux.channel_flag";
static const char LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE[] =
    "btmux.channel_flag_namespace";

typedef struct LuaMuxChannelFlags LuaMuxChannelFlags;
struct LuaMuxChannelFlags {
  LuaMuxPackage *package;
  struct Channel *identity;
  uint64_t generation;
  char name[CHAN_NAME_LEN];
};

typedef struct LuaMuxChannelFlag LuaMuxChannelFlag;
struct LuaMuxChannelFlag {
  LuaMuxPackage *package;
  int value;
  const char *name;
};

typedef struct LuaMuxChannelFlagNamespace LuaMuxChannelFlagNamespace;
struct LuaMuxChannelFlagNamespace {
  LuaMuxPackage *package;
};

typedef struct ChannelFlagDefinition ChannelFlagDefinition;
struct ChannelFlagDefinition {
  int value;
  const char *name;
};

/**
 * @par LuaLS definition mux catalog mux.comsys.flags
 * @code{.lua}
 * ---Immutable namespace of supported communication-channel flags. Unknown or
 * ---non-string lookups and attempted mutation raise
 * ---[`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
 * ---@class (exact) ChannelFlagNamespace
 * ---@field PUBLIC ChannelFlag Makes the channel visible without a successful join lock.
 * ---@field LOUD ChannelFlag Announces applicable connection and presence changes.
 * ---@field TRANSPARENT ChannelFlag Relaxes hidden-member filtering in native channel displays.
 * ---@see mux.error.codes.channel_flag.invalid
 * @endcode
 */
static const ChannelFlagDefinition CHANNEL_FLAGS[] = {
    {CHANNEL_PUBLIC, "PUBLIC"},
    {CHANNEL_LOUD, "LOUD"},
    {CHANNEL_TRANSPARENT, "TRANSPARENT"},
};

static const ChannelFlagDefinition *lua_mux_channel_flag_at(size_t index) {
  return checked_storage_at_const(
      CHANNEL_FLAGS, sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS),
      sizeof(*CHANNEL_FLAGS), index);
}

static LuaMuxChannelFlags *lua_mux_check_channel_flags(lua_State *state,
                                                       int argument) {
  LuaMuxChannelFlags *handle =
      luaL_checkudata(state, argument, LUA_MUX_CHANNEL_FLAGS_METATABLE);

  lua_mux_require_runtime(handle->package, state, "comsys.Channel:flags");
  (void)lua_mux_check_channel_identity(handle->package, state, argument,
                                       handle->name, handle->identity,
                                       handle->generation);
  return handle;
}

static void lua_mux_push_channel_flag(lua_State *state, LuaMuxPackage *package,
                                      const ChannelFlagDefinition *definition) {
  LuaMuxChannelFlag *flag = lua_newuserdata(state, sizeof(*flag));

  *flag = (LuaMuxChannelFlag){
      .package = package,
      .value = definition->value,
      .name = definition->name,
  };
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAG_METATABLE);
  lua_setmetatable(state, -2);
}

static LuaMuxChannelFlag *lua_mux_check_channel_flag(lua_State *state,
                                                     int argument,
                                                     LuaMuxPackage *package) {
  LuaMuxChannelFlag *flag =
      luaL_testudata(state, argument, LUA_MUX_CHANNEL_FLAG_METATABLE);

  if (flag == nullptr || flag->package != package)
    lua_error_arg(state, argument, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                  "expected a mux.comsys.flags constant");
  return flag;
}

/**
 * @par LuaLS definition mux callable Channel:flags
 * @code{.lua}
 * ---Opens the live administrative flag collection for this channel.
 * ---@return ChannelFlags flags
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function Channel:flags() end
 * @endcode
 */
static int lua_mux_channel_flags(lua_State *state) {
  LuaMuxChannel *handle = lua_mux_check_channel(state, 1);
  LuaMuxChannelFlags *flags = lua_newuserdata(state, sizeof(*flags));

  *flags = (LuaMuxChannelFlags){
      .package = handle->package,
      .identity = handle->identity,
      .generation = handle->generation,
  };
  (void)string_copy_bounded(flags->name, sizeof(flags->name), handle->name);
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAGS_METATABLE);
  lua_setmetatable(state, -2);
  return 1;
}

/**
 * @par LuaLS ignore mux __newindex -- Immutability is represented by the ChannelFlag class and ChannelFlagNamespace table declarations.
 */
static int lua_mux_channel_flag_immutable(lua_State *state) {
  return lua_error_raise(state, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                         "channel flag values are immutable");
}

/**
 * Lists set flags in `PUBLIC`, `LOUD`, `TRANSPARENT` order.
 *
 * @par LuaLS definition mux callable ChannelFlags:list
 * @code{.lua}
 * ---Lists set flags in `PUBLIC`, `LOUD`, `TRANSPARENT` order.
 * ---@return ChannelFlag[] flags
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking) or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * function ChannelFlags:list() end
 * @endcode
 */
static int lua_mux_channel_flags_list(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  int output = 1;

  lua_newtable(state);
  for (size_t index = 0; index < sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS);
       index++) {
    const ChannelFlagDefinition *definition = lua_mux_channel_flag_at(index);

    if ((handle->identity->type & definition->value) == 0)
      continue;
    lua_mux_push_channel_flag(state, handle->package, definition);
    lua_rawseti(state, -2, output++);
  }
  return 1;
}

/**
 * Tests whether the channel has a typed flag.
 *
 * @par LuaLS definition mux callable ChannelFlags:has
 * @code{.lua}
 * ---Tests whether the channel has a typed flag.
 * ---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
 * ---@return boolean present
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.channel_flag.invalid
 * function ChannelFlags:has(flag) end
 * @endcode
 */
static int lua_mux_channel_flags_has(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  LuaMuxChannelFlag *flag =
      lua_mux_check_channel_flag(state, 2, handle->package);

  lua_pushboolean(state, (handle->identity->type & flag->value) != 0);
  return 1;
}

static int lua_mux_channel_flags_change(lua_State *state, bool enabled) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);
  LuaMuxChannelFlag *flag =
      lua_mux_check_channel_flag(state, 2, handle->package);
  bool current = (handle->identity->type & flag->value) != 0;

  if (current == enabled) {
    lua_pushboolean(state, 0);
    return 1;
  }
  if (enabled)
    handle->identity->type |= flag->value;
  else
    handle->identity->type &= ~flag->value;
  lua_pushboolean(state, 1);
  return 1;
}

/**
 * Sets a typed channel flag.
 *
 * @par LuaLS definition mux callable ChannelFlags:add
 * @code{.lua}
 * ---Sets a typed channel flag.
 * ---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
 * ---@return boolean changed Whether the flag changed from unset to set.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.channel_flag.invalid
 * function ChannelFlags:add(flag) end
 * @endcode
 */
static int lua_mux_channel_flags_add(lua_State *state) {
  return lua_mux_channel_flags_change(state, true);
}

/**
 * Clears a typed channel flag.
 *
 * @par LuaLS definition mux callable ChannelFlags:remove
 * @code{.lua}
 * ---Clears a typed channel flag.
 * ---@param flag ChannelFlag Constant from [`mux.comsys.flags`](lua://mux.comsys.flags).
 * ---@return boolean changed Whether the flag changed from set to unset.
 * ---
 * ---Raises [`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking), [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid), or [`mux.error.codes.channel_flag.invalid`](lua://mux.error.codes.channel_flag.invalid).
 * ---@see mux.error.codes.unavailable.checking
 * ---@see mux.error.codes.channel.invalid
 * ---@see mux.error.codes.channel_flag.invalid
 * function ChannelFlags:remove(flag) end
 * @endcode
 */
static int lua_mux_channel_flags_remove(lua_State *state) {
  return lua_mux_channel_flags_change(state, false);
}

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the ChannelFlags class declaration.
 */
static int lua_mux_channel_flags_tostring(lua_State *state) {
  LuaMuxChannelFlags *handle = lua_mux_check_channel_flags(state, 1);

  lua_pushfstring(state, "channel_flags(%s)", handle->name);
  return 1;
}

/**
 * @par LuaLS ignore mux __tostring -- String conversion is represented by the ChannelFlag class declaration.
 */
static int lua_mux_channel_flag_tostring(lua_State *state) {
  LuaMuxChannelFlag *flag =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_METATABLE);

  lua_pushstring(state, flag->name);
  return 1;
}

/**
 * @par LuaLS ignore mux __eq -- LuaCATS has no equality-operator declaration; ChannelFlag equality semantics are documented on the class.
 */
static int lua_mux_channel_flag_equal(lua_State *state) {
  LuaMuxChannelFlag *left =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_METATABLE);
  LuaMuxChannelFlag *right =
      luaL_checkudata(state, 2, LUA_MUX_CHANNEL_FLAG_METATABLE);

  lua_pushboolean(state, left->package == right->package &&
                             left->value == right->value);
  return 1;
}

/**
 * @par LuaLS ignore mux __index -- Dynamic lookup is represented by the ChannelFlagNamespace table declaration.
 */
static int lua_mux_channel_flag_namespace_index(lua_State *state) {
  LuaMuxChannelFlagNamespace *name_space =
      luaL_checkudata(state, 1, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  const char *name = lua_tostring(state, 2);

  if (name == nullptr)
    return lua_error_arg(state, 2, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                         "channel flag name must be a string");
  for (size_t index = 0; index < sizeof(CHANNEL_FLAGS) / sizeof(*CHANNEL_FLAGS);
       index++) {
    const ChannelFlagDefinition *definition = lua_mux_channel_flag_at(index);

    if (strcmp(name, definition->name) == 0) {
      lua_mux_push_channel_flag(state, name_space->package, definition);
      return 1;
    }
  }
  return lua_error_arg(state, 2, LUA_ERROR_CODE_CHANNEL_FLAG_INVALID,
                       "unknown channel flag constant '%s'", name);
}

/**
 * Installs typed channel-flag methods and constants.
 *
 * @par LuaLS definition mux type channel.flag
 * @code{.lua}
 * ---A typed communication-channel flag constant from
 * ---[`mux.comsys.flags`](lua://mux.comsys.flags). Its string form is the
 * ---canonical uppercase name, and equality compares identity within the current
 * ---Lua runtime.
 * ---@class ChannelFlag
 * @endcode
 *
 * @par LuaLS definition mux type channel.flag_set
 * @code{.lua}
 * ---A live view of one channel's administrative flags. It becomes stale when
 * ---its originating channel is destroyed. `tostring` returns
 * ---`channel_flags(<name>)` and can raise
 * ---[`mux.error.codes.unavailable.checking`](lua://mux.error.codes.unavailable.checking)
 * ---or [`mux.error.codes.channel.invalid`](lua://mux.error.codes.channel.invalid).
 * ---@class ChannelFlags
 * local ChannelFlags = {}
 * @endcode
 *
 * @param[in,out] state Lua state whose top value is the `mux.comsys` table.
 * @param[in,out] package Package owning the flag constants.
 */
void lua_mux_install_channel_flag_bindings(lua_State *state,
                                           LuaMuxPackage *package) {
  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAGS_METATABLE);
  lua_pushvalue(state, -1);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_channel_flags_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_channel_flags_list);
  lua_setfield(state, -2, "list");
  lua_pushcfunction(state, lua_mux_channel_flags_has);
  lua_setfield(state, -2, "has");
  lua_pushcfunction(state, lua_mux_channel_flags_add);
  lua_setfield(state, -2, "add");
  lua_pushcfunction(state, lua_mux_channel_flags_remove);
  lua_setfield(state, -2, "remove");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAG_METATABLE);
  lua_pushcfunction(state, lua_mux_channel_flag_tostring);
  lua_setfield(state, -2, "__tostring");
  lua_pushcfunction(state, lua_mux_channel_flag_equal);
  lua_setfield(state, -2, "__eq");
  lua_pushcfunction(state, lua_mux_channel_flag_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_newmetatable(state, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  lua_pushcfunction(state, lua_mux_channel_flag_namespace_index);
  lua_setfield(state, -2, "__index");
  lua_pushcfunction(state, lua_mux_channel_flag_immutable);
  lua_setfield(state, -2, "__newindex");
  lua_pop(state, 1);

  luaL_getmetatable(state, LUA_MUX_CHANNEL_METATABLE);
  lua_pushlightuserdata(state, package);
  lua_pushcclosure(state, lua_mux_channel_flags, 1);
  lua_setfield(state, -2, "flags");
  lua_pop(state, 1);

  LuaMuxChannelFlagNamespace *name_space =
      lua_newuserdata(state, sizeof(*name_space));
  *name_space = (LuaMuxChannelFlagNamespace){.package = package};
  luaL_getmetatable(state, LUA_MUX_CHANNEL_FLAG_NAMESPACE_METATABLE);
  lua_setmetatable(state, -2);
  lua_setfield(state, -2, "flags");
}
