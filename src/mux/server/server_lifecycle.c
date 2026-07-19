/*
 * Server startup, service shutdown, and libuv lifecycle orchestration.
 */

#include "mux/commands/command_runtime.h"
#include "mux/server/platform.h"
#include "mux/world/world_context.h"

#include "mux/server/libuv.h"
#include <errno.h>
#include <limits.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "glue.h"
#include "mux/commands/command_queue.h"
#include "mux/communication/commac.h"
#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/database/flags.h"
#include "mux/lua/lua_runtime.h"
#include "mux/network/netcommon.h"
#include "mux/network/telnet_socket.h"
#include "mux/server/diagnostics.h"
#include "mux/server/event_timer.h"
#include "mux/server/log_cache.h"
#include "mux/server/maintenance.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/signals.h"
#include "mux/server/timer.h"
#include "mux/support/alloc.h"

struct ServerLifecycle {
  uv_loop_t event_loop;
  bool event_loop_initialized;
  MuxTimer *queue_timer;
  MuxTimer *shutdown_timer;
  uint64_t shutdown_started_at;
  struct timeval last_slice;
  struct timeval current_time;
  MaintenanceContext *maintenance;
  SignalHandlers *signals;
  TelnetSockets *sockets;
  ServerTimer *timer;
};

#ifdef BTMUX_PERSISTENCE_TESTING
/* Notify the integration test only after the server can handle shutdown. */
static void server_lifecycle_signal_test_ready(void) {
  const char *value;
  char *end;
  long descriptor;

  value = getenv("BTMUX_TEST_READY_FD");
  if (value == nullptr)
    return;
  errno = 0;
  descriptor = strtol(value, &end, 10);
  if (errno != 0 || end == value || *end != '\0' || descriptor < 0 ||
      descriptor > INT_MAX)
    return;
  if (write((int)descriptor, "", 1) != 1)
    return;
}
#endif

/* Run startup attributes and restore each object's forward list after load. */
static void server_lifecycle_process_preload(ServerLifecycle *lifecycle) {
  DbRef thing;
  DbRef parent;
  DbRef aowner;
  long aflags;
  int level;
  char *text;
  FWDLIST *forward_list;

  forward_list = (FWDLIST *)alloc_lbuf("process_preload.fwdlist");
  text = alloc_lbuf("process_preload.string");
  DO_WHOLE_DB(lifecycle->maintenance->database, thing) {
    if (is_going(lifecycle->maintenance->database, thing))
      continue;

    do_top(lifecycle->maintenance->commands, 10);
    ITER_PARENTS(lifecycle->maintenance->database,
                 lifecycle->maintenance->configuration, thing, parent, level) {
      if (game_object_flags(lifecycle->maintenance->database, thing) &
          HAS_STARTUP) {
        did_it(&lifecycle->maintenance->command->evaluation,
               game_object_owner(lifecycle->maintenance->database, thing),
               thing, 0, nullptr, 0, nullptr, A_STARTUP, nullptr, 0);
        do_second(lifecycle->maintenance->commands);
        do_top(lifecycle->maintenance->commands, 10);
        break;
      }
    }

    if (has_fwdlist(lifecycle->maintenance->database, thing)) {
      (void)attribute_get_string(lifecycle->maintenance->database, text, thing,
                                 A_FORWARDLIST, &aowner, &aflags);
      if (*text) {
        fwdlist_load(&lifecycle->maintenance->command->evaluation, forward_list,
                     GOD, text);
        if (forward_list->count > 0)
          fwdlist_set(lifecycle->maintenance->database, thing, forward_list);
      }
    }
  }
  free_lbuf(forward_list);
  free_lbuf(text);
}

