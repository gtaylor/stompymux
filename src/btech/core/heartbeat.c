/* Coordinates the BattleTech simulation heartbeat. */

#include "autopilot.h"
#include "btech/context.h"
#include "btech/lifecycle.h"
#include "context_internal.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_update_api.h"
#include "mux/server/event_timer.h"
#include "mux/server/maintenance.h"
#include "mux/server/server_lifecycle.h"
#include "mux/support/red_black_tree.h"
#include "special_object.h"

static void heartbeat_run(MuxTimer *timer, void *arg);

void btech_heartbeat_start(BtechContext *context) {
  if (context->heartbeat_running)
    return;
  context->heartbeat = mux_timer_create(
      server_lifecycle_loop(context->lifecycle), heartbeat_run, context);
  if (context->heartbeat == nullptr)
    return;
  if (!mux_timer_start(context->heartbeat, 1000, 1000)) {
    mux_timer_destroy(context->heartbeat);
    context->heartbeat = nullptr;
    return;
  }
  context->heartbeat_running = true;
}

void btech_heartbeat_stop(BtechContext *context) {
  if (!context->heartbeat_running)
    return;
  mux_timer_destroy(context->heartbeat);
  context->heartbeat = nullptr;
  context->heartbeat_running = false;
}

static bool heartbeat_dispatch(const RedBlackTreeVisitCall *call) {
  void *data = call->data;
  BtechSpecialObject *const XCODE_OBJ = data;

  switch (XCODE_OBJ->type) {
  case GTYPE_MECH:
    mech_update(mech_dbref((Mech *)XCODE_OBJ), XCODE_OBJ);
    break;

  case GTYPE_AUTO:
    auto_heartbeat((Autopilot *)XCODE_OBJ);
    break;

  case GTYPE_DEBUG:
  case GTYPE_MECHREP:
  case GTYPE_MAP:
  case GTYPE_TURRET:

  default:
    break;
  }

  return true;
}

static void heartbeat_run(MuxTimer *timer [[maybe_unused]], void *arg) {
  BtechContext *context = arg;

  red_black_tree_walk(context->special_objects, WALK_INORDER,
                      heartbeat_dispatch, context);
  context->tick++;
}
