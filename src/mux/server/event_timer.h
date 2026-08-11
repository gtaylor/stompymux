/* Heap-owned libuv timer used by server subsystems. */

#pragma once

#include <stdint.h>

// IWYU pragma: no_include "uv.h"

typedef struct uv_loop_s UvLoopT;
typedef struct MuxTimer MuxTimer;
typedef void (*MuxTimerCallback)(MuxTimer *timer, void *data);

MuxTimer *mux_timer_create(UvLoopT *loop, MuxTimerCallback callback,
                           void *data);
bool mux_timer_start(MuxTimer *timer, uint64_t timeout_ms, uint64_t repeat_ms);
void mux_timer_stop(MuxTimer *timer);
bool mux_timer_is_active(const MuxTimer *timer);
uint64_t mux_timer_due_in(const MuxTimer *timer);
void mux_timer_destroy(MuxTimer *timer);
