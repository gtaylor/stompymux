#include "lua_fixture.h"
#include "shared_contract.h"

/**
 * @par LuaLS definition mux catalog mux.comsys.flags
 * @code{.lua}
 * ---@class (exact) ChannelFlagNamespace
 * ---@field PUBLIC FixtureValue Public fixture channel.
 * ---@field LOUD FixtureValue Loud fixture channel.
 * @endcode
 */
static const ChannelFlagDefinition CHANNEL_FLAGS[] = {
    {1, "PUBLIC"},
    {2, "LOUD"},
};

/**
 * @par LuaLS definition mux catalog mux.world.access
 * @code{.lua}
 * ---@class AccessNamespace
 * ---@field PUBLIC FixtureValue Public access level.
 * @endcode
 */
static const FixtureCatalogEntry LUA_COMMAND_ACCESS_ENTRIES[] = {
    {1, "PUBLIC", "ignored"},
};

/**
 * @par LuaLS definition mux catalog mux.world.flags
 * @code{.lua}
 * ---@class FlagNamespace
 * ---@field WIZARD FixtureValue Wizard fixture flag.
 * @endcode
 */
static const POWERENT FLAG_ENTRIES[] = {
    {"WIZARD", 1, 0},
    {nullptr, 0, 0},
};

/**
 * @par LuaLS definition mux catalog mux.world.locks
 * @code{.lua}
 * ---@class LockNamespace
 * ---@field OPEN FixtureValue Open fixture lock.
 * @endcode
 */
static const FixtureCatalogEntry LUA_LOCK_DEFINITIONS[] = {
    {1, "open", "OPEN"},
};

/**
 * @par LuaLS definition mux catalog mux.world.powers
 * @code{.lua}
 * ---@class PowerNamespace
 * ---@field IDLE FixtureValue Idle fixture power.
 * @endcode
 */
static const POWERENT POWER_ENTRIES[] = {
    {"idle", 1, 0},
    {nullptr, 0, 0},
};

/**
 * @par LuaLS definition mux catalog mux.world.types
 * @code{.lua}
 * ---@class ObjectTypeNamespace
 * ---@field ROOM FixtureValue Room object type.
 * ---@field THING FixtureValue Thing object type.
 * @endcode
 */
static int lua_mux_object_type_namespace_index(lua_State *state) {
  lua_pushstring(state, "ROOM");
  lua_pushstring(state, "THING");
  return 1;
}

/**
 * @par LuaLS definition mux catalog mux.error.codes
 * @code{.lua}
 * ---@class MuxFixtureFailedErrorCode
 * ---@field code "mux.fixture.failed"
 * ---@class MuxFixtureErrorCodes
 * ---@field failed MuxFixtureFailedErrorCode Fixture failure.
 * ---@class MuxErrorCodes
 * ---@field fixture MuxFixtureErrorCodes Fixture error branch.
 * @endcode
 *
 * @par LuaLS definition mux catalog mux.testing.codes
 * @code{.lua}
 * ---@class TestingFixtureErrorCode
 * ---@field code "testing.fixture"
 * ---@class TestingErrorCodes
 * ---@field fixture TestingFixtureErrorCode Fixture testing error.
 * @endcode
 *
 * @par LuaLS definition btech catalog btech.error.codes
 * @code{.lua}
 * ---@class BtechFixtureFailedErrorCode
 * ---@field code "btech.fixture.failed"
 * ---@class BtechFixtureErrorCodes
 * ---@field failed BtechFixtureFailedErrorCode Fixture failure.
 * ---@class BtechErrorCodes
 * ---@field fixture BtechFixtureErrorCodes Fixture BattleTech error branch.
 * @endcode
 */
static const char *const LUA_ERROR_CODE_NAMES[] = {
    "mux.fixture.failed",
    "btech.fixture.failed",
    "testing.fixture",
};

/**
 * @par LuaLS definition mux callable mux.ping
 * @code{.lua}
 * ---Returns a fixture value.
 * ---@generic T
 * ---@overload fun(value: T): T
 * ---@param value? FixtureValue
 * ---@return FixtureValue value
 * function mux.ping(value) end
 * @endcode
 */
static int lua_mux_ping(lua_State *state [[maybe_unused]]) { return 1; }

/**
 * @par LuaLS definition mux callable Channel:name
 * @code{.lua}
 * ---Returns the fixture channel name.
 * ---@return string name
 * function Channel:name() end
 * @endcode
 */
static int lua_mux_channel_name(lua_State *state [[maybe_unused]]) { return 1; }

/**
 * @par LuaLS definition mux callable Flags:list
 * @code{.lua}
 * ---Lists fixture flags.
 * ---@return FixtureValue[] values
 * function Flags:list() end
 * @endcode
 */
static int lua_mux_flags_list(lua_State *state [[maybe_unused]]) { return 1; }

/**
 * @par LuaLS ignore mux __tostring -- Fixture metatable rendering has no callable declaration.
 */
static int lua_mux_tostring(lua_State *state [[maybe_unused]]) { return 1; }

/**
 * @par LuaLS definition mux callable mux.extension
 * @code{.lua}
 * ---Exercises a cross-file closure registration.
 * function mux.extension() end
 * @endcode
 */
int lua_mux_extension(lua_State *state [[maybe_unused]]) { return 0; }

static void install_list(lua_State *state, lua_CFunction list) {
  lua_pushcfunction(state, list);
  lua_setfield(state, -2, "list");
}

/**
 * @par LuaLS definition mux namespace mux
 * @code{.lua}
 * mux = {}
 * @endcode
 */
void install_mux_fixture(lua_State *state) {
  lua_pushcfunction(state, lua_mux_ping);
  lua_setfield(state, -2, "ping");
  lua_pushcfunction(state, lua_mux_tostring);
  lua_setfield(state, -2, "__tostring");

  const LuaMuxChannelMethod methods[] = {
      {"name", lua_mux_channel_name},
  };
  for (size_t index = 0; index < sizeof(methods) / sizeof(*methods); index++) {
    lua_pushcclosure(state, methods[index].function, 0);
    lua_setfield(state, -2, methods[index].name);
  }
  install_list(state, lua_mux_flags_list);
}
