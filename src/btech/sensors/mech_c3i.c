
/*
 * $Id: mech.c3i.c,v 1.1.1.1 2005/01/11 21:18:12 kstevens Exp $
 *
 * Author: Cord Awtry <kipsta@mediaone.net>
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <string.h>

#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "mech_c3_misc_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_network_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_state_types.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define C3_POS_IN_NETWORK -1
#define C3_POS_NO_ROOM -2

static bool mech_has_c3i(const Mech *mech) {
  return mech_technology_flags_secondary(mech) & C3I_TECH;
}

int mech_c3i_free_network_position(Mech *mech, Mech *mechToAdd) {
  int i;
  DbRef otherRef;

  mech_c3i_network_validate(mech);

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    otherRef = mech_c3i_network_node(mech, i);

    if (otherRef > 0) {
      if (otherRef == mech_dbref(mechToAdd))
        return C3_POS_IN_NETWORK;
    } else
      return i;
  }

  return C3_POS_NO_ROOM;
}

void mech_c3i_network_replicate(Mech *mechSrc, Mech *mechDest) {
  int i;
  DbRef otherRef;

  debugC3(mech_context(mechSrc),
          tprintf("REPLICATE: %ld's C3i network to %ld", mech_dbref(mechSrc),
                  mech_dbref(mechDest)));

  mech_c3i_network_clear(mechDest, 0);

  mech_c3i_network_node_set(mechDest, 0, mech_dbref(mechSrc));
  mech_c3i_network_size_set(mechDest, 1);

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    otherRef = mech_c3i_network_node(mechSrc, i);

    if (otherRef != mech_dbref(mechDest)) {
      const int destination_size = mech_c3i_network_size(mechDest);
      mech_c3i_network_node_set(mechDest, destination_size, otherRef);
      mech_c3i_network_size_set(mechDest, destination_size + 1);
    }
  }

  mech_c3i_network_validate(mechDest);
}

void mech_c3i_network_add(Mech *mech, Mech *mechToAdd) {
  Mech *otherMech;
  Mech *otherNotifyMech;
  DbRef otherRef;
  int i;
  int wPos = -1;

  debugC3(mech_context(mech), tprintf("ADD: %ld to the C3i network of %ld",
                                      mech_dbref(mechToAdd), mech_dbref(mech)));

  /* Find a position to add the new mech into my network */
  wPos = mech_c3i_free_network_position(mech, mechToAdd);

  /* If we have a number that's less than 0, then we have an invalid position.
   * Either we're already in the network or there's not enough room */
  if (wPos < 0)
    return;

  /* Well, we have a valid position, so let's put this mech in the network */
  mech_c3i_network_node_set(mech, wPos, mech_dbref(mechToAdd));
  mech_c3i_network_size_set(mech, mech_c3i_network_size(mech) + 1);

  mech_notify(mech, MECHALL,
              tprintf("%s connects to your C3i network.",
                      mech_to_mech_display_id(mech, mechToAdd).text));

  /* Now let's replicate the new network across the system so that everyone has
   * the same network settings */
  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    otherRef = mech_c3i_network_node(mech, i);

    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 0);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    if (otherRef != mech_dbref(mechToAdd)) {
      otherNotifyMech = getOtherMechInNetwork(mech, i, 1, 1, 1, 0);

      if (otherNotifyMech)
        mech_notify(
            otherNotifyMech, MECHALL,
            tprintf("%s connects to your C3i network.",
                    mech_to_mech_display_id(otherNotifyMech, mechToAdd).text));
    }

    mech_c3i_network_replicate(mech, otherMech);
  }

  /* Last, but not least, one final validation of the network */
  mech_c3i_network_validate(mech);
}

void mech_c3i_network_remove_reference(DbRef refToClear, Mech *mech) {
  int i;

  debugC3(mech_context(mech), tprintf("CLEAR: %ld from the C3i network of %ld",
                                      refToClear, mech_dbref(mech)));

  if (!mech_c3i_network_size(mech))
    return;

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    if (mech_c3i_network_node(mech, i) == refToClear)
      mech_c3i_network_node_set(mech, i, -1);
  }

  mech_c3i_network_validate(mech);
}

void mech_c3i_network_clear(Mech *mech, int tClearFromOthers) {
  Mech *otherMech;
  int i;

  debugC3(mech_context(mech),
          tprintf("CLEAR: %ld's C3i network", mech_dbref(mech)));

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 0);

    mech_c3i_network_node_set(mech, i, -1);

    if (tClearFromOthers) {
      if (!otherMech)
        continue;

      if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
        continue;

      mech_c3i_network_remove_reference(mech_dbref(mech), otherMech);
    }
  }

  mech_c3i_network_size_set(mech, 0);
}

