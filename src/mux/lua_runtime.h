#pragma once

#include "db.h"

#include <stddef.h>
#include <time.h>

int lua_initialize(char *error, size_t error_size);
void lua_shutdown(void);
int lua_reload(char *error, size_t error_size);
int lua_check(dbref player, char *error, size_t error_size);
int lua_validate_path(const char *path, char *error, size_t error_size);
int lua_command_match(dbref thing, dbref player, dbref cause,
                      const char *command);
int lua_list_command_match(dbref first, dbref player, dbref cause,
                           const char *command);
int lua_global_command_match(dbref player, dbref cause, const char *command);
int lua_event_dispatch(dbref player, dbref thing, int attribute, char *args[],
                       int nargs);
void lua_schedule_tick(time_t now);
void do_luaparent(dbref player, dbref cause, int key, char *target, char *path);
void do_luacheck(dbref player, dbref cause, int key);
void do_luareload(dbref player, dbref cause, int key);
void do_luaschedule(dbref player, dbref cause, int key, char *argument);
