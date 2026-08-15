#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * command.c - command parser and support routines
 */

#include <bits/types/struct_rusage.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

#include "mux/commands/command.h"
#include "mux/commands/command_catalog.h"
#include "mux/commands/command_context.h"
#include "mux/commands/command_handlers.h"
#include "mux/commands/command_internal.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/network_output.h"
#include "mux/network/site_access.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/objects/powers.h"
#include "mux/server/configuration.h"
#include "mux/server/configuration_registry.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/maintenance.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/name_table.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

#include "mux/server/log_cache.h"

constexpr char CACHING[] = "object";

typedef struct LuaCommandListContext {
  EvaluationContext *evaluation;
  GameDatabase *database;
  DbRef player;
} LuaCommandListContext;

static void list_global_lua_command(void *data, const char *source,
                                    DbRef object, const char *pattern) {
  LuaCommandListContext *context = data;

  notify_printf(context->evaluation, context->player, "  %s: %s", source,
                pattern);
}

static void list_object_lua_command(void *data, const char *source,
                                    DbRef object, const char *pattern) {
  LuaCommandListContext *context = data;

  notify_printf(context->evaluation, context->player, "  %s (#%ld): %s",
                game_object_name(context->database, object), object, pattern);
}

static size_t list_object_lua_command_source(LuaRuntime *lua,
                                             LuaCommandListContext *context,
                                             DbRef object, bool visited[],
                                             size_t visited_count) {
  if (!is_good_obj(context->database, object))
    return 0;
  bool *was_visited = checked_storage_at(visited, visited_count,
                                         sizeof(*visited), (size_t)object);

  if (*was_visited)
    return 0;
  *was_visited = true;
  return lua_visit_object_commands(lua, object, context->player,
                                   list_object_lua_command, context);
}

static size_t list_object_lua_command_sources(LuaRuntime *lua,
                                              LuaCommandListContext *context,
                                              DbRef first, bool visited[],
                                              size_t visited_count) {
  DbRef object;
  size_t count = 0;

  DOLIST(context->database, object, first) {
    count += list_object_lua_command_source(lua, context, object, visited,
                                            visited_count);
  }
  return count;
}

static size_t list_reachable_object_lua_commands(CommandRuntime *runtime,
                                                 EvaluationContext *evaluation,
                                                 DbRef player) {
  GameDatabase *database = runtime->world->database;
  LuaRuntime *lua = runtime->lua_owner->runtime;
  LuaCommandListContext context = {
      .evaluation = evaluation,
      .database = database,
      .player = player,
  };
  bool *visited = checked_storage_try_allocate_array((size_t)database->top,
                                                     sizeof(*visited));
  size_t count = 0;

  if (!visited)
    return 0;
  if (!is_no_command(database, player))
    count += list_object_lua_command_source(lua, &context, player, visited,
                                            (size_t)database->top);
  if (has_location(database, player)) {
    DbRef location = game_object_location(database, player);

    count += list_object_lua_command_sources(
        lua, &context, game_object_contents(database, location), visited,
        (size_t)database->top);
    if (!is_no_command(database, location))
      count += list_object_lua_command_source(lua, &context, location, visited,
                                              (size_t)database->top);
  }
  if (has_contents(database, player))
    count += list_object_lua_command_sources(
        lua, &context, game_object_contents(database, player), visited,
        (size_t)database->top);

  if (has_location(database, player)) {
    DbRef location = game_object_location(database, player);
    DbRef location_zone = game_object_zone(database, location);
    DbRef player_zone = game_object_zone(database, player);

    if (location_zone != NOTHING) {
      if (typeof_obj(database, location_zone) == OBJECT_TYPE_ROOM) {
        if (location != player_zone)
          count += list_object_lua_command_sources(
              lua, &context, game_object_contents(database, location_zone),
              visited, (size_t)database->top);
      } else if (!is_no_command(database, location_zone)) {
        count += list_object_lua_command_source(lua, &context, location_zone,
                                                visited, (size_t)database->top);
      }
    }
    if (player_zone != NOTHING && !is_no_command(database, player_zone) &&
        location_zone != player_zone)
      count += list_object_lua_command_source(lua, &context, player_zone,
                                              visited, (size_t)database->top);
  }
  free(visited);
  return count;
}

