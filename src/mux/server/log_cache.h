/* log_cache.h - Cached arbitrary-log file management interface. */

#pragma once

#include "mux/commands/command_context.h"
#include "mux/network/mux_event.h" // IWYU pragma: keep
#include "mux/server/event_timer.h"
#include "mux/server/log.h"
#include "mux/server/platform.h"

typedef struct uv_loop_s UvLoopT;
typedef struct LogCache LogCache;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerLog ServerLog;

LogCache *log_cache_create(UvLoopT *loop, ServerLog *log);
void log_cache_destroy(LogCache *cache);
void log_cache_list(EvaluationContext *evaluation, const LogCache *cache,
                    DbRef player);
int log_cache_write(LogCache *cache, char *fname, const char *data);
