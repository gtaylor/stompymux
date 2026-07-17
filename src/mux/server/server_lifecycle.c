/*
 * Server startup, service shutdown, and libuv lifecycle orchestration.
 */

#include "mux/server/platform.h"

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
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/server/signals.h"
#include "mux/server/timer.h"
#include "mux/support/alloc.h"

static uv_loop_t server_event_loop;
static bool server_event_loop_initialized;
static MuxTimer *queue_timer;
static MuxTimer *shutdown_timer;
static uint64_t shutdown_started_at;
static struct timeval last_slice;
static struct timeval current_time;

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
static void server_lifecycle_process_preload(void) {
  DbRef thing;
  DbRef parent;
  DbRef aowner;
  long aflags;
  int level;
  char *text;
  FWDLIST *forward_list;

  forward_list = (FWDLIST *)alloc_lbuf("process_preload.fwdlist");
  text = alloc_lbuf("process_preload.string");
  DO_WHOLE_DB(thing) {
    if (is_going(thing))
      continue;

    do_top(10);
    ITER_PARENTS(thing, parent, level) {
      if (obj_flags(thing) & HAS_STARTUP) {
        did_it(obj_owner(thing), thing, 0, nullptr, 0, nullptr, A_STARTUP,
               nullptr, 0);
        do_second();
        do_top(10);
        break;
      }
    }

    if (has_fwdlist(thing)) {
      (void)attribute_get_string(text, thing, A_FORWARDLIST, &aowner, &aflags);
      if (*text) {
        fwdlist_load(forward_list, GOD, text);
        if (forward_list->count > 0)
          fwdlist_set(thing, forward_list);
      }
    }
  }
  free_lbuf(forward_list);
  free_lbuf(text);
}

/* Reschedule the queue tick, replenish command quotas, and run queued work. */
static void server_lifecycle_run_queues(MuxTimer *timer, void *arg) {
  pid_t child;
  int status = 0;

  gettimeofday(&current_time, nullptr);
  last_slice = update_quotas(last_slice, current_time);
  child = waitpid(-1, &status, WNOHANG);
  if (child > 0) {
    dprintk("unexpected child %d exited with exit status %d.", child,
            WEXITSTATUS(status));
  }
  if (mudconf.queue_chunk)
    do_top(mudconf.queue_chunk);
}

static void server_lifecycle_close_timers(uv_handle_t *handle, void *arg) {
  if (uv_handle_get_type(handle) == UV_TIMER && !uv_is_closing(handle))
    mux_timer_destroy(handle->data);
}

static void server_lifecycle_drain_writes(MuxTimer *timer, void *arg) {
  Descriptor *descriptor;
  DescriptorIterator iterator;
  bool deadline_reached;

  deadline_reached = uv_hrtime() - shutdown_started_at >= 1000000000ULL;
  if (descriptor_count() != 0 && !deadline_reached)
    return;
  if (deadline_reached) {
    iterator = descriptor_iterator_all();
    while ((descriptor = descriptor_iterator_next(&iterator)) != nullptr)
      descriptor_force_close(descriptor);
  }
  mux_timer_destroy(shutdown_timer);
  shutdown_timer = nullptr;
}

int server_lifecycle_initialize(void) {
  if (uv_loop_init(&server_event_loop) < 0)
    return 0;
  server_event_loop_initialized = true;
  return 1;
}

uv_loop_t *server_lifecycle_loop(void) {
  return server_event_loop_initialized ? &server_event_loop : nullptr;
}

/* Initialize process-wide state that must exist before database validation. */
void server_lifecycle_prepare(void) {
  srandom((unsigned int)getpid());
  bind_signals();
}

/* Start services required after the database and descriptor state are ready. */
int server_lifecycle_boot(int mindb) {
  char lua_error[LBUF_SIZE];

  mudstate.now = time(nullptr);
  if (!lua_initialize(lua_error, sizeof(lua_error))) {
    log_error(LOG_ALWAYS, "INI", "LUA", "Unable to initialize Lua: %s",
              lua_error);
    return 0;
  }
  server_lifecycle_process_preload();
#ifdef ARBITRARY_LOGFILES
  logcache_init();
#endif
  init_timer();
  return 1;
}

/* Start socket listeners and run the event loop until a shutdown is requested.
 */
void server_lifecycle_run(int port) {
  if (!telnet_socket_listen(port))
    return;
  queue_timer = mux_timer_create(&server_event_loop,
                                 server_lifecycle_run_queues, nullptr);
  if (queue_timer == nullptr ||
      !mux_timer_start(queue_timer, (uint64_t)mudconf.timeslice,
                       (uint64_t)mudconf.timeslice)) {
    log_error(LOG_ALWAYS, "INI", "EVENT", "Unable to create queue timer.");
    return;
  }
  gettimeofday(&last_slice, nullptr);
  gettimeofday(&current_time, nullptr);
#ifdef BTMUX_PERSISTENCE_TESTING
  server_lifecycle_signal_test_ready();
#endif
  uv_run(&server_event_loop, UV_RUN_DEFAULT);
}

/* Request that the active event loop return at its next safe opportunity. */
void server_lifecycle_stop(void) {
  if (server_event_loop_initialized)
    uv_stop(&server_event_loop);
}

/* Stop services and flush pending output before process exit. */
void server_lifecycle_shutdown(void) {
  server_lifecycle_stop();
  if (queue_timer != nullptr) {
    mux_timer_destroy(queue_timer);
    queue_timer = nullptr;
  }
  timer_shutdown();
  heartbeat_stop();
  lua_shutdown();
  flush_sockets();
#ifdef ARBITRARY_LOGFILES
  logcache_destruct();
#endif
  signals_shutdown();
  mux_release_socket();
  if (server_event_loop_initialized) {
    int status;

    uv_walk(&server_event_loop, server_lifecycle_close_timers, nullptr);
    if (descriptor_count() != 0) {
      shutdown_started_at = uv_hrtime();
      shutdown_timer = mux_timer_create(&server_event_loop,
                                        server_lifecycle_drain_writes, nullptr);
      if (shutdown_timer != nullptr)
        mux_timer_start(shutdown_timer, 10, 10);
    }
    uv_run(&server_event_loop, UV_RUN_DEFAULT);
    status = uv_loop_close(&server_event_loop);
    if (status == 0)
      server_event_loop_initialized = false;
    else
      log_error(LOG_ALWAYS, "INI", "EVENT",
                "Unable to close libuv event loop: %s", uv_strerror(status));
  }
}
