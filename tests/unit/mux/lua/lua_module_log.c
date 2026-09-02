/* lua_module_log.c -- Lua callback error module attribution tests. */

#include "mux/lua/lua_internal.h"
#include "mux/server/log.h"

#include <stdarg.h>
#include <stdio.h>
#include <string.h>

static LogEntry captured_entry;
static char captured_message[512];

void log_error(LogEntry entry, const char *format, ...) {
  va_list arguments;

  captured_entry = entry;
  va_start(arguments, format);
  (void)vsnprintf(captured_message, sizeof(captured_message), format,
                  arguments);
  va_end(arguments);
}

int main(void) {
  ServerLog log = {};
  LuaServices services = {.log = &log};
  LuaRuntime runtime = {.services = &services};
  const char *module = "/game/lua/object_logic/default_exit.lua";

  lua_log_error(&runtime, 6, module, "LOCK", "traverse failed");

  return captured_entry.log == &log && !strcmp(captured_entry.primary, "LUA") &&
                 !strcmp(captured_entry.secondary, "LOCK") &&
                 !strcmp(captured_message,
                         "object #6 module "
                         "/game/lua/object_logic/default_exit.lua: "
                         "traverse failed")
             ? 0
             : 1;
}