static void list_cmdtable(EvaluationContext *evaluation,
                          CommandRuntime *runtime, DbRef player) {
  const ServerConfiguration *configuration = runtime->world->configuration;
  LuaCommandListContext context = {
      .evaluation = evaluation,
      .database = runtime->world->database,
      .player = player,
  };
  char *buf;
  size_t used = 0;
  size_t count;

  buf = alloc_lbuf("list_cmdtable");
  const char PREFIX[] = "Built-in commands:";

  memcpy(buf, PREFIX, sizeof(PREFIX) - 1);
  used = sizeof(PREFIX) - 1;
  for (size_t index = 0;
       index < command_registry_builtin_count(runtime->command_registry);
       index++) {
    const CMDENT *cmdp =
        command_registry_builtin_at_const(runtime->command_registry, index);

    if (check_access(evaluation->world->database, configuration, player,
                     cmdp->perms)) {
      if (!(cmdp->perms & CF_DARK)) {
        const size_t NAME_LENGTH = strlen(cmdp->cmdname);

        if (NAME_LENGTH > LBUF_SIZE - used - 2)
          abort();
        *(char *)checked_storage_at(buf, LBUF_SIZE, sizeof(char), used++) = ' ';
        memcpy(checked_storage_region(buf, LBUF_SIZE, used, NAME_LENGTH),
               cmdp->cmdname, NAME_LENGTH);
        used += NAME_LENGTH;
      }
    }
  }
  *(char *)checked_storage_at(buf, LBUF_SIZE, sizeof(char), used) = '\0';

  notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_buf(buf);

  notify_checked(evaluation, player, player, "Global commands:", MSG_ME);
  count = lua_visit_global_commands(runtime->lua_owner->runtime, player,
                                    list_global_lua_command, &context);
  if (!count)
    notify_checked(evaluation, player, player, "  (none)", MSG_ME);

  notify_checked(evaluation, player, player, "Object commands:", MSG_ME);
  count = list_reachable_object_lua_commands(runtime, evaluation, player);
  if (!count)
    notify_checked(evaluation, player, player, "  (none)", MSG_ME);
}

/*
 * ---------------------------------------------------------------------------
 * * command_list_access: List access commands.
 */

static void list_df_flags(EvaluationContext *evaluation,
                          const ServerConfiguration *configuration,
                          DbRef player) {
  char *playerb;
  char *roomb;
  char *thingb;
  char *exitb;
  char *buff;

  playerb = decode_flags(
      &(DecodeFlagsRequest){.database = evaluation->world->database,
                            .player = player,
                            .object_type = OBJECT_TYPE_PLAYER,
                            .flags = &configuration->default_player_flags});
  roomb = decode_flags(
      &(DecodeFlagsRequest){.database = evaluation->world->database,
                            .player = player,
                            .object_type = OBJECT_TYPE_ROOM,
                            .flags = &configuration->default_room_flags});
  exitb = decode_flags(
      &(DecodeFlagsRequest){.database = evaluation->world->database,
                            .player = player,
                            .object_type = OBJECT_TYPE_EXIT,
                            .flags = &configuration->default_exit_flags});
  thingb = decode_flags(
      &(DecodeFlagsRequest){.database = evaluation->world->database,
                            .player = player,
                            .object_type = OBJECT_TYPE_THING,
                            .flags = &configuration->default_thing_flags});
  buff = alloc_lbuf("list_df_flags");
  (void)snprintf(
      buff, LBUF_SIZE,
      "Default flags: Players...%s Rooms...%s Exits...%s Things...%s", playerb,
      roomb, exitb, thingb);
  raw_notify(evaluation, player, buff);
  free_buf(buff);
  free_buf(playerb);
  free_buf(roomb);
  free_buf(exitb);
  free_buf(thingb);
}

/*
 * ---------------------------------------------------------------------------
 * * list_options: List more game options from the context configuration.
 */

