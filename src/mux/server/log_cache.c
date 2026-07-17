/* log_cache.c - Cached arbitrary-log file management. */

#include "mux/server/platform.h"

#include <errno.h>
#include <signal.h>
#include <sys/file.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "mux/commands/command.h"
#include "mux/database/attrs.h"
#include "mux/database/flags.h"
#include "mux/server/diagnostics.h"
#include "mux/server/event_timer.h"
#include "mux/server/log_cache.h"
#include "mux/server/server_api.h"
#include "mux/server/server_lifecycle.h"
#include "mux/server/server_state.h"
#include "mux/support/red_black_tree.h"

/* The LOGFILE_TIMEOUT field describes how long a mux should keep an idle
 * open. LOGFILE_TIMEOUT seconds after the last write, it will close. The
 * timer is reset on each write. */
constexpr int LOGFILE_TIMEOUT = 300; // Five Minutes

struct logfile_t {
  char *filename;
  int fd;
  MuxTimer *timer;
};

RedBlackTree logfiles = nullptr;

static int logcache_compare(void *vleft, void *vright, void *arg) {
  return strcmp((char *)vleft, (char *)vright);
}

static int logcache_close(struct logfile_t *log) {
  dprintk("closing logfile '%s'.", log->filename);
  mux_timer_destroy(log->timer);
  close(log->fd);
  red_black_tree_delete(logfiles, log->filename);
  if (log->filename)
    free(log->filename);
  log->filename = nullptr;
  log->fd = -1;
  free(log);
  return 1;
}

static void logcache_expire(MuxTimer *timer, void *arg) {
  dprintk("Expiring '%s'.", ((struct logfile_t *)arg)->filename);
  logcache_close((struct logfile_t *)arg);
}

static int _logcache_list(void *key, void *data, int depth, void *arg) {
  struct logfile_t *log = (struct logfile_t *)data;
  DbRef player = *(DbRef *)arg;
  notify_printf(player, "%-40s%llu", log->filename,
                (unsigned long long)(mux_timer_due_in(log->timer) / 1000));
  return 1;
}

void logcache_list(DbRef player) {
  notify(player, "/--------------------------- Open Logfiles");
  if (red_black_tree_size(logfiles) == 0) {
    notify(player, "- There are no open logfile handles.");
    return;
  }
  notify(player, "Filename                               Timeout");
  red_black_tree_walk(logfiles, WALK_INORDER, _logcache_list, &player);
}

static int logcache_open(char *filename) {
  int fd;
  struct logfile_t *newlog;

  if (red_black_tree_exists(logfiles, filename)) {
    fprintf(stderr, "Serious braindamage, logcache_open() called for already "
                    "open logfile.\n");
    return 0;
  }

  fd = open(filename, O_RDWR | O_APPEND | O_CREAT, 0644);
  if (fd < 0) {
    fprintf(
        stderr,
        "Failed to open logfile %s because open() failed with code: %d -  %s\n",
        filename, errno, strerror(errno));
    return 0;
  }
  if (fcntl(fd, F_SETFD, FD_CLOEXEC) < 0) {
    log_perror("LOGCACHE", "FAIL", nullptr, "fcntl(fd, F_SETFD, FD_CLOEXEC)");
  }

  newlog = malloc(sizeof(struct logfile_t));
  newlog->fd = fd;
  newlog->filename = strdup(filename);
  newlog->timer =
      mux_timer_create(server_lifecycle_loop(), logcache_expire, newlog);
  if (newlog->timer == nullptr) {
    close(newlog->fd);
    free(newlog->filename);
    free(newlog);
    return 0;
  }
  mux_timer_start(newlog->timer, LOGFILE_TIMEOUT * 1000, 0);
  red_black_tree_insert(logfiles, newlog->filename, newlog);
  dprintk("opened logfile '%s' fd = %d.", filename, fd);
  return 1;
}

void logcache_init(void) {
  if (!logfiles) {
    dprintk("logcache initialized.");
    logfiles = red_black_tree_init(logcache_compare, nullptr);
  } else {
    dprintk("REDUNDANT CALL TO logcache_init()!");
  }
}

static int _logcache_destruct(void *key, void *data, int depth, void *arg) {
  struct logfile_t *log = (struct logfile_t *)data;
  logcache_close(log);
  return 1;
}

void logcache_destruct(void) {
  dprintk("logcache destructing.");
  if (!logfiles) {
    dprintk("logcache_destruct() CALLED WHILE UNITIALIZED!");
    return;
  }
  red_black_tree_walk(logfiles, WALK_INORDER, _logcache_destruct, nullptr);
  red_black_tree_destroy(logfiles);
  logfiles = nullptr;
}

int logcache_writelog(char *fname, char *fdata) {
  struct logfile_t *log;
  int len;

  if (!logfiles)
    logcache_init();

  len = (int)strlen(fdata);

  log = red_black_tree_find(logfiles, fname);

  if (!log) {
    if (logcache_open(fname) < 0) {
      return 0;
    }
    log = red_black_tree_find(logfiles, fname);
    if (!log) {
      return 0;
    }
  }

  mux_timer_start(log->timer, LOGFILE_TIMEOUT * 1000, 0);

  if (write(log->fd, fdata, (size_t)len) < 0) {
    fprintf(stderr,
            "System failed to write data to file with error '%s' on logfile "
            "'%s'. Closing.\n",
            strerror(errno), log->filename);
    logcache_close(log);
  }
  return 1;
}
