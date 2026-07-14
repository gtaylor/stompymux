/*
 * Server startup, service shutdown, and libevent lifecycle orchestration.
 */

#include "config.h"

#include <event.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#include "alloc.h"
#include "attrs.h"
#include "commac.h"
#include "cque.h"
#include "db.h"
#include "debug.h"
#include "dnschild.h"
#include "externs.h"
#include "flags.h"
#include "logcache.h"
#include "lua_runtime.h"
#include "mudconf.h"
#include "netcommon.h"
#include "persistence/restart_persistence.h"
#include "server_lifecycle.h"
#include "signals.h"
#include "telnet_socket.h"
#include "timer.h"

static struct timeval queue_slice = {0, 0};
static struct event queue_event;
static struct timeval last_slice;
static struct timeval current_time;

/* Run startup attributes and restore each object's forward list after load. */
static void server_lifecycle_process_preload(void) {
  dbref thing;
  dbref parent;
  dbref aowner;
  long aflags;
  int level;
  char *text;
  FWDLIST *forward_list;

  forward_list = (FWDLIST *)alloc_lbuf("process_preload.fwdlist");
  text = alloc_lbuf("process_preload.string");
  DO_WHOLE_DB(thing) {
    if (Going(thing))
      continue;

    do_top(10);
    ITER_PARENTS(thing, parent, level) {
      if (Flags(thing) & HAS_STARTUP) {
        did_it(Owner(thing), thing, 0, NULL, 0, NULL, A_STARTUP, NULL, 0);
        do_second();
        do_top(10);
        break;
      }
    }

    if (H_Fwdlist(thing)) {
      (void)atr_get_str(text, thing, A_FORWARDLIST, &aowner, &aflags);
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
static void server_lifecycle_run_queues(int fd, short event, void *arg) {
  pid_t child;
  int status = 0;

  event_add(&queue_event, &queue_slice);
  gettimeofday(&current_time, NULL);
  last_slice = update_quotas(last_slice, current_time);
  child = waitpid(-1, &status, WNOHANG);
  if (child > 0) {
    dprintk("unexpected child %d exited with exit status %d.", child,
            WEXITSTATUS(status));
  }
  if (mudconf.queue_chunk)
    do_top(mudconf.queue_chunk);
}

/* Initialize process-wide state that must exist before database validation. */
void server_lifecycle_prepare(void) {
  srandom(getpid());
  bind_signals();
}

/* Start services required after the database and descriptor state are ready. */
int server_lifecycle_boot(int restarting, int mindb) {
  char lua_error[LBUF_SIZE];

  mudstate.now = time(NULL);
  if (!lua_initialize(lua_error, sizeof(lua_error))) {
    log_error(LOG_ALWAYS, "INI", "LUA", "Unable to initialize Lua: %s",
              lua_error);
    return 0;
  }
  server_lifecycle_process_preload();
  dnschild_init();

  if (restarting) {
    if (mindb || restart_persistence_load() < 0) {
      log_error(LOG_ALWAYS, "INI", "RSTRT",
                "Unable to restore SQLite restart continuation state.");
      return 0;
    }
  } else if (!mindb && restart_persistence_discard() < 0) {
    log_error(LOG_ALWAYS, "INI", "RSTRT",
              "Unable to discard stale SQLite restart continuation state.");
    return 0;
  }

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
  evtimer_set(&queue_event, server_lifecycle_run_queues, NULL);
  evtimer_add(&queue_event, &queue_slice);
  gettimeofday(&last_slice, NULL);
  gettimeofday(&current_time, NULL);
  event_dispatch();
}

/* Request that the active event loop return at its next safe opportunity. */
void server_lifecycle_stop(void) { event_loopexit(NULL); }

/* Stop services and flush pending output before process exit or restart. */
void server_lifecycle_shutdown(void) {
  lua_shutdown();
  dnschild_destruct();
  flush_sockets();
#ifdef ARBITRARY_LOGFILES
  logcache_destruct();
#endif
  server_lifecycle_stop();
}
