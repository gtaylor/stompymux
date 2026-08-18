/* lua.c - Lua runtime initialization and MUX integration. */

#include <limits.h>
#include <linux/limits.h>
#include <lua.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "mux/lua/lua_error.h"
#include "mux/lua/lua_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/world/access.h"
#include "mux/world/match.h"

typedef struct LuaScheduleIdentity {
  const char *path;
  const char *name;
  DbRef object;
  time_t minute;
} LuaScheduleIdentity;

static unsigned long lua_schedule_hash(const LuaScheduleIdentity *identity) {
  unsigned long hash = 2166136261U;
  size_t index;

  for (index = 0; index < strlen(identity->path); index++) {
    const unsigned char *character = checked_storage_at_const(
        identity->path, strlen(identity->path), sizeof(*character), index);

    hash = (hash ^ *character) * 16777619U;
  }
  for (index = 0; index < strlen(identity->name); index++) {
    const unsigned char *character = checked_storage_at_const(
        identity->name, strlen(identity->name), sizeof(*character), index);

    hash = (hash ^ *character) * 16777619U;
  }
  hash ^= (unsigned long)identity->object;
  hash ^= (unsigned long)identity->minute;
  return hash;
}

static bool lua_schedule_add_job(LuaRuntime *runtime, LuaModuleRoot root,
                                 const char *path, const char *name,
                                 const char *cron, DbRef object,
                                 time_t minute) {
  LuaScheduleJob *jobs;
  LuaScheduleJob *job;
  if (runtime->schedule_job_count == SIZE_MAX)
    return false;

  char *path_copy = strdup(path);
  char *name_copy = strdup(name);
  char *cron_copy = strdup(cron);

  if (!path_copy || !name_copy || !cron_copy) {
    free(path_copy);
    free(name_copy);
    free(cron_copy);
    return false;
  }

  jobs = checked_storage_try_reallocate_array(
      runtime->schedule_jobs, runtime->schedule_job_count + 1, sizeof(*jobs));
  if (!jobs) {
    free(path_copy);
    free(name_copy);
    free(cron_copy);
    return false;
  }
  runtime->schedule_jobs = jobs;
  runtime->schedule_job_count++;
  job = lua_schedule_job_at(runtime, runtime->schedule_job_count - 1);
  memset(job, 0, sizeof(*job));
  job->root = root;
  job->object = object;
  job->path = path_copy;
  job->name = name_copy;
  job->cron = cron_copy;
  job->due =
      (minute * 60) +
      (time_t)(lua_schedule_hash(&(LuaScheduleIdentity){.path = path,
                                                        .name = name,
                                                        .object = object,
                                                        .minute = minute}) %
               55U);
  job->expires = (minute * 60) + 60;
  return true;
}

static void lua_schedule_collect_module(LuaRuntime *runtime, LuaModuleRoot root,
                                        const char *path, DbRef object,
                                        time_t minute) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (!lua_load_module(runtime, root, path, error, sizeof(error))) {
    lua_log_load_error(runtime, object, path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  if (!lua_istable(state, schedules)) {
    lua_settop(state, top);
    return;
  }
  for (index = 1; index <= (int)lua_objlen(state, schedules); index++) {
    const char *name;
    const char *cron;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "cron");
    cron = lua_tostring(state, -1);
    lua_pop(state, 1);
    if (name && cron &&
        lua_cron_matches(cron, minute * 60, error, sizeof(error)) > 0)
      lua_schedule_add_job(runtime, root, path, name, cron, object, minute);
    lua_pop(state, 1);
  }
  lua_settop(state, top);
}

static void lua_schedule_run_job(LuaRuntime *runtime, LuaScheduleJob *job) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (job->root == LUA_ROOT_OBJECT_LOGIC &&
      (!is_good_obj(runtime->services->database, job->object) ||
       is_going(runtime->services->database, job->object)))
    return;
  if (!lua_load_module(runtime, job->root, job->path, error, sizeof(error))) {
    lua_log_load_error(runtime, job->object, job->path, error);
    lua_settop(state, top);
    return;
  }
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  for (index = 1; lua_istable(state, schedules) &&
                  index <= (int)lua_objlen(state, schedules);
       index++) {
    const char *name;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    if (!name || strcmp(name, job->name) != 0) {
      lua_pop(state, 1);
      continue;
    }
    lua_getfield(state, -1, "handler");
    if (lua_isfunction(state, -1)) {
      LuaModuleRoot previous_root;
      int status;

      lua_push_context(runtime->services->database, nullptr, state, job->object,
                       GOD, GOD, nullptr, "schedule",
                       job->root == LUA_ROOT_OBJECT_LOGIC ? "object" : "global",
                       nullptr, 0);
      lua_pushstring(state, job->name);
      lua_setfield(state, -2, "schedule");
      lua_pushstring(state, job->cron);
      lua_setfield(state, -2, "cron");
      if (job->root == LUA_ROOT_GLOBAL_LOGIC) {
        lua_pushnil(state);
        lua_setfield(state, -2, "enactor");
        lua_pushnil(state);
        lua_setfield(state, -2, "cause");
      }
      previous_root = runtime->current_root;
      runtime->current_root = job->root;
      status = lua_callback_pcall_checked(runtime, 1, 0);
      runtime->current_root = previous_root;
      if (status) {
        char *description = alloc_lbuf("lua_schedule_tick.error");

        lua_error_describe(state, -1, description, LBUF_SIZE);
        if (job->root == LUA_ROOT_OBJECT_LOGIC) {
          log_error((LogEntry){.log = runtime->services->log,
                               .key = LOG_PROBLEMS,
                               .primary = "LUA",
                               .secondary = "SCHEDULE"},
                    "object #%ld module %s schedule %s: %s", job->object,
                    job->path, job->name, description);
        } else {
          log_error((LogEntry){.log = runtime->services->log,
                               .key = LOG_PROBLEMS,
                               .primary = "LUA",
                               .secondary = "SCHEDULE"},
                    "global module %s schedule %s: %s", job->path, job->name,
                    description);
        }
        free_buf(description);
      }
    } else {
      lua_pop(state, 1);
    }
    lua_pop(state, 1);
    break;
  }
  lua_settop(state, top);
}

