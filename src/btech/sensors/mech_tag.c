/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001 Cord Awtry
 *       All rights reserved
 */

#include <string.h>

#include "btech_event.h"
#include "command_handlers_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_sensor_state_api.h"
#include "mech_status_types.h"
#include "mech_tag_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define TAGRECYCLE_TICK 30
#define TAG_SHORT 5
#define TAG_MED 10
#define TAG_LONG 15

static void tag_recycle_event(MuxEvent *e) {
  Mech *mech = (Mech *)e->data;
  long data = (long)e->data2;
  Mech *target;

  if (mech_tag_is_destroyed(mech))
    return;

  if (data == 0) {
    mech_notify(mech, MECHALL,
                "[fg=green]Your TAG system has finished recycling.[reset]");
    return;
  }

  target =
      btech_context_get_mech(mech_context(mech), mech_tag_target_dbref(mech));

  if (!target)
    return;

  if (mech_tagged_by_dbref(target) != mech_dbref(mech))
    return;

  mech_notify(mech, MECHALL,
              "[fg=green]Your TAG system has achieved a stable lock.[reset]");
}

void mech_tag(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[2];
  DbRef refTarget;
  int LOS = 1;
  float range = 0.0;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_tag_system(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with TAG!");
    return;
  }
  if (mech_tag_is_destroyed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your TAG system is destroyed!");
    return;
  }
  if (mech_event_count(mech, EVENT_TAG_RECYCLE)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your TAG system is recycling!");
    return;
  }
  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to function!");
    return;
  }

  /* Clear our TAG */
  if (!strcmp(args[0], "-")) {
    refTarget = mech_tag_target_dbref(mech);

    if (refTarget <= 0) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You are not currently tagging anything!");
      return;
    }

    mech_tag_stop(mech);

    return;
  }

  /* TAG something... anything :) */
  refTarget = FindTargetDBREFFromMapNumber(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), refTarget);

  if (target) {
    range = mech_range_to(mech, target);

    LOS = mech_los_check_unblocked(mech, target, mech_position_x(target),
                                   mech_position_y(target), range);
  } else
    refTarget = 0;

  if (refTarget < 1 || !LOS) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That is not a valid TAG targetID. Try again.");
    return;
  }
  if (mech_team(mech) == mech_team(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't TAG friendly units!");
    return;
  }
  if (mech == target) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't TAG yourself!");
    return;
  }
  if (range > TAG_LONG) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 tprintf("Out of range! TAG ranges are %d/%d/%d", TAG_SHORT,
                         TAG_MED, TAG_LONG));
    return;
  }

  /*
   * This should actually make a roll...
   */

  /*
     if ( checkAllSections(mech,INARC_HAYWIRE_ATTACHED) )
     BTH += 1;
   */

  mech_printf(mech, MECHALL, "You light up %s with your TAG.",
              mech_to_mech_display_id(mech, target).text);

  mech_tagged_by_dbref_set(target, mech_dbref(mech));
  mech_tag_target_dbref_set(mech, mech_dbref(target));

  mech_event_schedule(mech, EVENT_TAG_RECYCLE, tag_recycle_event,
                      TAGRECYCLE_TICK, 1);
}

bool mech_tag_is_destroyed(const Mech *mech) {
  return mech_tag_system_is_destroyed(mech);
}

void mech_tag_stop(Mech *mech) {
  Mech *target;

  target =
      btech_context_get_mech(mech_context(mech), mech_tag_target_dbref(mech));

  if (target)
    if (mech_tagged_by_dbref(target) == mech_dbref(mech))
      mech_tagged_by_dbref_set(target, 0);

  if (mech_tag_target_dbref(mech) > 0) {
    mech_tag_target_dbref_set(mech, 0);

    mech_notify(mech, MECHALL, "Your TAG connection has been broken.");

    mech_event_schedule(mech, EVENT_TAG_RECYCLE, tag_recycle_event,
                        TAGRECYCLE_TICK, 0);
  }
}

void mech_tag_check(Mech *mech) {
  Mech *target;
  DbRef refTarget;
  float range;
  int LOS = 1;

  refTarget = mech_tag_target_dbref(mech);

  if (refTarget <= 0)
    return;

  target = btech_context_get_mech(mech_context(mech), refTarget);

  if (!target) {
    mech_tag_stop(mech);
    return;
  }

  if (mech_tagged_by_dbref(target) != mech_dbref(mech)) {
    mech_tag_stop(mech);
    return;
  }

  range = mech_range_to(mech, target);
  LOS = mech_los_check_unblocked(mech, target, mech_position_x(target),
                                 mech_position_y(target), range);

  if (!LOS || (range > TAG_LONG)) {
    mech_tag_stop(mech);
    return;
  }
}
