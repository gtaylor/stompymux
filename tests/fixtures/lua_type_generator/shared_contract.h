#pragma once

#include "lua_fixture.h"

/**
 * @par LuaLS definition mux alias fixture.value
 * @code{.lua}
 * ---@alias FixtureValue string|number A deliberately long contract line that clang-format must preserve byte-for-byte past the normal column limit.
 * @endcode
 */
typedef const char *LuaFixtureValue;

int lua_mux_extension(lua_State *state);
