/* lua_runtime.h - Lua runtime lifecycle and MUX callback declarations. */

#pragma once

#include "mux/database/db.h"

#include <stddef.h>
#include <time.h>

int lua_initialize(char *error, size_t error_size);
void lua_shutdown(void);
int lua_reload(char *error, size_t error_size);
int lua_check(DbRef player, char *error, size_t error_size);
int lua_validate_path(const char *path, char *error, size_t error_size);
int lua_command_match(DbRef thing, DbRef player, DbRef cause,
                      const char *command);
int lua_list_command_match(DbRef first, DbRef player, DbRef cause,
                           const char *command);
int lua_global_command_match(DbRef player, DbRef cause, const char *command);
int lua_event_dispatch(DbRef player, DbRef thing, int attribute, char *args[],
                       int nargs);
void lua_schedule_tick(time_t now);
void do_luaparent(DbRef player, DbRef cause, int key, char *target, char *path);
void do_luacheck(DbRef player, DbRef cause, int key);
void do_luareload(DbRef player, DbRef cause, int key);
void do_luaschedule(DbRef player, DbRef cause, int key, char *argument);
