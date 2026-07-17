/*
 * timer.c -- Subroutines for (system-) timed events
 */

#include "mux/server/platform.h"

#include "p.glue.h"

#include <signal.h>

#include "mux/commands/command.h"
#include "mux/commands/command_queue.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/powers.h"
#include "mux/lua/lua_runtime.h"
#include "mux/server/platform.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/server/timer.h"
#include "mux/support/stringutil.h"
#include "mux/world/match.h"

extern void pool_reset(void);
extern void fork_and_dump(int key);
extern unsigned int alarm(unsigned int seconds);
extern void pcache_trim(void);
static void check_events(void);

static void timer_callback(evutil_socket_t fd, short event, void *arg);

static struct timeval tv = {0, 100000};
static struct event *timer_event;

void init_timer(void) {
  mudstate.now = time(nullptr);
  mudstate.dump_counter =
      ((mudconf.dump_offset == 0) ? mudconf.database.dump_interval
                                  : mudconf.dump_offset) +
      mudstate.now;
  mudstate.check_counter =
      ((mudconf.check_offset == 0) ? mudconf.check_interval
                                   : mudconf.check_offset) +
      mudstate.now;
  mudstate.idle_counter = mudconf.idle_interval + mudstate.now;
  mudstate.mstats_counter = 15 + mudstate.now;
  mudstate.events_counter = 900 + mudstate.now;
  timer_event =
      evtimer_new(server_lifecycle_event_base(), timer_callback, nullptr);
  if (timer_event != nullptr)
    evtimer_add(timer_event, &tv);
}

void check_idle(void) {
  Descriptor *d, *dnext;
  time_t idletime;

  for (d = descriptor_first(); d != nullptr; d = dnext) {
    dnext = descriptor_next(d);
    if (d->is_connected) {
      idletime = mudstate.now - d->last_time;
      if ((idletime > d->timeout) && !can_idle(d->player)) {
        descriptor_queue_string(d, "*** Inactivity Timeout ***\r\n");
        descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_TIMEOUT);
      } else if (mudconf.idle_wiz_dark && (idletime > mudconf.idle_timeout) &&
                 can_idle(d->player) && !is_dark(d->player)) {
        s_flags(d->player, obj_flags(d->player) | DARK);
        d->is_autodark = true;
      }
    } else {
      idletime = mudstate.now - d->connected_at;
      if (idletime > mudconf.conn_timeout) {
        descriptor_queue_string(d, "*** Login Timeout ***\r\n");
        descriptor_shutdown(d, DESCRIPTOR_SHUTDOWN_TIMEOUT);
      }
    }
  }
}

static void check_events(void) {
  struct tm *ltime;
  DbRef thing, parent;
  int lev;

  ltime = localtime(&mudstate.now);
  if ((ltime->tm_hour == mudconf.events_daily_hour) &&
      !(mudstate.events_flag & ET_DAILY)) {
    mudstate.events_flag = mudstate.events_flag | ET_DAILY;
    DO_WHOLE_DB(thing) {
      if (is_going(thing))
        continue;

      ITER_PARENTS(thing, parent, lev) {
        if (obj_flags2(thing) & HAS_DAILY) {
          did_it(obj_owner(thing), thing, 0, nullptr, 0, nullptr, A_DAILY,
                 (char **)nullptr, 0);

          break;
        }
      }
    }
  }
  if (ltime->tm_hour != mudstate.events_lasthour) {
    if (mudstate.events_lasthour >= 0) {
      /* Run hourly maintenance */
      DO_WHOLE_DB(thing) {
        if (is_going(thing))
          continue;

        ITER_PARENTS(thing, parent, lev) {
          if (obj_flags2(thing) & HAS_HOURLY) {
            did_it(obj_owner(thing), thing, 0, nullptr, 0, nullptr, A_HOURLY,
                   (char **)nullptr, 0);

            break;
          }
        }
      }
    }
    mudstate.events_lasthour = ltime->tm_hour;
  }
  if (ltime->tm_hour == 23) { /*
                               * Nightly resetting
                               */
    mudstate.events_flag = 0;
  }
}

