/*
 * timer.c -- Subroutines for (system-) timed events
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "p.glue.h"

#include <signal.h>

#include "mux/commands/command.h"
#include "mux/commands/command_queue.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/event_timer.h"
#include "mux/server/maintenance.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/timer.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"

extern void pool_reset(void);
extern unsigned int alarm(unsigned int seconds);
static void check_events(MaintenanceContext *maintenance);

static void timer_callback(MuxTimer *timer, void *arg);

struct ServerTimer {
  MuxTimer *event;
  MaintenanceContext *maintenance;
};

ServerTimer *server_timer_create(uv_loop_t *loop,
                                 MaintenanceContext *maintenance) {
  ServerTimer *timer = calloc(1, sizeof(*timer));

  if (timer == nullptr)
    return nullptr;
  timer->maintenance = maintenance;
  maintenance->clock->now = time(nullptr);
  maintenance->clock->dump_deadline =
      ((maintenance->configuration->dump_offset == 0)
           ? maintenance->configuration->database.dump_interval
           : maintenance->configuration->dump_offset) +
      maintenance->clock->now;
  maintenance->clock->check_deadline =
      ((maintenance->configuration->check_offset == 0)
           ? maintenance->configuration->check_interval
           : maintenance->configuration->check_offset) +
      maintenance->clock->now;
  maintenance->clock->idle_deadline =
      maintenance->configuration->idle_interval + maintenance->clock->now;
  maintenance->clock->metrics_deadline = 15 + maintenance->clock->now;
  maintenance->clock->events_deadline = 900 + maintenance->clock->now;
  timer->event = mux_timer_create(loop, timer_callback, timer);
  if (timer->event == nullptr || !mux_timer_start(timer->event, 100, 100)) {
    server_timer_destroy(timer);
    return nullptr;
  }
  return timer;
}

static void check_idle(MaintenanceContext *maintenance) {
  Descriptor *d;
  DescriptorIterator iterator =
      descriptor_iterator_all(maintenance->descriptors);
  time_t idletime;

  while ((d = descriptor_iterator_next(&iterator)) != nullptr) {
    if (d->is_dead)
      continue;
    if (d->is_connected) {
      idletime = maintenance->clock->now - d->last_time;
      if ((idletime > d->timeout) &&
          !can_idle(maintenance->database, d->player)) {
        descriptor_queue_string(d, "*** Inactivity Timeout ***\r\n");
        descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_TIMEOUT);
      } else if (maintenance->configuration->idle_wiz_dark &&
                 (idletime > maintenance->configuration->idle_timeout) &&
                 can_idle(maintenance->database, d->player) &&
                 !is_dark(maintenance->database, d->player)) {
        game_object_set_flags(
            maintenance->database, d->player,
            game_object_flags(maintenance->database, d->player) | DARK);
        d->is_autodark = true;
      }
    } else {
      idletime = maintenance->clock->now - d->connected_at;
      if (idletime > maintenance->configuration->conn_timeout) {
        descriptor_queue_string(d, "*** Login Timeout ***\r\n");
        descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_TIMEOUT);
      }
    }
  }
}

static void check_events(MaintenanceContext *maintenance) {
  struct tm *ltime;
  DbRef thing, parent;
  int lev;

  ltime = localtime(&maintenance->clock->now);
  if ((ltime->tm_hour == maintenance->configuration->events_daily_hour) &&
      !(maintenance->clock->events_run & ET_DAILY)) {
    maintenance->clock->events_run = maintenance->clock->events_run | ET_DAILY;
    DO_WHOLE_DB(maintenance->database, thing) {
      if (is_going(maintenance->database, thing))
        continue;

      ITER_PARENTS(maintenance->database, maintenance->configuration, thing,
                   parent, lev) {
        if (game_object_flags2(maintenance->database, thing) & HAS_DAILY) {
          did_it(&maintenance->command->evaluation,
                 game_object_owner(maintenance->database, thing), thing, 0,
                 nullptr, 0, nullptr, A_DAILY, (char **)nullptr, 0);

          break;
        }
      }
    }
  }
  if (ltime->tm_hour != maintenance->clock->events_last_hour) {
    if (maintenance->clock->events_last_hour >= 0) {
      /* Run hourly maintenance */
      DO_WHOLE_DB(maintenance->database, thing) {
        if (is_going(maintenance->database, thing))
          continue;

        ITER_PARENTS(maintenance->database, maintenance->configuration, thing,
                     parent, lev) {
          if (game_object_flags2(maintenance->database, thing) & HAS_HOURLY) {
            did_it(&maintenance->command->evaluation,
                   game_object_owner(maintenance->database, thing), thing, 0,
                   nullptr, 0, nullptr, A_HOURLY, (char **)nullptr, 0);

            break;
          }
        }
      }
    }
    maintenance->clock->events_last_hour = ltime->tm_hour;
  }
  if (ltime->tm_hour == 23) { /*
                               * Nightly resetting
                               */
    maintenance->clock->events_run = 0;
  }
}

