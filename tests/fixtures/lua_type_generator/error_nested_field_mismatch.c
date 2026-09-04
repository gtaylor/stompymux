#include "lua_fixture.h"

/**
 * @par LuaLS definition mux catalog mux.error.codes
 * @code{.lua}
 * ---@class MuxStateInvalidErrorCode
 * ---@field code "mux.state.invalid"
 * ---@class MuxStateErrorCodes
 * ---@field wrong MuxStateInvalidErrorCode Wrong nested field.
 * ---@class MuxErrorCodes
 * ---@field state MuxStateErrorCodes State errors.
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
    "mux.state.invalid",
};
