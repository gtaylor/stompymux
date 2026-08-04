#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * command.c - command parser and support routines
 */

#include <bits/types/struct_rusage.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "btmux_build_config.h"
#include "mux/commands/command.h"
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
#include "mux/server/game.h"
#include "mux/server/maintenance.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "mux/support/hash_table.h"
#include "mux/support/name_table.h"
#include "mux/world/player.h"
#include "mux/world/world_context.h"

#ifdef ARBITRARY_LOGFILES
#include "mux/server/log_cache.h"
#endif

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
                                             DbRef object, bool visited[]) {
  if (!is_good_obj(context->database, object) || visited[object])
    return 0;
  visited[object] = true;
  return lua_visit_object_commands(lua, object, context->player,
                                   list_object_lua_command, context);
}

static size_t list_object_lua_command_sources(LuaRuntime *lua,
                                              LuaCommandListContext *context,
                                              DbRef first, bool visited[]) {
  DbRef object;
  size_t count = 0;

  DOLIST(context->database, object, first) {
    count += list_object_lua_command_source(lua, context, object, visited);
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
  bool *visited = calloc((size_t)database->top, sizeof(*visited));
  size_t count = 0;

  if (!visited)
    return 0;
  if (!is_no_command(database, player))
    count += list_object_lua_command_source(lua, &context, player, visited);
  if (has_location(database, player)) {
    DbRef location = game_object_location(database, player);

    count += list_object_lua_command_sources(
        lua, &context, game_object_contents(database, location), visited);
    if (!is_no_command(database, location))
      count += list_object_lua_command_source(lua, &context, location, visited);
  }
  if (has_contents(database, player))
    count += list_object_lua_command_sources(
        lua, &context, game_object_contents(database, player), visited);

  if (has_location(database, player)) {
    DbRef location = game_object_location(database, player);
    DbRef location_zone = game_object_zone(database, location);
    DbRef player_zone = game_object_zone(database, player);

    if (location_zone != NOTHING) {
      if (typeof_obj(database, location_zone) == OBJECT_TYPE_ROOM) {
        if (location != player_zone)
          count += list_object_lua_command_sources(
              lua, &context, game_object_contents(database, location_zone),
              visited);
      } else if (!is_no_command(database, location_zone)) {
        count += list_object_lua_command_source(lua, &context, location_zone,
                                                visited);
      }
    }
    if (player_zone != NOTHING && !is_no_command(database, player_zone) &&
        location_zone != player_zone)
      count +=
          list_object_lua_command_source(lua, &context, player_zone, visited);
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
  CMDENT *cmdp;
  char *buf, *bp;
  const char *cp;
  size_t count;

  buf = alloc_lbuf("list_cmdtable");
  bp = buf;
  for (cp = "Built-in commands:"; *cp; cp++)
    *bp++ = *cp;
  for (cmdp = command_table; cmdp->cmdname; cmdp++) {
    if (check_access(evaluation->world->database, configuration, player,
                     cmdp->perms)) {
      if (!(cmdp->perms & CF_DARK)) {
        *bp++ = ' ';
        for (cp = cmdp->cmdname; *cp; cp++)
          *bp++ = *cp;
      }
    }
  }
  *bp = '\0';

  notify_checked(evaluation, player, player, buf, MSG_ME_ALL | MSG_F_DOWN);
  free_lbuf(buf);

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
  char *playerb, *roomb, *thingb, *exitb, *buff;

  playerb =
      decode_flags(evaluation->world->database, player, OBJECT_TYPE_PLAYER,
                   &configuration->default_player_flags);
  roomb = decode_flags(evaluation->world->database, player, OBJECT_TYPE_ROOM,
                       &configuration->default_room_flags);
  exitb = decode_flags(evaluation->world->database, player, OBJECT_TYPE_EXIT,
                       &configuration->default_exit_flags);
  thingb = decode_flags(evaluation->world->database, player, OBJECT_TYPE_THING,
                        &configuration->default_thing_flags);
  buff = alloc_lbuf("list_df_flags");
  snprintf(buff, LBUF_SIZE,
           "Default flags: Players...%s Rooms...%s Exits...%s Things...%s",
           playerb, roomb, exitb, thingb);
  raw_notify(evaluation, player, buff);
  free_lbuf(buff);
  free_sbuf(playerb);
  free_sbuf(roomb);
  free_sbuf(exitb);
  free_sbuf(thingb);
}

/*
 * ---------------------------------------------------------------------------
 * * list_options: List more game options from the context configuration.
 */

static const char *ed[] = {"Disabled", "Enabled"};

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
  raw_notify(
      evaluation, player,
      tprintf("Players may have at most %d commands in the queue at one time.",
              configuration->command_queue_limit));
  if (!is_wizard(evaluation->world->database, player))
    return;

  raw_notify(
      evaluation, player,
      tprintf(
          "%d commands are run from the queue when there is no net activity.",
          configuration->command_queue_idle_chunk));
  raw_notify(
      evaluation, player,
      tprintf("%d commands are run from the queue when there is net activity.",
              configuration->command_queue_active_chunk));
  raw_notify(evaluation, player,
             tprintf("The %s cache is %d entries wide by %d entries deep.",
                     CACHING, configuration->cache_width,
                     configuration->cache_depth));
  if (configuration->cache_names)
    raw_notify(evaluation, player, "A seperate name cache is used.");
  if (configuration->cache_trim)
    raw_notify(
        evaluation, player,
        "The cache depth is periodically trimmed back to its initial value.");
  if (configuration->database.fork_dump) {
    raw_notify(evaluation, player,
               "Database dumps are performed by a fork()ed process.");
  }
  if (configuration->max_players >= 0)
    raw_notify(evaluation, player,
               tprintf("There may be at most %d players logged in at once.",
                       configuration->max_players));
  raw_notify(evaluation, player,
             tprintf("The head of the object freelist is #%ld.",
                     runtime->world->database->freelist));

  snprintf(buff, MBUF_SIZE, "Intervals: Dump...%d  Clean...%d  Idlecheck...%d",
           configuration->database.dump_interval, configuration->check_interval,
           configuration->idle_interval);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Timers: Dump...%ld  Clean...%ld  Idlecheck...%ld",
           (long)runtime->clock->dump_deadline - now,
           (long)runtime->clock->check_deadline - now,
           (long)runtime->clock->idle_deadline - now);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Timeouts: Idle...%d  Connect...%d  Tries...%d",
           configuration->idle_timeout, configuration->conn_timeout,
           configuration->retry_limit);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE,
           "Scheduling: Timeslice...%d  Max_Quota...%d  Increment...%d",
           configuration->command_quota_interval,
           configuration->command_quota_max,
           configuration->command_quota_increment);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Spaces...%s", ed[configuration->space_compress]);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE,
           "New characters: Room...#%d  Home...#%d  DefaultHome...#%d",
           configuration->start_room, configuration->start_home,
           configuration->default_home);
  raw_notify(evaluation, player, buff);

  snprintf(buff, MBUF_SIZE, "Queue: IdleChunk...%d  ActiveChunk...%d",
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
  int pid, psize, maxfds;

  struct rusage usage;
  int curr, last, dur;

  getrusage(RUSAGE_SELF, &usage);
  /*
   * Calculate memory use from the aggregate totals
   */

  curr = clock->current_sample;
  last = 1 - curr;
  dur = clock->sample_time[curr] - clock->sample_time[last];
  if (dur > 0) {
  }
  maxfds = getdtablesize();

  pid = getpid();
  psize = getpagesize();

  /*
   * Go display everything
   */

  raw_notify(
      evaluation, player,
      tprintf("Process ID:  %10d        %10d bytes per page", pid, psize));
  raw_notify(evaluation, player,
             tprintf("Time used:   %10ld user   %10ld sys",
                     usage.ru_utime.tv_sec, usage.ru_stime.tv_sec));

  /*
   * raw_notify(evaluation, player,
   * * tprintf("Resident mem:%10d shared %10d private%10d stack",
   * * ixrss, idrss, isrss));
   */
  raw_notify(evaluation, player,
             tprintf("Integral mem:%10ld shared %10ld private%10ld stack",
                     usage.ru_ixrss, usage.ru_idrss, usage.ru_isrss));
  raw_notify(evaluation, player,
             tprintf("Max res mem: %10ld pages  %10ld bytes", usage.ru_maxrss,
                     (usage.ru_maxrss * psize)));
  raw_notify(evaluation, player,
             tprintf("Page faults: %10ld hard   %10ld soft   %10ld swapouts",
                     usage.ru_majflt, usage.ru_minflt, usage.ru_nswap));
  raw_notify(evaluation, player,
             tprintf("Disk I/O:    %10ld reads  %10ld writes", usage.ru_inblock,
                     usage.ru_oublock));
  raw_notify(evaluation, player,
             tprintf("Network I/O: %10ld in     %10ld out", usage.ru_msgrcv,
                     usage.ru_msgsnd));
  raw_notify(evaluation, player,
             tprintf("Context swi: %10ld vol    %10ld forced %10ld sigs",
                     usage.ru_nvcsw, usage.ru_nivcsw, usage.ru_nsignals));
  raw_notify(evaluation, player, tprintf("Descs avail: %10d", maxfds));
}