/* Reschedule the queue tick, replenish command quotas, and run queued work. */
static void server_lifecycle_run_queues(MuxTimer *timer, void *arg) {
  ServerLifecycle *lifecycle = arg;
  pid_t child;
  int status = 0;

  gettimeofday(&lifecycle->current_time, nullptr);
  lifecycle->last_slice =
      update_quotas(lifecycle->maintenance->configuration,
                    lifecycle->maintenance->descriptors, lifecycle->last_slice,
                    lifecycle->current_time);
  child = waitpid(-1, &status, WNOHANG);
  if (child > 0) {
    dprintk("unexpected child %d exited with exit status %d.", child,
            WEXITSTATUS(status));
  }
  if (lifecycle->maintenance->configuration->queue_chunk)
    do_top(lifecycle->maintenance->commands,
           lifecycle->maintenance->configuration->queue_chunk);
}

static void server_lifecycle_close_timers(uv_handle_t *handle, void *arg) {
  if (uv_handle_get_type(handle) == UV_TIMER && !uv_is_closing(handle))
    mux_timer_destroy(handle->data);
}

static void server_lifecycle_drain_writes(MuxTimer *timer, void *arg) {
  ServerLifecycle *lifecycle = arg;
  Descriptor *descriptor;
  DescriptorIterator iterator;
  bool deadline_reached;

  deadline_reached =
      uv_hrtime() - lifecycle->shutdown_started_at >= 1000000000ULL;
  if (descriptor_count(lifecycle->maintenance->descriptors) != 0 &&
      !deadline_reached)
    return;
  if (deadline_reached) {
    iterator = descriptor_iterator_all(lifecycle->maintenance->descriptors);
    while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr)
      descriptor_force_close(descriptor);
  }
  mux_timer_destroy(lifecycle->shutdown_timer);
  lifecycle->shutdown_timer = nullptr;
}

ServerLifecycle *server_lifecycle_create(MaintenanceContext *maintenance) {
  ServerLifecycle *lifecycle = calloc(1, sizeof(*lifecycle));

  if (lifecycle == nullptr)
    return nullptr;
  if (uv_loop_init(&lifecycle->event_loop) < 0) {
    free(lifecycle);
    return nullptr;
  }
  lifecycle->event_loop_initialized = true;
  lifecycle->maintenance = maintenance;
  lifecycle->sockets =
      telnet_sockets_create(&lifecycle->event_loop, maintenance->connections);
  if (lifecycle->sockets == nullptr) {
    uv_loop_close(&lifecycle->event_loop);
    free(lifecycle);
    return nullptr;
  }
  return lifecycle;
}

void server_lifecycle_destroy(ServerLifecycle *lifecycle) {
  if (lifecycle == nullptr)
    return;
  if (lifecycle->event_loop_initialized)
    server_lifecycle_shutdown(lifecycle);
  telnet_sockets_destroy(lifecycle->sockets);
  free(lifecycle);
}

uv_loop_t *server_lifecycle_loop(ServerLifecycle *lifecycle) {
  return lifecycle != nullptr && lifecycle->event_loop_initialized
             ? &lifecycle->event_loop
             : nullptr;
}

/* Initialize process-wide state that must exist before database validation. */
void server_lifecycle_prepare(ServerLifecycle *lifecycle) {
  srandom((unsigned int)getpid());
  lifecycle->signals = signal_handlers_create(server_lifecycle_loop(lifecycle),
                                              lifecycle->maintenance->control);
}

void server_lifecycle_unbind_signals(ServerLifecycle *lifecycle) {
  if (lifecycle != nullptr)
    signal_handlers_unbind(lifecycle->signals);
}

/* Start services required after the database and descriptor state are ready. */
int server_lifecycle_boot(ServerLifecycle *lifecycle, int mindb) {
  char lua_error[LBUF_SIZE];

  lifecycle->maintenance->clock->now = time(nullptr);
  if (!lua_initialize(lifecycle->maintenance->lua,
                      lifecycle->maintenance->lua_services, lua_error,
                      sizeof(lua_error))) {
    log_error(lifecycle->maintenance->log, LOG_ALWAYS, "INI", "LUA",
              "Unable to initialize Lua: %s", lua_error);
    return 0;
  }
  server_lifecycle_process_preload(lifecycle);
  lifecycle->timer =
      server_timer_create(&lifecycle->event_loop, lifecycle->maintenance);
  return lifecycle->timer != nullptr;
}

