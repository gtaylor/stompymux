/*
 *
 * Copyright (c) 2005 Martin Murray
 *
 * This is much better than what we had.
 *
 */

#include "autopilot.h"
#include "btech/context.h"
#include "btech/lifecycle.h"
#include "context_internal.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_update_api.h"
#include "mux/server/diagnostics.h"
#include "mux/server/event_timer.h"
#include "mux/server/maintenance.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/red_black_tree.h"
#include "special_object.h"

static void heartbeat_run(MuxTimer *timer, void *arg);

void btech_heartbeat_start(BtechContext *context) {
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

void btech_heartbeat_stop(BtechContext *context) {
  if (!context->heartbeat_running)
    return;
  mux_timer_destroy(context->heartbeat);
  context->heartbeat = nullptr;
  dprintk("heartbeat stopped.\n");
  context->heartbeat_running = false;
}

void auto_heartbeat(Autopilot *);

static int heartbeat_dispatch(void *key, void *data, int depth, void *arg) {
  BtechSpecialObject *const xcode_obj = data;

  switch (xcode_obj->type) {
  case GTYPE_MECH:
    mech_update(mech_dbref((Mech *)xcode_obj), xcode_obj);
    break;

  case GTYPE_AUTO:
    auto_heartbeat((Autopilot *)xcode_obj);
    break;

  case GTYPE_DEBUG:
  case GTYPE_MECHREP:
  case GTYPE_MAP:
  case GTYPE_TURRET:
  case GTYPE_UNUSED1:

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
