#include "mux/server/runtime_clock.h" // IWYU pragma: keep
/*
 * timer.c -- Subroutines for (system-) timed events
 */

#include <bits/types/struct_rusage.h>
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <time.h>
#include <unistd.h>

#include "btech/special_objects.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/descriptor.h"
#include "mux/network/network_output.h"
#include "mux/objects/db.h"
#include "mux/objects/powers.h"
#include "mux/server/event_timer.h"
#include "mux/server/game.h"
#include "mux/server/server_config.h"
#include "mux/server/timer.h"
#include "mux/support/checked_storage.h"
#include "mux/world/database_check.h"
#include "mux/world/player_cache.h"

extern void pool_reset(void);

static void timer_callback(MuxTimer *timer, void *arg);

struct ServerTimer {
  MuxTimer *event;
  MaintenanceContext *maintenance;
#ifdef BTECH_PERSISTENCE_TESTING
  unsigned test_ticks;
#endif
};

#ifdef BTECH_PERSISTENCE_TESTING
static void signal_test_tick(void) {
  const char *value = getenv("BTECH_TEST_TICK_FD");
  char *end;
  long descriptor;

  if (value == nullptr)
    return;
  errno = 0;
  descriptor = strtol(value, &end, 10);
  if (errno != 0 || end == value || *end != '\0' || descriptor < 0 ||
      descriptor > INT_MAX)
    return;
  (void)write((int)descriptor, "1", 1);
  close((int)descriptor);
  unsetenv("BTECH_TEST_TICK_FD");
}
#endif

ServerTimer *server_timer_create(UvLoopT *loop,
                                 MaintenanceContext *maintenance) {
  ServerTimer *timer = checked_storage_try_allocate_array(1, sizeof(*timer));
  uint64_t initial_timeout = 100;

  if (timer == nullptr)
    return nullptr;
  timer->maintenance = maintenance;
  maintenance->clock->now = time(nullptr);
#ifdef BTECH_PERSISTENCE_TESTING
  timer->test_ticks = 0;
#endif
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
  timer->event = mux_timer_create(loop, timer_callback, timer);
#ifdef BTECH_PERSISTENCE_TESTING
  if (getenv("BTECH_TEST_TICK_FD") == nullptr)
    initial_timeout = 5000;
#endif
  if (timer->event == nullptr ||
      !mux_timer_start(timer->event, initial_timeout, 100)) {
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

  lua_schedule_tick(maintenance->lua->runtime, maintenance->clock->now);

  /*
   * Free list reconstruction
   */

  if (maintenance->configuration->is_db_check_enabled &&
      maintenance->clock->check_deadline <= maintenance->clock->now) {
    maintenance->clock->check_deadline =
        maintenance->configuration->check_interval + maintenance->clock->now;
    maintenance->command->debug_command = "< dbck >";
    database_check(&(DatabaseCheckRequest){
        .evaluation = &maintenance->command->evaluation, .player = NOTHING});
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

  btech_special_objects_update(maintenance->btech);

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
   * Memory use stats
   */

  if (maintenance->clock->metrics_deadline <= maintenance->clock->now) {

    int curr;

    maintenance->clock->metrics_deadline = 15 + maintenance->clock->now;
    curr = maintenance->clock->current_sample;
    int *sample_time = checked_storage_at(
        maintenance->clock->sample_time, 2,
        sizeof(*maintenance->clock->sample_time), (size_t)curr);
    if (maintenance->clock->now > *sample_time) {

      struct rusage usage;

      curr = 1 - curr;
      getrusage(RUSAGE_SELF, &usage);
      *(int *)checked_storage_at(maintenance->clock->shared_memory, 2,
                                 sizeof(*maintenance->clock->shared_memory),
                                 (size_t)curr) = (int)usage.ru_ixrss;
      *(int *)checked_storage_at(maintenance->clock->private_memory, 2,
                                 sizeof(*maintenance->clock->private_memory),
                                 (size_t)curr) = (int)usage.ru_idrss;
      *(int *)checked_storage_at(maintenance->clock->stack_memory, 2,
                                 sizeof(*maintenance->clock->stack_memory),
                                 (size_t)curr) = (int)usage.ru_isrss;
      *(int *)checked_storage_at(maintenance->clock->sample_time, 2,
                                 sizeof(*maintenance->clock->sample_time),
                                 (size_t)curr) = (int)maintenance->clock->now;
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
#ifdef BTECH_PERSISTENCE_TESTING
  server_timer->test_ticks++;
  if (server_timer->test_ticks >= 11)
    signal_test_tick();
#endif
}

void server_timer_destroy(ServerTimer *timer) {
  if (timer == nullptr)
    return;
  if (timer->event != nullptr)
    mux_timer_destroy(timer->event);
  free(timer);
}