void lua_schedule_tick(LuaRuntime *runtime, time_t now) {
  time_t minute = now / 60;
  size_t index;

  if (!runtime)
    return;
  if (runtime->schedule_high_water < 0)
    runtime->schedule_high_water = minute;
  if (minute > runtime->schedule_high_water) {
    DbRef object;

    runtime->schedule_high_water = minute;
    for (index = 0; index < runtime->global_module_count; index++)
      lua_schedule_collect_module(runtime, LUA_ROOT_GLOBAL_LOGIC,
                                  lua_global_module_at(runtime, index), NOTHING,
                                  minute);
    for (object = 0; object < runtime->services->database->top; object++) {
      char path[PATH_MAX];

      if (!is_good_obj(runtime->services->database, object) ||
          is_going(runtime->services->database, object) ||
          !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
        continue;
      lua_schedule_collect_module(runtime, LUA_ROOT_OBJECT_LOGIC, path, object,
                                  minute);
    }
  }
  for (index = 0; index < runtime->schedule_job_count;) {
    LuaScheduleJob *job = lua_schedule_job_at(runtime, index);

    if (now >= job->expires) {
      free(job->path);
      free(job->name);
      free(job->cron);
      if (index + 1 < runtime->schedule_job_count)
        *job = *lua_schedule_job_at(runtime, runtime->schedule_job_count - 1);
      runtime->schedule_job_count--;
      continue;
    }
    if (now >= job->due) {
      lua_schedule_run_job(runtime, job);
      free(job->path);
      free(job->name);
      free(job->cron);
      if (index + 1 < runtime->schedule_job_count)
        *job = *lua_schedule_job_at(runtime, runtime->schedule_job_count - 1);
      runtime->schedule_job_count--;
      continue;
    }
    index++;
  }
}

static bool lua_schedule_count(LuaRuntime *runtime, LuaModuleRoot root,
                               const char *path, int *count, char *error,
                               size_t error_size) {
  lua_State *state = runtime->state;
  int top = lua_gettop(state);

  if (!lua_load_module(runtime, root, path, error, error_size)) {
    lua_settop(state, top);
    return false;
  }
  lua_getfield(state, -1, "schedules");
  *count = lua_istable(state, -1) ? (int)lua_objlen(state, -1) : 0;
  lua_settop(state, top);
  return true;
}

static void lua_schedule_show_module(EvaluationContext *evaluation,
                                     DbRef player, LuaRuntime *runtime,
                                     LuaModuleRoot root, const char *path,
                                     int show_objects) {
  lua_State *state;
  int top;
  int schedules;
  int index;
  char error[LBUF_SIZE];

  if (!runtime || !lua_load_module(runtime, root, path, error, sizeof(error))) {
    notify_printf(evaluation, player, "Lua schedule unavailable: %s", error);
    return;
  }
  state = runtime->state;
  top = lua_gettop(state) - 1;
  notify_printf(evaluation, player, "Schedules for %s/%s:", lua_root_name(root),
                path);
  lua_getfield(state, -1, "schedules");
  schedules = lua_gettop(state);
  if (!lua_istable(state, schedules))
    notify_checked(evaluation, player, player, "  (none)", MSG_ME);
  for (index = 1; lua_istable(state, schedules) &&
                  index <= (int)lua_objlen(state, schedules);
       index++) {
    const char *name;
    const char *cron;

    lua_rawgeti(state, schedules, index);
    lua_getfield(state, -1, "name");
    name = lua_tostring(state, -1);
    lua_pop(state, 1);
    lua_getfield(state, -1, "cron");
    cron = lua_tostring(state, -1);
    lua_pop(state, 2);
    notify_printf(evaluation, player, "  %s: %s", name ? name : "<invalid>",
                  cron ? cron : "<invalid>");
  }
  lua_settop(state, top);
  if (show_objects) {
    DbRef object;

    notify_checked(evaluation, player, player, "Objects:", MSG_ME);
    for (object = 0; object < runtime->services->database->top; object++) {
      char attached[PATH_MAX];

      if (is_good_obj(runtime->services->database, object) &&
          lua_attached_path(runtime, object, attached, sizeof(attached),
                            nullptr) &&
          !strcmp(attached, path))
        notify_printf(evaluation, player, "  %s (#%ld)",
                      game_object_name(runtime->services->database, object),
                      object);
    }
  }
}

void do_luaschedule(CommandInvocation *invocation) {
  DbRef player = invocation->player;
  char *argument = invocation->first;
  LuaRuntime *runtime = invocation->context->runtime->lua_owner->runtime;
  LuaRuntime *inspection;
  char error[LBUF_SIZE];

  if (!runtime) {
    notify_checked(&invocation->context->evaluation, player, player,
                   "Lua is not initialized.", MSG_ME);
    return;
  }
  inspection =
      lua_runtime_create(nullptr, runtime->services, error, sizeof(error));
  if (!inspection) {
    notify_printf(&invocation->context->evaluation, player,
                  "Lua schedule unavailable: %s", error);
    return;
  }
  inspection->checking = 1;
  if (*argument) {
    if (!strncmp(argument, "global_logic/", 13)) {
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_GLOBAL_LOGIC,
                               checked_string_suffix(argument, 13), 0);
      goto done;
    }
    if (lua_valid_relative_path(argument)) {
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_OBJECT_LOGIC, argument, 1);
      goto done;
    }
    init_match(&invocation->context->match, player, argument,
               OBJECT_TYPE_NOTYPE);
    match_everything(&invocation->context->match, 0);
    {
      DbRef object = noisy_match_result(&invocation->context->match);
      char path[PATH_MAX];

      if (object == NOTHING)
        goto done;
      if (!lua_attached_path(runtime, object, path, sizeof(path), nullptr)) {
        notify_checked(&invocation->context->evaluation, player, player,
                       "That object has no Luaparent.", MSG_ME);
        goto done;
      }
      lua_schedule_show_module(&invocation->context->evaluation, player,
                               inspection, LUA_ROOT_OBJECT_LOGIC, path, 0);
      goto done;
    }
  }
  {
    size_t index;
    DbRef object;
    char **paths = nullptr;
    size_t *counts = nullptr;
    size_t path_count = 0;

    for (index = 0; index < runtime->global_module_count; index++) {
      int count;

      const char *module = lua_global_module_at(runtime, index);

      if (lua_schedule_count(inspection, LUA_ROOT_GLOBAL_LOGIC, module, &count,
                             error, sizeof(error)) &&
          count)
        notify_printf(&invocation->context->evaluation, player,
                      "global_logic/%s: %d schedules (global)", module, count);
    }
    for (object = 0; object < runtime->services->database->top; object++) {
      char path[PATH_MAX];

      if (!is_good_obj(runtime->services->database, object) ||
          !lua_attached_path(runtime, object, path, sizeof(path), nullptr))
        continue;
      for (index = 0; index < path_count; index++) {
        char **stored_path = (char **)checked_storage_at(
            (void *)paths, path_count, sizeof(*paths), index);
        size_t *stored_count =
            checked_storage_at(counts, path_count, sizeof(*counts), index);

        if (!strcmp(*stored_path, path)) {
          (*stored_count)++;
          break;
        }
      }
      if (index == path_count) {
        if (path_count == SIZE_MAX)
          break;
        char **new_paths = (char **)checked_storage_try_reallocate_array(
            (void *)paths, path_count + 1, sizeof(*paths));
        size_t *new_counts;

        if (!new_paths)
          break;
        paths = new_paths;
        new_counts = checked_storage_try_reallocate_array(
            counts, path_count + 1, sizeof(*counts));
        if (!new_counts)
          break;
        counts = new_counts;
        *(char **)checked_storage_at((void *)paths, path_count + 1,
                                     sizeof(*paths), path_count) = strdup(path);
        *(size_t *)checked_storage_at(counts, path_count + 1, sizeof(*counts),
                                      path_count) = 1;
        path_count++;
      }
    }
    for (index = 0; index < path_count; index++) {
      int count;

      char **stored_path = (char **)checked_storage_at(
          (void *)paths, path_count, sizeof(*paths), index);
      size_t *stored_count =
          checked_storage_at(counts, path_count, sizeof(*counts), index);

      if (lua_schedule_count(inspection, LUA_ROOT_OBJECT_LOGIC, *stored_path,
                             &count, error, sizeof(error)) &&
          count)
        notify_printf(&invocation->context->evaluation, player,
                      "object_logic/%s: %d schedules (%zu objects)",
                      *stored_path, count, *stored_count);
      free(*stored_path);
    }
    free((void *)paths);
    free(counts);
  }
done:
  lua_runtime_destroy(inspection);
}