static void dispatch(void) {
  const char *cmdsave;

  cmdsave = mudstate.debug_cmd;
  mudstate.debug_cmd = "< dispatch >";
  /*
   * this routine can be used to poll from interface.c
   */

  if (!mudstate.alarm_triggered)
    return;
  mudstate.alarm_triggered = 0;
  mudstate.now = time(nullptr);

  do_second();
  lua_schedule_tick(mudstate.now);

  /*
   * Free list reconstruction
   */

  if ((mudconf.control_flags & CF_DBCHECK) &&
      (mudstate.check_counter <= mudstate.now)) {
    mudstate.check_counter = mudconf.check_interval + mudstate.now;
    mudstate.debug_cmd = "< dbck >";
    do_dbck(NOTHING, NOTHING, 0);
    pcache_trim();
  }
  /*
   * Database dump routines
   */

  if ((mudconf.control_flags & CF_CHECKPOINT) &&
      (mudstate.dump_counter <= mudstate.now)) {
    mudstate.dump_counter = mudconf.database.dump_interval + mudstate.now;
    mudstate.debug_cmd = "< dump >";
    fork_and_dump(0);
  }
  /*
     Mech stuff ; hopefully it means once ~per sec, although you
     can never be sure - therefore, the code does 'timejumps' as
     needed (see UpdateSpecialObjects for details)
   */

  if (mudconf.have_specials)
    UpdateSpecialObjects();

  /*
   * Idle user check
   */

  if ((mudconf.control_flags & CF_IDLECHECK) &&
      (mudstate.idle_counter <= mudstate.now)) {
    mudstate.idle_counter = mudconf.idle_interval + mudstate.now;
    mudstate.debug_cmd = "< idlecheck >";
    check_idle();
  }
  /*
   * Check for execution of attribute events
   */

  if ((mudconf.control_flags & CF_EVENTCHECK) &&
      (mudstate.events_counter <= mudstate.now)) {
    mudstate.events_counter = 900 + mudstate.now;
    mudstate.debug_cmd = "< eventcheck >";
    check_events();
  }
  /*
   * Memory use stats
   */

  if (mudstate.mstats_counter <= mudstate.now) {

    int curr;

    mudstate.mstats_counter = 15 + mudstate.now;
    curr = mudstate.mstat_curr;
    if (mudstate.now > mudstate.mstat_secs[curr]) {

      struct rusage usage;

      curr = 1 - curr;
      getrusage(RUSAGE_SELF, &usage);
      mudstate.mstat_ixrss[curr] = (int)usage.ru_ixrss;
      mudstate.mstat_idrss[curr] = (int)usage.ru_idrss;
      mudstate.mstat_isrss[curr] = (int)usage.ru_isrss;
      mudstate.mstat_secs[curr] = (int)mudstate.now;
      mudstate.mstat_curr = curr;
    }
  }
  mudstate.debug_cmd = cmdsave;
}

static void timer_callback(evutil_socket_t fd, short event, void *arg) {
  mudstate.alarm_triggered = 1;
  evtimer_add(timer_event, &tv);
  dispatch();
}

void timer_shutdown(void) {
  if (timer_event != nullptr) {
    event_free(timer_event);
    timer_event = nullptr;
  }
}

/**
 * Adjust various internal timers.
 */
void do_timewarp(DbRef player, DbRef cause, int key, char *arg) {
  int secs;

  secs = clamped_atoi(arg);

  if ((key == 0) || (key & TWARP_QUEUE)) /*
                                          * Sem/Wait queues
                                          */
    do_queue(player, cause, QUEUE_WARP, arg);
  if (key & TWARP_DUMP)
    mudstate.dump_counter -= secs;
  if (key & TWARP_CLEAN)
    mudstate.check_counter -= secs;
  if (key & TWARP_IDLE)
    mudstate.idle_counter -= secs;
  if (key & TWARP_EVENTS)
    mudstate.events_counter -= secs;
}
