#include "mux/server/event_timer.h"
#include "mux/server/libuv.h"

static int callback_count;

static void repeating_callback(MuxTimer *timer, void *data) {
  callback_count++;
  if (callback_count == 3)
    mux_timer_destroy(timer);
}

static void cancelled_callback(MuxTimer *timer, void *data) {
  callback_count++;
}

int main(void) {
  uv_loop_t loop;
  MuxTimer *timer;

  if (uv_loop_init(&loop) < 0)
    return 1;
  timer = mux_timer_create(&loop, repeating_callback, nullptr);
  if (timer == nullptr || !mux_timer_start(timer, 1, 1) ||
      !mux_timer_is_active(timer) || uv_run(&loop, UV_RUN_DEFAULT) != 0 ||
      callback_count != 3 || uv_loop_close(&loop) < 0)
    return 1;

  callback_count = 0;
  if (uv_loop_init(&loop) < 0)
    return 1;
  timer = mux_timer_create(&loop, cancelled_callback, nullptr);
  if (timer == nullptr || !mux_timer_start(timer, 100, 0) ||
      mux_timer_due_in(timer) > 100)
    return 1;
  mux_timer_stop(timer);
  if (mux_timer_is_active(timer))
    return 1;
  mux_timer_destroy(timer);
  if (uv_run(&loop, UV_RUN_DEFAULT) != 0 || callback_count != 0 ||
      uv_loop_close(&loop) < 0)
    return 1;
  return 0;
}