/*
 * ---------------------------------------------------------------------------
 * * do_list: List information stored in internal structures.
 */

constexpr int LIST_COMMANDS = 2;
constexpr int LIST_FLAGS = 4;
constexpr int LIST_GLOBALS = 6;
constexpr int LIST_LOGGING = 8;
constexpr int LIST_DF_FLAGS = 9;
constexpr int LIST_PERMS = 10;
constexpr int LIST_OPTIONS = 12;
constexpr int LIST_CONF_PERMS = 15;
constexpr int LIST_SITEINFO = 16;
constexpr int LIST_POWERS = 17;
constexpr int LIST_SWITCHES = 18;
constexpr int LIST_PROCESS = 21;
constexpr int LIST_BADNAMES = 22;
constexpr int LIST_LOGFILES = 23;

NameTable list_names[] = {{"bad_names", 2, CA_WIZARD, LIST_BADNAMES},
                          {"commands", 3, CA_PUBLIC, LIST_COMMANDS},
                          {"config_permissions", 3, CA_GOD, LIST_CONF_PERMS},
                          {"default_flags", 1, CA_PUBLIC, LIST_DF_FLAGS},
                          {"flags", 2, CA_PUBLIC, LIST_FLAGS},
                          {"globals", 1, CA_WIZARD, LIST_GLOBALS},
                          {"logging", 4, CA_GOD, LIST_LOGGING},
                          {"options", 1, CA_PUBLIC, LIST_OPTIONS},
                          {"permissions", 2, CA_WIZARD, LIST_PERMS},
                          {"powers", 2, CA_WIZARD, LIST_POWERS},
                          {"process", 2, CA_WIZARD, LIST_PROCESS},
                          {"site_information", 2, CA_WIZARD, LIST_SITEINFO},
                          {"switches", 2, CA_PUBLIC, LIST_SWITCHES},
#ifdef ARBITRARY_LOGFILES
                          {"logfiles", 4, CA_WIZARD, LIST_LOGFILES},
#endif
                          {nullptr, 0, 0, 0}};