static void list_options(EvaluationContext *evaluation, CommandRuntime *runtime,
                         DbRef player) {
  const ServerConfiguration *configuration = runtime->world->configuration;
  char buff[MBUF_SIZE];
  time_t now;

  now = time(nullptr);
  if (configuration->name_spaces)
    raw_notify(evaluation, player, "Player names may contain spaces.");
  else
    raw_notify(evaluation, player, "Player names may not contain spaces.");
  raw_notify(evaluation, player,
             "Player names and aliases use printable ASCII; other text uses "
             "UTF-8.");
  raw_notify(evaluation, player,
             "The '@examine' command lists the flag names for the object's "
             "flags.");
  notify_printf(
      evaluation, player,
      "Players may have at most %d commands in the queue at one time.",
      configuration->command_queue_limit);
  if (!is_wizard(evaluation->world->database, player))
    return;

  notify_printf(
      evaluation, player,
      "%d commands are run from the queue when there is no net activity.",
      configuration->command_queue_idle_chunk);
  notify_printf(
      evaluation, player,
      "%d commands are run from the queue when there is net activity.",
      configuration->command_queue_active_chunk);
  notify_printf(evaluation, player,
                "The %s cache is %d entries wide by %d entries deep.", CACHING,
                configuration->cache_width, configuration->cache_depth);
  if (configuration->cache_trim)
    raw_notify(
        evaluation, player,
        "The cache depth is periodically trimmed back to its initial value.");
  if (configuration->database.fork_dump) {
    raw_notify(evaluation, player,
               "Database dumps are performed by a fork()ed process.");
  }
  if (configuration->max_players >= 0)
    notify_printf(evaluation, player,
                  "There may be at most %d players logged in at once.",
                  configuration->max_players);
  notify_printf(evaluation, player, "The head of the object freelist is #%ld.",
                runtime->world->database->freelist);

  (void)snprintf(buff, MBUF_SIZE,
                 "Intervals: Dump...%d  Clean...%d  Idlecheck...%d",
                 configuration->database.dump_interval,
                 configuration->check_interval, configuration->idle_interval);
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE,
                 "Timers: Dump...%ld  Clean...%ld  Idlecheck...%ld",
                 (long)runtime->clock->dump_deadline - now,
                 (long)runtime->clock->check_deadline - now,
                 (long)runtime->clock->idle_deadline - now);
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE,
                 "Timeouts: Idle...%d  Connect...%d  Tries...%d",
                 configuration->idle_timeout, configuration->conn_timeout,
                 configuration->retry_limit);
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE,
                 "Scheduling: Timeslice...%d  Max_Quota...%d  Increment...%d",
                 configuration->command_quota_interval,
                 configuration->command_quota_max,
                 configuration->command_quota_increment);
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE, "Spaces...%s",
                 configuration->space_compress ? "Enabled" : "Disabled");
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE,
                 "New characters: Room...#%d  Home...#%d  DefaultHome...#%d",
                 configuration->start_room, configuration->start_home,
                 configuration->default_home);
  raw_notify(evaluation, player, buff);

  (void)snprintf(buff, MBUF_SIZE, "Queue: IdleChunk...%d  ActiveChunk...%d",
                 configuration->command_queue_idle_chunk,
                 configuration->command_queue_active_chunk);
  raw_notify(evaluation, player, buff);
}

/*
 * ---------------------------------------------------------------------------
 * * list_process: List local resource usage stats of the mush process.
 * * Adapted from code by Claudius,
 * *     posted to the net by Howard/Dark_Lord.
 */

static void list_process(EvaluationContext *evaluation,
                         const RuntimeClock *clock, DbRef player) {
  int pid;
  int psize;
  int maxfds;

  struct rusage usage;
  int curr;
  int last;
  int dur;

  getrusage(RUSAGE_SELF, &usage);
  /*
   * Calculate memory use from the aggregate totals
   */

  curr = clock->current_sample;
  last = 1 - curr;
  dur = *(const int *)checked_storage_at_const(
            clock->sample_time,
            sizeof(clock->sample_time) / sizeof(*clock->sample_time),
            sizeof(*clock->sample_time), (size_t)curr) -
        *(const int *)checked_storage_at_const(
            clock->sample_time,
            sizeof(clock->sample_time) / sizeof(*clock->sample_time),
            sizeof(*clock->sample_time), (size_t)last);
  if (dur > 0) {
  }
  maxfds = getdtablesize();

  pid = getpid();
  psize = getpagesize();

  /*
   * Go display everything
   */

  notify_printf(evaluation, player,
                "Process ID:  %10d        %10d bytes per page", pid, psize);
  notify_printf(evaluation, player, "Time used:   %10ld user   %10ld sys",
                usage.ru_utime.tv_sec, usage.ru_stime.tv_sec);

  notify_printf(evaluation, player,
                "Integral mem:%10ld shared %10ld private%10ld stack",
                usage.ru_ixrss, usage.ru_idrss, usage.ru_isrss);
  notify_printf(evaluation, player, "Max res mem: %10ld pages  %10ld bytes",
                usage.ru_maxrss, (usage.ru_maxrss * psize));
  notify_printf(evaluation, player,
                "Page faults: %10ld hard   %10ld soft   %10ld swapouts",
                usage.ru_majflt, usage.ru_minflt, usage.ru_nswap);
  notify_printf(evaluation, player, "Disk I/O:    %10ld reads  %10ld writes",
                usage.ru_inblock, usage.ru_oublock);
  notify_printf(evaluation, player, "Network I/O: %10ld in     %10ld out",
                usage.ru_msgrcv, usage.ru_msgsnd);
  notify_printf(evaluation, player,
                "Context swi: %10ld vol    %10ld forced %10ld sigs",
                usage.ru_nvcsw, usage.ru_nivcsw, usage.ru_nsignals);
  notify_printf(evaluation, player, "Descs avail: %10d", maxfds);
}

