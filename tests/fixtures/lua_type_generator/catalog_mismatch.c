#include "lua_fixture.h"

/**
 * @par LuaLS definition mux catalog mux.comsys.flags
 * @code{.lua}
 * ---@class ChannelFlagNamespace
 * ---@field PUBLIC FixtureValue Public fixture channel.
 * ---@field EXTRA FixtureValue Extra fixture channel.
 * @endcode
 */
static const ChannelFlagDefinition CHANNEL_FLAGS[] = {
    {1, "PUBLIC"},
    {2, "LOUD"},
};
