/** @file
 * Heap-owned libuv timer used by server subsystems.
 */
#pragma once

#include <stdint.h>

// IWYU pragma: no_include "uv.h"

// libuv owns this external structure-tag spelling.
// NOLINTNEXTLINE(readability-identifier-naming)
typedef struct uv_loop_s UvLoopT;
typedef struct MuxTimer MuxTimer;
typedef void (*MuxTimerCallback)(MuxTimer *timer, void *data);

/** Creates mux timer. @param[in] loop Event loop. @param[in] callback Callback
 * to invoke. @param[in] data Caller-provided data. */

MuxTimer *mux_timer_create(UvLoopT *loop, MuxTimerCallback callback,
                           void *data);
/** Starts mux timer. @param[in,out] timer Timer instance. @param[in] timeout_ms
 * Timeout ms. @param[in] repeat_ms Repeat ms. */

[[nodiscard]] bool mux_timer_start(MuxTimer *timer, uint64_t timeout_ms,
                                   uint64_t repeat_ms);
/** Stops mux timer. @param[in,out] timer Timer instance. */

void mux_timer_stop(MuxTimer *timer);
/** Executes mux timer is active. @param[in] timer Timer instance. */

bool mux_timer_is_active(const MuxTimer *timer);
/** Executes mux timer due in. @param[in] timer Timer instance. */

uint64_t mux_timer_due_in(const MuxTimer *timer);
/** Destroys mux timer. @param[in,out] timer Timer instance. */

void mux_timer_destroy(MuxTimer *timer);