extern NameTable logoptions_nametab[];
extern NameTable logdata_nametab[];

void do_list(CommandInvocation *invocation) {
  CommandRuntime *runtime = invocation->context->runtime;
  ServerConfiguration *configuration = runtime->world->configuration;
  const DbRef player = invocation->player;
  char *arg = invocation->first;
  int flagvalue;

  flagvalue = name_table_search(runtime->world->database, configuration, player,
                                list_names, arg);
  switch (flagvalue) {
  case LIST_COMMANDS:
    list_cmdtable(&invocation->context->evaluation, runtime, player);
    break;
  case LIST_SWITCHES:
    command_list_switches(&invocation->context->evaluation, configuration,
                          player);
    break;
  case LIST_OPTIONS:
    list_options(&invocation->context->evaluation, runtime, player);
    break;
  case LIST_SITEINFO:
    list_siteinfo(&invocation->context->evaluation,
                  invocation->context->world->access_control, player);
    break;
  case LIST_FLAGS:
    display_flagtab(&invocation->context->evaluation, player);
    break;
  case LIST_GLOBALS:
    list_global_controls(&invocation->context->evaluation, configuration,
                         player);
    break;
  case LIST_DF_FLAGS:
    list_df_flags(&invocation->context->evaluation, configuration, player);
    break;
  case LIST_PERMS:
    command_list_access(&invocation->context->evaluation, configuration,
                        runtime->command_registry, player);
    break;
  case LIST_CONF_PERMS:
    configuration_list_access(&invocation->context->evaluation, player);
    break;
  case LIST_POWERS:
    display_powertab(&invocation->context->evaluation, player);
    break;
  case LIST_LOGGING:
    name_table_interpret(&invocation->context->evaluation, configuration,
                         player, logoptions_nametab, configuration->log_options,
                         "Events Logged:", "enabled", "disabled");
    name_table_interpret(&invocation->context->evaluation, configuration,
                         player, logdata_nametab, configuration->log_info,
                         "Information Logged:", "yes", "no");
    break;
  case LIST_PROCESS:
    list_process(&invocation->context->evaluation, runtime->clock, player);
    break;
  case LIST_BADNAMES:
    badname_list(&invocation->context->evaluation, invocation->context->world,
                 player, "Disallowed names:");
    break;
#ifdef ARBITRARY_LOGFILES
  case LIST_LOGFILES:
    log_cache_list(&invocation->context->evaluation,
                   invocation->context->log->cache, player);
    break;
#endif
  default:
    name_table_display(&invocation->context->evaluation, configuration, player,
                       list_names, "Unknown option.  Use one of:", 1);
  }
}
