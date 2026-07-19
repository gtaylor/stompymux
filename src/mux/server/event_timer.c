/* Heap-owned libuv timer used by server subsystems. */

#include "mux/server/platform.h"

#include "mux/server/libuv.h"

#include "mux/server/event_timer.h"

struct MuxTimer {
  uv_timer_t handle;
  MuxTimerCallback callback;
  void *data;
};

static void mux_timer_run(uv_timer_t *handle) {
  MuxTimer *timer = handle->data;

  timer->callback(timer, timer->data);
}

static void mux_timer_free(uv_handle_t *handle) { free(handle->data); }

MuxTimer *mux_timer_create(uv_loop_t *loop, MuxTimerCallback callback,
                           void *data) {
  MuxTimer *timer = calloc(1, sizeof(*timer));

  if (timer == nullptr)
    return nullptr;
  if (uv_timer_init(loop, &timer->handle) < 0) {
    free(timer);
    return nullptr;
  }
  timer->handle.data = timer;
  timer->callback = callback;
  timer->data = data;
  return timer;
}

bool mux_timer_start(MuxTimer *timer, uint64_t timeout_ms, uint64_t repeat_ms) {
  return uv_timer_start(&timer->handle, mux_timer_run, timeout_ms, repeat_ms) ==
         0;
}

void mux_timer_stop(MuxTimer *timer) {
  if (timer != nullptr && !uv_is_closing((uv_handle_t *)&timer->handle))
    uv_timer_stop(&timer->handle);
}

bool mux_timer_is_active(const MuxTimer *timer) {
  return timer != nullptr &&
         uv_is_active((const uv_handle_t *)&timer->handle) != 0;
}

uint64_t mux_timer_due_in(const MuxTimer *timer) {
  if (timer == nullptr || uv_is_closing((const uv_handle_t *)&timer->handle))
    return 0;
  return uv_timer_get_due_in(&timer->handle);
}

void mux_timer_destroy(MuxTimer *timer) {
  if (timer == nullptr || uv_is_closing((uv_handle_t *)&timer->handle))
    return;
  uv_timer_stop(&timer->handle);
  timer->data = nullptr;
  uv_close((uv_handle_t *)&timer->handle, mux_timer_free);
}
