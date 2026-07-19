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

static void heartbeat_run(MuxTimer *timer, void *arg);

void heartbeat_init(BtechContext *context) {
  if (context->heartbeat_running)
    return;
  dprintk("hearbeat initialized, 1s timeout.");
  context->heartbeat = mux_timer_create(
      server_lifecycle_loop(context->lifecycle), heartbeat_run, context);
  if (context->heartbeat == nullptr)
    return;
  mux_timer_start(context->heartbeat, 1000, 1000);
  context->heartbeat_running = true;
}

void heartbeat_stop(BtechContext *context) {
  if (!context->heartbeat_running)
    return;
  mux_timer_destroy(context->heartbeat);
  context->heartbeat = nullptr;
  dprintk("heartbeat stopped.\n");
  context->heartbeat_running = false;
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
  BtechContext *context = arg;

  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      heartbeat_dispatch, context);
  context->tick++;
}
