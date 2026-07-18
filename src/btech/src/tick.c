/*
 *
 * Copyright (c) 2005 Martin Murray
 *
 * This is much better than what we had.
 *
 */

#include "mux/server/platform.h"

#include "autopilot.h"
#include "debug.h"
#include "glue_types.h"
#include "mech.h"
#include "mux/server/event_timer.h"
#include "mux/server/server_config.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/red_black_tree.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <sys/types.h>
#include <unistd.h>

static MuxTimer *heartbeat_ev;
static int heartbeat_running = 0;
unsigned int global_tick = 0;
extern RedBlackTree xcode_tree;

static void heartbeat_run(MuxTimer *timer, void *arg);

void heartbeat_init() {
  if (heartbeat_running)
    return;
  dprintk("hearbeat initialized, 1s timeout.");
  heartbeat_ev =
      mux_timer_create(server_lifecycle_loop(btech_context_active()->lifecycle),
                       heartbeat_run, nullptr);
  if (heartbeat_ev == nullptr)
    return;
  mux_timer_start(heartbeat_ev, 1000, 1000);
  heartbeat_running = 1;
}

void heartbeat_stop() {
  if (!heartbeat_running)
    return;
  mux_timer_destroy(heartbeat_ev);
  heartbeat_ev = nullptr;
  dprintk("heartbeat stopped.\n");
  heartbeat_running = 0;
}

void mech_heartbeat(MECH *);
void auto_heartbeat(AUTO *);

static int heartbeat_dispatch(void *key, void *data, int depth, void *arg) {
  XCODE *const xcode_obj = data;

  switch (xcode_obj->type) {
  case GTYPE_MECH:
    mech_heartbeat((MECH *)xcode_obj);
    break;

  case GTYPE_AUTO:
    auto_heartbeat((AUTO *)xcode_obj);
    break;

  default:
    break;
  }

  return 1;
}

static void heartbeat_run(MuxTimer *timer, void *arg) {
  red_black_tree_walk(xcode_tree, WALK_INORDER, heartbeat_dispatch, NULL);
  global_tick++;
}