/*
 * ---------------------------------------------------------------------------
 * * do_list: List information stored in internal structures.
 */

void do_list(CommandInvocation *invocation) {
  CommandRuntime *runtime = invocation->context->runtime;
  ServerConfiguration *configuration = runtime->world->configuration;
  const DbRef PLAYER = invocation->player;
  char *arg = invocation->first;
  int flagvalue;

  const ConfigurationRegistry *configuration_registry =
      runtime->configuration_context->configuration_registry;
  const NameTable *list_option_table =
      configuration_registry_list_options_const(configuration_registry);
  flagvalue = name_table_search(runtime->world->database, configuration, PLAYER,
                                list_option_table, arg);
  switch (flagvalue) {
  case LIST_COMMANDS:
    list_cmdtable(&invocation->context->evaluation, runtime, PLAYER);
    break;
  case LIST_SWITCHES:
    command_list_switches(&invocation->context->evaluation, configuration,
                          runtime->command_registry, PLAYER);
    break;
  case LIST_OPTIONS:
    list_options(&invocation->context->evaluation, runtime, PLAYER);
    break;
  case LIST_SITEINFO:
    list_siteinfo(&invocation->context->evaluation,
                  invocation->context->world->access_control, PLAYER);
    break;
  case LIST_FLAGS:
    display_flagtab(&invocation->context->evaluation, PLAYER);
    break;
  case LIST_GLOBALS:
    list_global_controls(&invocation->context->evaluation, configuration,
                         PLAYER);
    break;
  case LIST_DF_FLAGS:
    list_df_flags(&invocation->context->evaluation, configuration, PLAYER);
    break;
  case LIST_PERMS:
    command_list_access(&invocation->context->evaluation, configuration,
                        runtime->command_registry, PLAYER);
    break;
  case LIST_CONF_PERMS:
    configuration_list_access(&invocation->context->evaluation,
                              configuration_registry, PLAYER);
    break;
  case LIST_POWERS:
    display_powertab(&invocation->context->evaluation, PLAYER);
    break;
  case LIST_LOGGING:
    name_table_interpret(&(NameTableInterpretRequest){
        .evaluation = &invocation->context->evaluation,
        .configuration = configuration,
        .player = PLAYER,
        .table = LOGOPTIONS_NAMETAB,
        .flags = configuration->log_options,
        .prefix = "Events Logged:",
        .true_text = "enabled",
        .false_text = "disabled",
    });
    name_table_interpret(&(NameTableInterpretRequest){
        .evaluation = &invocation->context->evaluation,
        .configuration = configuration,
        .player = PLAYER,
        .table = LOGDATA_NAMETAB,
        .flags = configuration->log_info,
        .prefix = "Information Logged:",
        .true_text = "yes",
        .false_text = "no",
    });
    break;
  case LIST_PROCESS:
    list_process(&invocation->context->evaluation, runtime->clock, PLAYER);
    break;
  case LIST_BADNAMES:
    badname_list(&invocation->context->evaluation, invocation->context->world,
                 PLAYER, "Disallowed names:");
    break;
  case LIST_LOGFILES:
    log_cache_list(&invocation->context->evaluation,
                   invocation->context->log->cache, PLAYER);
    break;
  default:
    name_table_display(&invocation->context->evaluation, configuration, PLAYER,
                       list_option_table, "Unknown option.  Use one of:", 1);
  }
}
