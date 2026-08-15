/* log_cache.c - Cached arbitrary-log file management. */

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "btech/context.h" // IWYU pragma: keep
#include "mux/server/event_timer.h"
#include "mux/server/game.h"
#include "mux/server/log.h"
#include "mux/server/log_cache.h"
#include "mux/server/platform.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "mux/support/stringutil.h"

/* The LOGFILE_TIMEOUT field describes how long a mux should keep an idle
 * open. LOGFILE_TIMEOUT seconds after the last write, it will close. The
 * timer is reset on each write. */
constexpr int LOGFILE_TIMEOUT = 300; // Five Minutes

struct LogfileT {
  LogCache *cache;
  char *filename;
  int fd;
  MuxTimer *timer;
};

struct LogCache {
  UvLoopT *loop;
  ServerLog *log;
  RedBlackTree files;
};

static int logcache_compare(const RedBlackTreeCompareCall *call) {
  const void *vleft = call->lhs;
  const void *vright = call->rhs;
  return strcmp(vleft, vright);
}

static bool log_cache_close(LogCache *cache, struct LogfileT *log,
                            bool remove_from_cache) {
  mux_timer_destroy(log->timer);
  close(log->fd);
  if (remove_from_cache)
    red_black_tree_delete(cache->files, log->filename);
  if (log->filename)
    free(log->filename);
  log->filename = nullptr;
  log->fd = -1;
  free(log);
  return true;
}

static void logcache_expire(MuxTimer *timer [[maybe_unused]], void *arg) {
  struct LogfileT *log = arg;

  log_cache_close(log->cache, log, true);
}

typedef struct LogCacheListContext {
  EvaluationContext *evaluation;
  DbRef player;
} LogCacheListContext;

static bool logcache_list(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  void *arg = call->context;
  struct LogfileT *log = (struct LogfileT *)data;
  LogCacheListContext *context = arg;
  notify_printf(context->evaluation, context->player, "%-40s%llu",
                log->filename,
                (unsigned long long)(mux_timer_due_in(log->timer) / 1000));
  return true;
}

void log_cache_list(EvaluationContext *evaluation, const LogCache *cache,
                    DbRef player) {
  LogCacheListContext context = {.evaluation = evaluation, .player = player};
  notify_checked(evaluation, player, player,
                 "/--------------------------- Open Logfiles",
                 MSG_ME_ALL | MSG_F_DOWN);
  if (red_black_tree_size(cache->files) == 0) {
    notify_checked(evaluation, player, player,
                   "- There are no open logfile handles.",
                   MSG_ME_ALL | MSG_F_DOWN);
    return;
  }
  notify_checked(evaluation, player, player,
                 "Filename                               Timeout",
                 MSG_ME_ALL | MSG_F_DOWN);
  red_black_tree_walk(cache->files, WALK_INORDER, logcache_list, &context);
}

static bool log_cache_open(LogCache *cache, char *filename) {
  int fd;
  struct LogfileT *newlog;

  if (red_black_tree_exists(cache->files, filename)) {
    (void)fprintf(stderr,
                  "Serious braindamage, logcache_open() called for already "
                  "open logfile.\n");
    return false;
  }

  fd = open(filename, O_RDWR | O_APPEND | O_CREAT, 0644);
  if (fd < 0) {
    (void)fprintf(
        stderr,
        "Failed to open logfile %s because open() failed with code: %d -  %s\n",
        filename, errno,
        system_error_message(errno, (char[SYSTEM_ERROR_MESSAGE_SIZE]){0},
                             SYSTEM_ERROR_MESSAGE_SIZE));
    return false;
  }
  if (fcntl(fd, F_SETFD, FD_CLOEXEC) < 0) {
    log_perror(
        &(LogSystemError){.log = cache->log,
                          .primary = "LOGCACHE",
                          .secondary = "FAIL",
                          .failing_object = "fcntl(fd, F_SETFD, FD_CLOEXEC)"});
  }

  newlog = checked_storage_allocate(sizeof(struct LogfileT));
  newlog->cache = cache;
  newlog->fd = fd;
  newlog->filename = strdup(filename);
  newlog->timer = mux_timer_create(cache->loop, logcache_expire, newlog);
  if (newlog->timer == nullptr) {
    close(newlog->fd);
    free(newlog->filename);
    free(newlog);
    return false;
  }
  mux_timer_start(newlog->timer, (uint64_t)LOGFILE_TIMEOUT * 1000U, 0);
  red_black_tree_insert(cache->files, newlog->filename, newlog);
  return true;
}

LogCache *log_cache_create(UvLoopT *loop, ServerLog *log) {
  LogCache *cache = checked_storage_try_allocate_array(1, sizeof(*cache));

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

static void log_cache_release_file(const RedBlackTreeReleaseCall *call) {
  void *data = call->data;
  void *arg = call->context;
  LogCache *cache = arg;
  struct LogfileT *log = (struct LogfileT *)data;

  log_cache_close(cache, log, false);
}

void log_cache_destroy(LogCache *cache) {
  if (cache == nullptr)
    return;
  red_black_tree_release(cache->files, log_cache_release_file, cache);
  free(cache);
}

bool log_cache_write(LogCache *cache, char *fname, const char *fdata) {
  struct LogfileT *log;
  int len;

  len = (int)strlen(fdata);

  log = red_black_tree_find(cache->files, fname);

  if (!log) {
    if (!log_cache_open(cache, fname)) {
      return false;
    }
    log = red_black_tree_find(cache->files, fname);
    if (!log) {
      return false;
    }
  }

  mux_timer_start(log->timer, (uint64_t)LOGFILE_TIMEOUT * 1000U, 0);

  if (write(log->fd, fdata, (size_t)len) < 0) {
    (void)fprintf(
        stderr,
        "System failed to write data to file with error '%s' on logfile "
        "'%s'. Closing.\n",
        system_error_message(errno, (char[SYSTEM_ERROR_MESSAGE_SIZE]){0},
                             SYSTEM_ERROR_MESSAGE_SIZE),
        log->filename);
    log_cache_close(cache, log, true);
  }
  return true;
}
