#pragma once

#define REGISTER_FIXTURE(state, handler, name)                                 \
  do {                                                                         \
    lua_pushcclosure((state), (handler), 0);                                   \
    lua_setfield((state), -2, (name));                                         \
  } while (0)