/* Start socket listeners and run the event loop until a shutdown is requested.
 */
void server_lifecycle_run(ServerLifecycle *lifecycle, int port) {
  if (!telnet_sockets_listen(lifecycle->sockets, port))
    return;
  lifecycle->queue_timer = mux_timer_create(
      &lifecycle->event_loop, server_lifecycle_run_queues, lifecycle);
  if (lifecycle->queue_timer == nullptr ||
      !mux_timer_start(
          lifecycle->queue_timer,
          (uint64_t)lifecycle->maintenance->configuration->timeslice,
          (uint64_t)lifecycle->maintenance->configuration->timeslice)) {
    log_error(lifecycle->maintenance->log, LOG_ALWAYS, "INI", "EVENT",
              "Unable to create queue timer.");
    return;
  }
  gettimeofday(&lifecycle->last_slice, nullptr);
  gettimeofday(&lifecycle->current_time, nullptr);
#ifdef BTMUX_PERSISTENCE_TESTING
  server_lifecycle_signal_test_ready();
#endif
  uv_run(&lifecycle->event_loop, UV_RUN_DEFAULT);
}

/* Request that the active event loop return at its next safe opportunity. */
void server_lifecycle_stop(ServerLifecycle *lifecycle) {
  if (lifecycle != nullptr && lifecycle->event_loop_initialized)
    uv_stop(&lifecycle->event_loop);
}

void server_lifecycle_release_sockets(ServerLifecycle *lifecycle) {
  if (lifecycle != nullptr)
    telnet_sockets_release(lifecycle->sockets);
}

void server_lifecycle_close_connections(ServerLifecycle *lifecycle,
                                        bool emergency, const char *message) {
  if (lifecycle != nullptr)
    telnet_sockets_close(lifecycle->sockets, emergency, message);
}

int server_lifecycle_eradicate_fd(ServerLifecycle *lifecycle, int fd) {
  if (lifecycle == nullptr)
    return 0;
  return telnet_sockets_eradicate_fd(lifecycle->sockets, fd);
}

/* Stop services and flush pending output before process exit. */
void server_lifecycle_shutdown(ServerLifecycle *lifecycle) {
  if (lifecycle == nullptr)
    return;
  if (lifecycle->queue_timer != nullptr) {
    mux_timer_destroy(lifecycle->queue_timer);
    lifecycle->queue_timer = nullptr;
  }
  server_timer_destroy(lifecycle->timer);
  lifecycle->timer = nullptr;
  heartbeat_stop(lifecycle->maintenance->btech);
  lua_shutdown(lifecycle->maintenance->lua);
  signal_handlers_destroy(lifecycle->signals);
  lifecycle->signals = nullptr;
  telnet_sockets_release(lifecycle->sockets);
  if (lifecycle->event_loop_initialized) {
    int status;

    uv_walk(&lifecycle->event_loop, server_lifecycle_close_timers, nullptr);
    if (descriptor_count(lifecycle->maintenance->descriptors) != 0) {
      lifecycle->shutdown_started_at = uv_hrtime();
      lifecycle->shutdown_timer = mux_timer_create(
          &lifecycle->event_loop, server_lifecycle_drain_writes, lifecycle);
      if (lifecycle->shutdown_timer != nullptr)
        mux_timer_start(lifecycle->shutdown_timer, 10, 10);
    }
    uv_run(&lifecycle->event_loop, UV_RUN_DEFAULT);
    status = uv_loop_close(&lifecycle->event_loop);
    if (status == 0)
      lifecycle->event_loop_initialized = false;
    else
      log_error(lifecycle->maintenance->log, LOG_ALWAYS, "INI", "EVENT",
                "Unable to close libuv event loop: %s", uv_strerror(status));
  }
}
