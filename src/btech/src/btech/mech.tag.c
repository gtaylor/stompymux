/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001 Cord Awtry
 *       All rights reserved
 */

#include "mech.h"
#include "mech.events.h"
#include "p.mech.los.h"
#include "p.mech.tag.h"
#include "p.mech.utils.h"

#define TAGRECYCLE_TICK 30
#define TAG_SHORT 5
#define TAG_MED 10
#define TAG_LONG 15

static void tag_recycle_event(MuxEvent *e) {
  MECH *mech = (MECH *)e->data;
  long data = (long)e->data2;
  MECH *target;

  if (isTAGDestroyed(mech))
    return;

  if (data == 0) {
    mech_notify(mech, MECHALL, "%cgYour TAG system has finished recycling.%cn");
    return;
  }

  target = btech_context_get_mech(mech->xcode.context, TAGTarget(mech));

  if (!target)
    return;

  if (TaggedBy(target) != mech->mynum)
    return;

  mech_notify(mech, MECHALL,
              "%cgYour TAG system has achieved a stable lock.%cn");
}

void mech_tag(DbRef player, void *data, char *buffer) {
  MECH *mech = (MECH *)data, *target;
  char *args[2];
  DbRef refTarget;
  int LOS = 1;
  float range = 0.0;

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech->xcode.context, !HasTAG(mech),
                  "This unit is not equipped with TAG!");
  DOCHECK_CONTEXT(mech->xcode.context, isTAGDestroyed(mech),
                  "Your TAG system is destroyed!");
  DOCHECK_CONTEXT(mech->xcode.context, TagRecycling(mech),
                  "Your TAG system is recycling!");
  DOCHECK_CONTEXT(mech->xcode.context,
                  mech_parseattributes(buffer, args, 2) != 1,
                  "Invalid number of arguments to function!");

  /* Clear our TAG */
  if (!strcmp(args[0], "-")) {
    refTarget = TAGTarget(mech);

    DOCHECK_CONTEXT(mech->xcode.context, refTarget <= 0,
                    "You are not currently tagging anything!");

    stopTAG(mech);

    return;
  }

  /* TAG something... anything :) */
  refTarget = FindTargetDBREFFromMapNumber(mech, args[0]);
  target = btech_context_get_mech(mech->xcode.context, refTarget);

  if (target) {
    range = FaMechRange(mech, target);

    LOS = InLineOfSight_NB(mech, target, MechX(target), MechY(target), range);
  } else
    refTarget = 0;

  DOCHECK_CONTEXT(mech->xcode.context, refTarget < 1 || !LOS,
                  "That is not a valid TAG targetID. Try again.");
  DOCHECK_CONTEXT(mech->xcode.context, MechTeam(mech) == MechTeam(target),
                  "You can't TAG friendly units!");
  DOCHECK_CONTEXT(mech->xcode.context, mech == target,
                  "You can't TAG yourself!");
  DOCHECK_CONTEXT(mech->xcode.context, range > TAG_LONG,
                  tprintf("Out of range! TAG ranges are %d/%d/%d", TAG_SHORT,
                          TAG_MED, TAG_LONG));

  /*
   * This should actually make a roll...
   */

  /*
     if ( checkAllSections(mech,INARC_HAYWIRE_ATTACHED) )
     BTH += 1;
   */

  mech_printf(mech, MECHALL, "You light up %s with your TAG.",
              mech_to_mech_display_id(mech, target).text);

  TaggedBy(target) = mech->mynum;
  TAGTarget(mech) = target->mynum;

  MECHEVENT(mech, EVENT_TAG_RECYCLE, tag_recycle_event, TAGRECYCLE_TICK, 1);
}

int isTAGDestroyed(MECH *mech) {
  if ((MechSpecials2(mech) & TAG_TECH) &&
      (MechCritStatus(mech) & TAG_DESTROYED))
    return 1;

  if (HasC3m(mech) && (MechCritStatus(mech) & C3_DESTROYED))
    return 1;

  return 0;
}

void stopTAG(MECH *mech) {
  MECH *target;

  target = btech_context_get_mech(mech->xcode.context, TAGTarget(mech));

  if (target)
    if (TaggedBy(target) == mech->mynum)
      TaggedBy(target) = 0;

  if (TAGTarget(mech) > 0) {
    TAGTarget(mech) = 0;

    mech_notify(mech, MECHALL, "Your TAG connection has been broken.");

    MECHEVENT(mech, EVENT_TAG_RECYCLE, tag_recycle_event, TAGRECYCLE_TICK, 0);
  }
}

void checkTAG(MECH *mech) {
  MECH *target;
  DbRef refTarget;
  float range;
  int LOS = 1;

  refTarget = TAGTarget(mech);

  if (refTarget <= 0)
    return;

  target = btech_context_get_mech(mech->xcode.context, refTarget);

  if (!target) {
    stopTAG(mech);
    return;
  }

  if (TaggedBy(target) != mech->mynum) {
    stopTAG(mech);
    return;
  }

  range = FlMechRange(
      btech_context_get_map(mech->xcode.context, mech->mapindex), mech, target);
  LOS = InLineOfSight_NB(mech, target, MechX(target), MechY(target), range);

  if (!LOS || (range > TAG_LONG)) {
    stopTAG(mech);
    return;
  }
}
