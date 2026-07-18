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
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/red_black_tree.h"

/* The LOGFILE_TIMEOUT field describes how long a mux should keep an idle
 * open. LOGFILE_TIMEOUT seconds after the last write, it will close. The
 * timer is reset on each write. */
constexpr int LOGFILE_TIMEOUT = 300; // Five Minutes

struct logfile_t {
  LogCache *cache;
  char *filename;
  int fd;
  MuxTimer *timer;
};

struct LogCache {
  uv_loop_t *loop;
  ServerLog *log;
  RedBlackTree files;
};

static int logcache_compare(void *vleft, void *vright, void *arg) {
  return strcmp((char *)vleft, (char *)vright);
}

static int log_cache_close(LogCache *cache, struct logfile_t *log,
                           bool remove_from_cache) {
  dprintk("closing logfile '%s'.", log->filename);
  mux_timer_destroy(log->timer);
  close(log->fd);
  if (remove_from_cache)
    red_black_tree_delete(cache->files, log->filename);
  if (log->filename)
    free(log->filename);
  log->filename = nullptr;
  log->fd = -1;
  free(log);
  return 1;
}

static void logcache_expire(MuxTimer *timer, void *arg) {
  struct logfile_t *log = arg;

  dprintk("Expiring '%s'.", log->filename);
  log_cache_close(log->cache, log, true);
}

typedef struct LogCacheListContext {
  EvaluationContext *evaluation;
  DbRef player;
} LogCacheListContext;

static int _logcache_list(void *key, void *data, int depth, void *arg) {
  struct logfile_t *log = (struct logfile_t *)data;
  LogCacheListContext *context = arg;
  notify_printf(context->evaluation, context->player, "%-40s%llu",
                log->filename,
                (unsigned long long)(mux_timer_due_in(log->timer) / 1000));
  return 1;
}

void log_cache_list(EvaluationContext *evaluation, const LogCache *cache,
                    DbRef player) {
  LogCacheListContext context = {.evaluation = evaluation, .player = player};
  notify(evaluation, player, "/--------------------------- Open Logfiles");
  if (red_black_tree_size(cache->files) == 0) {
    notify(evaluation, player, "- There are no open logfile handles.");
    return;
  }
  notify(evaluation, player, "Filename                               Timeout");
  red_black_tree_walk(cache->files, WALK_INORDER, _logcache_list, &context);
}

static int log_cache_open(LogCache *cache, char *filename) {
  int fd;
  struct logfile_t *newlog;

  if (red_black_tree_exists(cache->files, filename)) {
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
    log_perror(cache->log, "LOGCACHE", "FAIL", nullptr,
               "fcntl(fd, F_SETFD, FD_CLOEXEC)");
  }

  newlog = malloc(sizeof(struct logfile_t));
  newlog->cache = cache;
  newlog->fd = fd;
  newlog->filename = strdup(filename);
  newlog->timer = mux_timer_create(cache->loop, logcache_expire, newlog);
  if (newlog->timer == nullptr) {
    close(newlog->fd);
    free(newlog->filename);
    free(newlog);
    return 0;
  }
  mux_timer_start(newlog->timer, LOGFILE_TIMEOUT * 1000, 0);
  red_black_tree_insert(cache->files, newlog->filename, newlog);
  dprintk("opened logfile '%s' fd = %d.", filename, fd);
  return 1;
}

LogCache *log_cache_create(uv_loop_t *loop, ServerLog *log) {
  LogCache *cache = calloc(1, sizeof(*cache));

  if (cache == nullptr)
    return nullptr;
  cache->loop = loop;
  cache->log = log;
  cache->files = red_black_tree_init(logcache_compare, nullptr);
  if (cache->files == nullptr) {
    free(cache);
    return nullptr;
  }
  return cache;
}

static void log_cache_release_file(void *key, void *data, void *arg) {
  LogCache *cache = arg;
  struct logfile_t *log = (struct logfile_t *)data;

  log_cache_close(cache, log, false);
}

void log_cache_destroy(LogCache *cache) {
  if (cache == nullptr)
    return;
  red_black_tree_release(cache->files, log_cache_release_file, cache);
  free(cache);
}

int log_cache_write(LogCache *cache, char *fname, const char *fdata) {
  struct logfile_t *log;
  int len;

  len = (int)strlen(fdata);

  log = red_black_tree_find(cache->files, fname);

  if (!log) {
    if (!log_cache_open(cache, fname)) {
      return 0;
    }
    log = red_black_tree_find(cache->files, fname);
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
    log_cache_close(cache, log, true);
  }
  return 1;
}
