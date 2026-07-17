/* libuv include shim for names that collide with the MUX API. */

#pragma once

#include <uv.h>

#ifdef CLONE_PARENT
#undef CLONE_PARENT
#endif
