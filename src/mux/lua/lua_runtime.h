/* lua_runtime.h - Lua runtime lifecycle and MUX callback declarations. */

#pragma once

#include "mux/database/db.h"

#include <stddef.h>
#include <time.h>

typedef struct LuaRuntime LuaRuntime;
typedef struct Descriptor Descriptor;
typedef struct MuxServer MuxServer;
typedef struct EvaluationContext EvaluationContext;

int lua_initialize(LuaRuntime **owner, MuxServer *server, char *error,
                   size_t error_size);
void lua_shutdown(LuaRuntime **owner);
int lua_reload(LuaRuntime **owner, char *error, size_t error_size);
int lua_check(EvaluationContext *evaluation, MuxServer *server, DbRef player,
              char *error, size_t error_size);
int lua_validate_path(LuaRuntime *runtime, const char *path, char *error,
                      size_t error_size);
int lua_command_match(LuaRuntime *runtime, Descriptor *descriptor, DbRef thing,
                      DbRef player, DbRef cause, const char *command);
int lua_list_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                           DbRef first, DbRef player, DbRef cause,
                           const char *command);
int lua_global_command_match(LuaRuntime *runtime, Descriptor *descriptor,
                             DbRef player, DbRef cause, const char *command);
int lua_event_dispatch(LuaRuntime *runtime, DbRef player, DbRef thing,
                       int attribute, char *args[], int nargs);
void lua_schedule_tick(LuaRuntime *runtime, time_t now);
