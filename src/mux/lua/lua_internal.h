/* Private Lua runtime state and cross-module helpers. */

#pragma once

#include <limits.h>
#include <stddef.h>
#include <time.h>

#include <lua.h>

#include "mux/lua/btech_package.h"
#include "mux/lua/lua_runtime.h"
#include "mux/lua/mux_package.h"

typedef struct CommandInvocation CommandInvocation;

typedef enum LuaModuleRoot {
  LUA_ROOT_OBJECT_LOGIC,
  LUA_ROOT_GLOBAL_LOGIC,
  LUA_ROOT_PACKAGES,
  LUA_ROOT_COUNT,
} LUA_MODULE_ROOT;

typedef struct LuaScheduleJob LUA_SCHEDULE_JOB;
struct LuaScheduleJob {
  LUA_MODULE_ROOT root;
  DbRef object;
  time_t due;
  time_t expires;
  char *path;
  char *name;
  char *cron;
};

struct LuaRuntime {
  LuaOwner *owner;
  const LuaServices *services;
  lua_State *state;
  char root[PATH_MAX];
  char roots[LUA_ROOT_COUNT][PATH_MAX];
  char module[PATH_MAX];
  LUA_MODULE_ROOT current_root;
  int checking;
  char **global_modules;
  size_t global_module_count;
  LUA_SCHEDULE_JOB *schedule_jobs;
  size_t schedule_job_count;
  time_t schedule_high_water;
  LuaMuxPackage mux_package;
  LuaBtechPackage btech_package;
};

const char *lua_global_module_at(const LuaRuntime *runtime, size_t index);
char *lua_global_module_slot(LuaRuntime *runtime, size_t index);
const char *lua_runtime_root_at(const LuaRuntime *runtime,
                                LUA_MODULE_ROOT root);
char *lua_runtime_root_slot(LuaRuntime *runtime, LUA_MODULE_ROOT root);
LUA_SCHEDULE_JOB *lua_schedule_job_at(LuaRuntime *runtime, size_t index);

extern const char LUA_MODULES_KEY[];
extern const char *const LUA_EVENT_NAMES[LUA_EVENT_COUNT];
extern const char *const LUA_LOCK_NAMES[LUA_LOCK_COUNT];
extern const char *const LUA_LOCK_OPERATION_NAMES[LUA_LOCK_OPERATION_COUNT];
extern const char *const LUA_MESSAGE_NAMES[LUA_MESSAGE_COUNT];
extern const char *const
    LUA_MESSAGE_OPERATION_NAMES[LUA_MESSAGE_OPERATION_COUNT];

void lua_set_error(char *error, size_t error_size, const char *format, ...)
    __attribute__((format(printf, 3, 4)));
int lua_pcall_checked(LuaRuntime *runtime, int arguments, int results);
int lua_callback_pcall_checked(LuaRuntime *runtime, int arguments, int results);
void lua_log_error(LuaRuntime *runtime, DbRef object, const char *kind,
                   const char *error);
void lua_log_load_error(LuaRuntime *runtime, DbRef object, const char *path,
                        const char *error);
int lua_valid_relative_path(const char *path);
const char *lua_root_name(LUA_MODULE_ROOT root);
int lua_join_path(char *destination, size_t destination_size, const char *first,
                  const char *second);
int lua_resolve_path(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                     const char *path, char *resolved, size_t resolved_size,
                     char *error, size_t error_size);
int lua_load_module(LuaRuntime *runtime, LUA_MODULE_ROOT root, const char *path,
                    char *error, size_t error_size);
LUA_MODULE_ROOT lua_require_root(lua_State *state, LuaRuntime *runtime);
LuaRuntime *lua_runtime_create(LuaOwner *owner, const LuaServices *services,
                               char *error, size_t error_size);
void lua_runtime_destroy(LuaRuntime *runtime);

void lua_free_modules(char **modules, size_t module_count);
int lua_compare_module_paths(const void *left, const void *right);
int lua_collect_modules(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                        const char *relative, char ***modules,
                        size_t *module_count, char *error, size_t error_size);
int lua_cron_matches(const char *cron, time_t when, char *error,
                     size_t error_size);
int lua_check_one_module(LuaRuntime *runtime, LUA_MODULE_ROOT root,
                         const char *path, char *error, size_t error_size);

bool lua_event_name_is_known(const char *name);
bool lua_lock_name_is_known(const char *name);
bool lua_message_name_is_known(const char *name);
void lua_push_context(GameDatabase *database, Descriptor *descriptor,
                      lua_State *state, DbRef object, DbRef player, DbRef cause,
                      const char *command, const char *event, const char *scope,
                      char *arguments[], int argument_count);
int lua_attached_path(LuaRuntime *runtime, DbRef object, char *path,
                      size_t path_size, DbRef *source);
void do_luaschedule(CommandInvocation *invocation);

int lua_runtime_is_checking(void *context);
int lua_runtime_flow_start(void *context, lua_State *state, int descriptor_id,
                           const char *module, const char *first_step);
int lua_runtime_exit_enter_lock_passes(void *context, DbRef exit,
                                       DbRef enactor);
