/*
 * Server startup, service shutdown, and libevent lifecycle orchestration.
 */

#include "mux/server/platform.h"

#include <errno.h>
#include <event2/event.h>
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
#include "mux/server/debug.h"
#include "mux/server/log_cache.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/server/signals.h"
#include "mux/server/timer.h"
#include "mux/support/alloc.h"

static struct event_base *server_event_base;
static struct timeval queue_slice = {0, 0};
static struct event *queue_event;
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
static void server_lifecycle_run_queues(evutil_socket_t fd, short event,
                                        void *arg) {
  pid_t child;
  int status = 0;

  event_add(queue_event, &queue_slice);
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

int server_lifecycle_initialize(void) {
  server_event_base = event_base_new();
  return server_event_base != nullptr;
}

struct event_base *server_lifecycle_event_base(void) {
  return server_event_base;
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
  queue_slice.tv_sec = 0;
  queue_slice.tv_usec = mudconf.timeslice * 1000;
  telnet_socket_listen(port);
  queue_event =
      evtimer_new(server_event_base, server_lifecycle_run_queues, nullptr);
  if (queue_event == nullptr) {
    log_error(LOG_ALWAYS, "INI", "EVENT", "Unable to create queue timer.");
    return;
  }
  evtimer_add(queue_event, &queue_slice);
  gettimeofday(&last_slice, nullptr);
  gettimeofday(&current_time, nullptr);
#ifdef BTMUX_PERSISTENCE_TESTING
  server_lifecycle_signal_test_ready();
#endif
  event_base_dispatch(server_event_base);
}

/* Request that the active event loop return at its next safe opportunity. */
void server_lifecycle_stop(void) {
  if (server_event_base != nullptr)
    event_base_loopexit(server_event_base, nullptr);
}

/* Stop services and flush pending output before process exit. */
void server_lifecycle_shutdown(void) {
  server_lifecycle_stop();
  if (queue_event != nullptr) {
    event_free(queue_event);
    queue_event = nullptr;
  }
  timer_shutdown();
  heartbeat_stop();
  lua_shutdown();
  flush_sockets();
#ifdef ARBITRARY_LOGFILES
  logcache_destruct();
#endif
  if (server_event_base != nullptr) {
    event_base_free(server_event_base);
    server_event_base = nullptr;
  }
}
