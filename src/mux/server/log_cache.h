/** @file
 * Cached arbitrary-log file management interface.
 */
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

/** Creates log cache. @param[in] loop Event loop. @param[in] log Server log. */

LogCache *log_cache_create(UvLoopT *loop, ServerLog *log);
/** Destroys log cache. @param[in,out] cache Cache. */

void log_cache_destroy(LogCache *cache);
/** Executes log cache list. @param[in,out] evaluation Expression evaluation
 * context. @param[in] cache Cache. @param[in] player Player object. */

void log_cache_list(EvaluationContext *evaluation, const LogCache *cache,
                    DbRef player);
/** Executes log cache write. @param[in,out] cache Cache. @param[in,out] fname
 * Fname. @param[in] data Caller-provided data. */

bool log_cache_write(LogCache *cache, char *fname, const char *data);
