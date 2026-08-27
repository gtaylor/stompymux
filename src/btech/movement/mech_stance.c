/* Implements BattleTech movement mechanics for unit stance. */

#include <math.h>
#include <string.h>
#include <strings.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_condition_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

static int mech_hull_down_change_delay(const Mech *mech) {
  const float SPEED_FACTOR = mech_maximum_speed(mech) / MP2;
  const float BOUNDED_FACTOR = fminf(fmaxf(1.0F, SPEED_FACTOR), 30.0F);
  const float DELAY = 30.0F / BOUNDED_FACTOR;
  return (int)DELAY;
}
static void mech_hulldown_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long type = (long)e->secondary.integer;

  if (!mech_event_count(mech, EVENT_CHANGING_HULLDOWN))
    return;

  if (!mech_is_started(mech))
    return;

  if (type == 0) {
    mech_hull_down_set(mech, false);
    mech_notify(mech, MECHALL, "You finish lifting yourself up.");
    mech_los_broadcast(mech, "finishes lifting itself up");
  } else {
    mech_hull_down_set(mech, true);
    mech_notify(mech, MECHALL, "You finish lowering yourself to the ground.");
    mech_los_broadcast(mech, "finishes lowering itself to the ground.");
  }
}

void mech_hulldown(DbRef player, Mech *mech, char *buffer) {
  char *args[1];
  int argc;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "While falling out of the sky?");
    return;
  }
  if (mech_movement_type(mech) != MOVE_QUAD) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Only QUADs can hulldown.");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown from a FALLEN position");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown while jumping!");
    return;
  }
  if (mech_current_speed(mech) > 0.5F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't hulldown while moving!");
    return;
  }
  if (mech_event_count(mech, EVENT_JUMPSTABIL)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are still stabilizing from your last jump.");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You haven't finished standing up yet.");
    return;
  }

  argc = mech_parseattributes(buffer, args, 1);

  if (argc > 0) {
    if (!strcmp(args[0], "-")) {
      if (!condition.hull_down) {
        mech_notify(mech, MECHALL, "You are not hulldown.");
      } else if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
        mech_notify(mech, MECHALL, "You are busy changing your hulldown mode.");
      } else {
        mech_notify(mech, MECHALL, "You start to lift yourself up.");
        mech_los_broadcast(mech, "begins to raise up on its legs.");

        mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                            mech_hull_down_change_delay(mech), 0);
      }
    } else if (!strcasecmp(args[0], "stop")) {
      if (!mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
        mech_notify(mech, MECHALL,
                    "You are not currently changing your hulldown mode.");
      } else {
        mech_event_cancel(mech, EVENT_CHANGING_HULLDOWN);
        mech_notify(mech, MECHALL, "You stop changing your hulldown mode.");
      }
    } else {
      mech_notify(mech, MECHALL, "Invalid argument for 'hulldown'.");
    }

    return;
  }

  if (condition.hull_down) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already hulldown.");
    return;
  }
  if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are busy changing your hulldown mode.");
    return;
  }

  mech_notify(mech, MECHALL, "You start to lower yourself to the ground.");
  mech_los_broadcast(mech, "begins to lower itself to the ground.");
  mech_desired_speed_set(mech, 0.0F);

  mech_event_schedule(mech, EVENT_CHANGING_HULLDOWN, mech_hulldown_event,
                      mech_hull_down_change_delay(mech), 1);
}