static void dispatch(MaintenanceContext *maintenance) {
  const char *cmdsave;

  cmdsave = maintenance->command->debug_command;
  maintenance->command->debug_command = "< dispatch >";
  /*
   * this routine can be used to poll from interface.c
   */

  if (!maintenance->clock->tick_pending)
    return;
  maintenance->clock->tick_pending = false;
  maintenance->clock->now = time(nullptr);

  do_second(maintenance->commands);
  lua_schedule_tick(maintenance->lua->runtime, maintenance->clock->now);

  /*
   * Free list reconstruction
   */

  if (maintenance->configuration->is_db_check_enabled &&
      maintenance->clock->check_deadline <= maintenance->clock->now) {
    maintenance->clock->check_deadline =
        maintenance->configuration->check_interval + maintenance->clock->now;
    maintenance->command->debug_command = "< dbck >";
    database_check(&maintenance->command->evaluation, NOTHING, 0);
    pcache_trim(maintenance->players);
  }
  /*
   * Database dump routines
   */

  if (maintenance->configuration->is_checkpointing_enabled &&
      maintenance->clock->dump_deadline <= maintenance->clock->now) {
    maintenance->clock->dump_deadline =
        maintenance->configuration->database.dump_interval +
        maintenance->clock->now;
    maintenance->command->debug_command = "< dump >";
    fork_and_dump(maintenance->control, 0);
  }
  /*
     Mech stuff ; hopefully it means once ~per sec, although you
     can never be sure - therefore, the code does 'timejumps' as
     needed (see UpdateSpecialObjects for details)
   */

  if (maintenance->configuration->have_specials)
    UpdateSpecialObjects(maintenance->btech);

  /*
   * Idle user check
   */

  if (maintenance->configuration->is_idle_check_enabled &&
      maintenance->clock->idle_deadline <= maintenance->clock->now) {
    maintenance->clock->idle_deadline =
        maintenance->configuration->idle_interval + maintenance->clock->now;
    maintenance->command->debug_command = "< idlecheck >";
    check_idle(maintenance);
  }
  /*
   * Check for execution of attribute events
   */

  if (maintenance->configuration->is_event_check_enabled &&
      maintenance->clock->events_deadline <= maintenance->clock->now) {
    maintenance->clock->events_deadline = 900 + maintenance->clock->now;
    maintenance->command->debug_command = "< eventcheck >";
    check_events(maintenance);
  }
  /*
   * Memory use stats
   */

  if (maintenance->clock->metrics_deadline <= maintenance->clock->now) {

    int curr;

    maintenance->clock->metrics_deadline = 15 + maintenance->clock->now;
    curr = maintenance->clock->current_sample;
    if (maintenance->clock->now > maintenance->clock->sample_time[curr]) {

      struct rusage usage;

      curr = 1 - curr;
      getrusage(RUSAGE_SELF, &usage);
      maintenance->clock->shared_memory[curr] = (int)usage.ru_ixrss;
      maintenance->clock->private_memory[curr] = (int)usage.ru_idrss;
      maintenance->clock->stack_memory[curr] = (int)usage.ru_isrss;
      maintenance->clock->sample_time[curr] = (int)maintenance->clock->now;
      maintenance->clock->current_sample = curr;
    }
  }
  maintenance->command->debug_command = cmdsave;
}

static void timer_callback(MuxTimer *timer, void *arg) {
  ServerTimer *server_timer = arg;
  MaintenanceContext *maintenance = server_timer->maintenance;

  maintenance->clock->tick_pending = true;
  dispatch(maintenance);
}

void server_timer_destroy(ServerTimer *timer) {
  if (timer == nullptr)
    return;
  if (timer->event != nullptr)
    mux_timer_destroy(timer->event);
  free(timer);
}

/**
 * Adjust various internal timers.
 */
void do_timewarp(CommandInvocation *invocation) {
  int secs;
  RuntimeClock *clock = invocation->context->runtime->clock;

  secs = clamped_atoi(invocation->first);

  if ((invocation->key == 0) || (invocation->key & TWARP_QUEUE)) {
    CommandInvocation queue_invocation = *invocation;

    queue_invocation.key = QUEUE_WARP;
    do_queue(&queue_invocation);
  }
  if (invocation->key & TWARP_DUMP)
    clock->dump_deadline -= secs;
  if (invocation->key & TWARP_CLEAN)
    clock->check_deadline -= secs;
  if (invocation->key & TWARP_IDLE)
    clock->idle_deadline -= secs;
  if (invocation->key & TWARP_EVENTS)
    clock->events_deadline -= secs;
}
