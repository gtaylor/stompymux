#include "lua_fixture.h"

/**
 * @par LuaLS definition mux catalog mux.error.codes
 * @code{.lua}
 * ---@class MuxFixtureErrorCode
 * ---@field code "mux.fixture.failed"
 * ---@class MuxErrorCodes
 * ---@field wrong MuxFixtureErrorCode Wrong root field.
 * @endcode
 *
 * @par LuaLS definition btech catalog btech.error.codes
 * @code{.lua}
 * ---@class BtechErrorCodes
 * @endcode
 *
 * @par LuaLS definition mux catalog mux.testing.codes
 * @code{.lua}
 * ---@class TestingErrorCodes
 * @endcode
 */
static const char *const LUA_ERROR_CODE_NAMES[] = {
    "mux.fixture.failed",
};