void mech_c3i_network_validate(Mech *mech) {
  Mech *otherMech;
  DbRef myTempNetwork[C3I_NETWORK_SIZE];
  int i;
  int networkSize = 0;

  debugC3(mech_context(mech),
          tprintf("VALIDATE: %ld's C3i network", mech_dbref(mech)));

  if (!mech_has_c3i(mech) || mech_is_destroyed(mech) ||
      mech_condition_summary(mech).c3i_destroyed) {
    mech_c3i_network_clear(mech, 1);

    return;
  }

  if (mech_c3i_network_size(mech) < 0) {
    mech_c3i_network_clear(mech, 1);

    return;
  }

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 0);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    debugC3(mech_context(mech),
            tprintf("VALIDATE INFO: %ld is now in %ld's C3i network",
                    mech_dbref(otherMech), mech_dbref(mech)));

    myTempNetwork[networkSize++] = mech_dbref(otherMech);
  }

  mech_c3i_network_clear(mech, 0);

  for (i = 0; i < networkSize; i++)
    mech_c3i_network_node_set(mech, i, myTempNetwork[i]);

  mech_c3i_network_size_set(mech, networkSize);

  debugC3(mech_context(mech),
          tprintf("VALIDATE INFO: %ld's C3i network is %d elements",
                  mech_dbref(mech), mech_c3i_network_size(mech)));
}

void mech_c3i_join_leave(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[2];
  DbRef refTarget;
  int LOS = 1;
  float range = 0.0;

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech),
                  mech_parseattributes(buffer, args, 2) != 1,
                  "Invalid number of arguments to function!");

  DOCHECK_CONTEXT(mech_context(mech), !mech_has_c3i(mech),
                  "This unit is not equipped with C3i!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_condition_summary(mech).c3i_destroyed,
                  "Your C3i system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_any_ecm_disturbed(mech),
                  "Your C3i system is not currently operational!");

  mech_c3i_network_validate(mech);

  /* Clear our C3i Network */
  if (!strcmp(args[0], "-")) {
    if (mech_c3i_network_size(mech) <= 0) {
      mech_notify(mech, MECHALL, "You are not connected to a C3i network!");

      return;
    }

    mech_c3i_network_clear(mech, 1);

    mech_notify(mech, MECHALL, "You disconnect from the C3i network.");

    return;
  }

  /* Well, if we're here then we wanna connect to a network */
  /* Let's check to see if we're already in one... can't be in two at the same
   * time */
  DOCHECK_CONTEXT(mech_context(mech), mech_c3i_network_size(mech) > 0,
                  "You are already in a C3i network!");

  /* Find who we're trying to connect to */
  refTarget = FindTargetDBREFFromMapNumber(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), refTarget);

  if (target) {
    LOS = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), range);
  } else
    refTarget = 0;

  DOCHECK_CONTEXT(mech_context(mech), (refTarget < 1) || !LOS,
                  "That is not a valid targetID. Try again.");
  DOCHECK_CONTEXT(mech_context(mech), mech_team(mech) != mech_team(target),
                  "You can't use the C3i network of unfriendly units!");
  DOCHECK_CONTEXT(mech_context(mech), mech == target,
                  "You can't connect to yourself!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_destroyed(target),
                  "That unit is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), !mech_is_started(target),
                  "That unit is not started!");
  DOCHECK_CONTEXT(mech_context(mech), !mech_has_c3i(target),
                  "That unit does not appear to be equipped with C3i!");

  /* validate the network of our target */
  mech_c3i_network_validate(target);
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_c3i_network_size(target) >= C3I_NETWORK_SIZE,
                  "That unit's C3i network is operating at maximum capacity!");

  /* Connect us up */
  mech_notify(mech, MECHALL,
              tprintf("You connect to %s's C3i network.",
                      mech_to_mech_display_id(mech, target).text));

  mech_c3i_network_add(target, mech);
}

void mech_c3i_message(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !mech_has_c3i(mech),
                  "This unit is not equipped with C3i!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_condition_summary(mech).c3i_destroyed,
                  "Your C3i system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_any_ecm_disturbed(mech),
                  "Your C3i system is not currently operational!");

  mech_c3i_network_validate(mech);

  DOCHECK_CONTEXT(mech_context(mech), mech_c3i_network_size(mech) <= 0,
                  "There are no other units in your C3i network!");

  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer,
                  "What do you want to send on the C3i Network?");

  sendNetworkMessage(player, mech, buffer, 0);
}

void mech_c3i_targets(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !mech_has_c3i(mech),
                  "This unit is not equipped with C3i!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_condition_summary(mech).c3i_destroyed,
                  "Your C3i system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_any_ecm_disturbed(mech),
                  "Your C3i system is not currently operational!");

  mech_c3i_network_validate(mech);

  DOCHECK_CONTEXT(mech_context(mech), mech_c3i_network_size(mech) <= 0,
                  "There are no other units in your C3i network!");

  showNetworkTargets(player, mech, 0);
}

void mech_c3i_network(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !mech_has_c3i(mech),
                  "This unit is not equipped with C3i!");
  DOCHECK_CONTEXT(mech_context(mech),
                  mech_condition_summary(mech).c3i_destroyed,
                  "Your C3i system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), mech_is_any_ecm_disturbed(mech),
                  "Your C3i system is not currently operational!");

  mech_c3i_network_validate(mech);

  DOCHECK_CONTEXT(mech_context(mech), mech_c3i_network_size(mech) <= 0,
                  "There are no other units in your C3i network!");

  showNetworkData(player, mech, 0);
}
