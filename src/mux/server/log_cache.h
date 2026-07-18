/* log_cache.h - Cached arbitrary-log file management interface. */

#pragma once

#include "mux/database/db.h"

typedef struct uv_loop_s uv_loop_t;
typedef struct LogCache LogCache;
typedef struct EvaluationContext EvaluationContext;
typedef struct ServerLog ServerLog;

LogCache *log_cache_create(uv_loop_t *loop, ServerLog *log);
void log_cache_destroy(LogCache *cache);
void log_cache_list(EvaluationContext *evaluation, const LogCache *cache,
                    DbRef player);
int log_cache_write(LogCache *cache, char *filename, const char *data);
