
/*
 * $Id: mech.c3.c,v 1.1.1.1 2005/01/11 21:18:11 kstevens Exp $
 *
 * Author: Cord Awtry <kipsta@mediaone.net>
 *
 *  Copyright (c) 2001 Cord Awtry
 *       All rights reserved
 */

#include <string.h>

#include "command_handlers_api.h"
#include "legacy_macros.h"
#include "mech.h"
#include "mech_c3_api.h"
#include "mech_c3_misc_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define C3_POS_IN_NETWORK -1
#define C3_POS_NO_ROOM -2

#define C3_MASTER_MECH_SIZE 5
#define C3_MASTER_OTHER_SIZE 1

int getC3MasterSize(Mech *mech) {
  if (MechType(mech) == CLASS_MECH)
    return C3_MASTER_MECH_SIZE;
  else
    return C3_MASTER_OTHER_SIZE;
}

int isPartOfWorkingC3Master(Mech *mech, int section, int slot) {
  int x = 0;
  int y, t;
  int wcWorkingSlots = 0;
  int wStartCheck = 0;
  int tDoBump;

  wStartCheck = MAX(0, slot - (getC3MasterSize(mech) - 1));

  while (x < CritsInLoc(mech, section)) {
    tDoBump = 0;

    if ((t = GetPartType(mech, section, x))) {
      if (Special2I(t) == C3_MASTER) {
        if (x < wStartCheck) {
          tDoBump = 1;
        } else {
          /* We're within range of our slot, if not already on it */
          for (y = x; y < (x + getC3MasterSize(mech)); y++) {
            if (y != slot) {
              if (!PartIsNonfunctional(mech, section, y))
                wcWorkingSlots++;
            }
          }
        }
      }
    }

    if (tDoBump)
      x += getC3MasterSize(mech);
    else
      x++;
  }

  return (wcWorkingSlots == (getC3MasterSize(mech) - 1));
}

int countWorkingC3MastersOnMech(Mech *mech) {
  int x, y, t;
  int wcSlots;
  int wcWorkingSlots;
  int wcMasters = 0;

  debugC3(mech_context(mech),
          tprintf("Counting working C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wcSlots = 0;
    wcWorkingSlots = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++) {
      if ((t = GetPartType(mech, x, y))) {
        if (Special2I(t) == C3_MASTER) {
          debugC3(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wcSlots++;

          if (!PartIsNonfunctional(mech, x, y)) {
            debugC3(mech_context(mech), "......and the slot is functional.");
            wcWorkingSlots++;
          }
        }
      }

      if (wcSlots == getC3MasterSize(mech)) {
        debugC3(mech_context(mech),
                tprintf("...found enough slots for a C3Master for %ld.",
                        mech_dbref(mech)));
        wcSlots = 0;

        if (wcWorkingSlots == getC3MasterSize(mech)) {
          debugC3(mech_context(mech),
                  tprintf("...there is even enough working slots to make the "
                          "computer work on %ld.",
                          mech_dbref(mech)));
          wcMasters++;
        }
      }
    }
  }

  debugC3(mech_context(mech), tprintf("Found %d working C3 masters on %ld",
                                      wcMasters, mech_dbref(mech)));

  return wcMasters;
}

int countTotalC3MastersOnMech(Mech *mech) {
  int x, y, t;
  int wcSlots;
  int wcMasters = 0;

  debugC3(mech_context(mech),
          tprintf("Counting total C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wcSlots = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++) {
      if ((t = GetPartType(mech, x, y))) {
        if (Special2I(t) == C3_MASTER) {
          debugC3(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wcSlots++;
        }
      }

      if (wcSlots == getC3MasterSize(mech)) {
        debugC3(mech_context(mech),
                tprintf("...found enough slots for a C3Master for %ld.",
                        mech_dbref(mech)));

        wcSlots = 0;
        wcMasters++;
      }
    }
  }

  debugC3(mech_context(mech), tprintf("Found %d total C3 masters on %ld",
                                      wcMasters, mech_dbref(mech)));

  return wcMasters;
}

int countMaxC3Units(Mech *mech, DbRef *myTempNetwork, int tempNetworkSize,
                    Mech *targMech) {
  DbRef otherRef;
  Mech *otherMech;
  int i;
  int wcC3Masters = 0;
  int myMasters = 0;
  int maxC3Size;

  debugC3(mech_context(mech),
          tprintf("Counting max C3 units in %ld's network", mech_dbref(mech)));

  if (targMech)
    debugC3(mech_context(mech), tprintf("...using %ld as an additional mech",
                                        mech_dbref(targMech)));

  /* First we iterate over the list and find all the masters */
  for (i = 0; i < tempNetworkSize; i++) {
    otherRef = myTempNetwork[i];
    otherMech = btech_context_get_mech(mech_context(mech), otherRef);

    if (!otherMech)
      continue;

    wcC3Masters += MechWorkingC3Masters(otherMech);

    debugC3(mech_context(mech),
            tprintf("...for %ld, we add %d masters", mech_dbref(otherMech),
                    MechWorkingC3Masters(otherMech)));
  }

  /* Let's find out the max number of mechs in this network. Make sure we add in
   * any slaves we can control */
  maxC3Size = (wcC3Masters * 4) - wcC3Masters;

  debugC3(mech_context(mech),
          tprintf("...we now have a max size of %d", maxC3Size));

  myMasters = MechWorkingC3Masters(mech);

  if (myMasters > 0)
    maxC3Size += (myMasters * 4) - myMasters;

  debugC3(
      mech_context(mech),
      tprintf("...and after adding in my masters, we now have a max size of %d",
              maxC3Size));

  /* Let's see if a 2nd mech has been supplied to us */
  if (targMech) {
    myMasters = MechWorkingC3Masters(targMech);

    if (myMasters > 0)
      maxC3Size += (myMasters * 4) - myMasters;
  }

  maxC3Size = MIN(maxC3Size, 11);

  debugC3(mech_context(mech), tprintf("...final max size of %d", maxC3Size));

  return maxC3Size;
}

int trimC3Network(Mech *mech, DbRef *myTempNetwork, int tempNetworkSize) {
  DbRef otherRef;
  Mech *otherMech;
  int i;
  int newNetworkSize;
  int maxC3Size = 0; /* This is calc'd based on the number of masters */
  DbRef newNetwork[C3_NETWORK_SIZE];

  debugC3(mech_context(mech),
          tprintf("C3 TRIM: Trimming %ld's C3 network", mech_dbref(mech)));

  /* Initialize our data */
  newNetworkSize = tempNetworkSize;

  for (i = 0; i < C3_NETWORK_SIZE; i++)
    newNetwork[i] = -1;

  /* Get our count of max units */
  maxC3Size = countMaxC3Units(mech, myTempNetwork, tempNetworkSize, NULL);

  debugC3(mech_context(mech), tprintf("C3 TRIM: Max C3 size: %d", maxC3Size));
  debugC3(mech_context(mech),
          tprintf("C3 TRIM: Current C3 size: %d", tempNetworkSize));

  /* Now we see if our network is oversized */
  if (maxC3Size < tempNetworkSize) {
    newNetworkSize = 0;

    /* First put our masters in */
    for (i = 0; i < tempNetworkSize; i++) {
      otherRef = myTempNetwork[i];
      otherMech = btech_context_get_mech(mech_context(mech), otherRef);

      if (!otherMech)
        continue;

      if (MechWorkingC3Masters(otherMech) > 0)
        newNetwork[newNetworkSize++] = otherRef;
    }

    /* Next we put in slaves up to the max amount */
    if (newNetworkSize < maxC3Size) {
      for (i = 0; i < tempNetworkSize; i++) {
        otherRef = myTempNetwork[i];
        otherMech = btech_context_get_mech(mech_context(mech), otherRef);

        if (!otherMech)
          continue;

        if (MechWorkingC3Masters(otherMech) == 0)
          newNetwork[newNetworkSize++] = otherRef;

        if (newNetworkSize >= maxC3Size)
          break;
      }
    }

    /* Now, refill our other temp network */
    for (i = 0; i < newNetworkSize; i++)
      myTempNetwork[i] = newNetwork[i];
  }

  return newNetworkSize;
}

int getFreeC3NetworkPos(Mech *mech, Mech *mechToAdd) {
  int i;
  DbRef otherRef;

  validateC3Network(mech);

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = MechC3NetworkElem(mech, i);

    if (otherRef > 0) {
      if (otherRef == mech_dbref(mechToAdd))
        return C3_POS_IN_NETWORK;
    } else
      return i;
  }

  return C3_POS_NO_ROOM;
}

void replicateC3Network(Mech *mechSrc, Mech *mechDest) {
  int i;
  DbRef otherRef;

  debugC3(mech_context(mechSrc),
          tprintf("C3 REPLICATE: %ld's C3 network to %ld", mech_dbref(mechSrc),
                  mech_dbref(mechDest)));

  clearC3Network(mechDest, 0);

  MechC3NetworkElem(mechDest, 0) = mech_dbref(mechSrc);
  MechC3NetworkSize(mechDest) = 1;

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = MechC3NetworkElem(mechSrc, i);

    if (otherRef != mech_dbref(mechDest)) {
      MechC3NetworkElem(mechDest, MechC3NetworkSize(mechDest)) = otherRef;
      MechC3NetworkSize(mechDest) += 1;
    }
  }

  validateC3Network(mechDest);
}

void addMechToC3Network(Mech *mech, Mech *mechToAdd) {
  Mech *otherMech;
  Mech *otherNotifyMech;
  DbRef otherRef;
  int i;
  int wPos = -1;

  debugC3(mech_context(mech), tprintf("C3 ADD: %ld to the C3 network of %ld",
                                      mech_dbref(mechToAdd), mech_dbref(mech)));

  /* Find a position to add the new mech into my network */
  wPos = getFreeC3NetworkPos(mech, mechToAdd);

  /* If we have a number that's less than 0, then we have an invalid position.
   * Either we're already in the network or there's not enough room */
  if (wPos < 0)
    return;

  /* Well, we have a valid position, so let's put this mech in the network */
  debugC3(mech_context(mech),
          tprintf("C3 ADD: Position to add to %ld's network is %d",
                  mech_dbref(mech), wPos));

  MechC3NetworkElem(mech, wPos) = mech_dbref(mechToAdd);
  MechC3NetworkSize(mech) += 1;

  mech_notify(mech, MECHALL,
              tprintf("%s connects to your C3 network.",
                      mech_to_mech_display_id(mech, mechToAdd).text));

  /* Now let's replicate the new network across the system so that everyone has
   * the same network settings */
  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = MechC3NetworkElem(mech, i);

    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 1);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    if (otherRef != mech_dbref(mechToAdd)) {
      otherNotifyMech = getOtherMechInNetwork(mech, i, 1, 1, 1, 1);

      if (otherNotifyMech)
        mech_notify(
            otherNotifyMech, MECHALL,
            tprintf("%s connects to your C3 network.",
                    mech_to_mech_display_id(otherNotifyMech, mechToAdd).text));
    }

    replicateC3Network(mech, otherMech);
  }

  /* Last, but not least, one final validation of the network */
  validateC3Network(mech);
}

void clearMechFromC3Network(DbRef refToClear, Mech *mech) {
  int i;

  debugC3(mech_context(mech),
          tprintf("C3 CLEAR: %ld from the C3 network of %ld", refToClear,
                  mech_dbref(mech)));

  if (!MechC3NetworkSize(mech))
    return;

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    if (MechC3NetworkElem(mech, i) == refToClear)
      MechC3NetworkElem(mech, i) = -1;
  }

  validateC3Network(mech);
}

void clearC3Network(Mech *mech, int tClearFromOthers) {
  Mech *otherMech;
  int i;

  debugC3(mech_context(mech),
          tprintf("C3 CLEAR: %ld's C3 network", mech_dbref(mech)));

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 1);

    MechC3NetworkElem(mech, i) = -1;

    if (tClearFromOthers) {
      if (!otherMech)
        continue;

      if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
        continue;

      clearMechFromC3Network(mech_dbref(mech), otherMech);
    }
  }

  MechC3NetworkSize(mech) = 0;
}

void validateC3Network(Mech *mech) {
  Mech *otherMech;
  DbRef myTempNetwork[C3_NETWORK_SIZE];
  int i;
  int networkSize = 0;

  debugC3(mech_context(mech),
          tprintf("C3 VALIDATE: %ld's C3 network", mech_dbref(mech)));

  if (!HasC3(mech) || Destroyed(mech) || C3Destroyed(mech)) {
    clearC3Network(mech, 1);

    return;
  }

  if (MechC3NetworkSize(mech) < 0) {
    clearC3Network(mech, 1);

    return;
  }

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherMech = getOtherMechInNetwork(mech, i, 0, 0, 0, 1);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    debugC3(mech_context(mech),
            tprintf("C3 VALIDATE INFO: %ld is now in %ld's C3 network",
                    mech_dbref(otherMech), mech_dbref(mech)));

    myTempNetwork[networkSize] = mech_dbref(otherMech);
    networkSize++;
  }

  clearC3Network(mech, 0);

  for (i = 0; i < networkSize; i++)
    MechC3NetworkElem(mech, i) = myTempNetwork[i];

  MechC3NetworkSize(mech) = networkSize;

  debugC3(mech_context(mech),
          tprintf("C3 VALIDATE INFO: (PreTrim) %ld's C3 network is %d elements",
                  mech_dbref(mech), MechC3NetworkSize(mech)));

  networkSize = trimC3Network(mech, myTempNetwork, networkSize);

  debugC3(mech_context(mech),
          tprintf("C3 VALIDATE INFO: (PostTrim) %ld's C3 network has been "
                  "trimmed to %d elements",
                  mech_dbref(mech), networkSize));

  if (networkSize != MechC3NetworkSize(mech)) {
    clearC3Network(mech, 0);

    for (i = 0; i < networkSize; i++)
      MechC3NetworkElem(mech, i) = myTempNetwork[i];

    MechC3NetworkSize(mech) = networkSize;
  }
}

void mech_c3_join_leave(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[2];
  DbRef refTarget;
  int LOS = 1;
  float range = 0.0;
  int maxC3Size = 0;

  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech),
                  mech_parseattributes(buffer, args, 2) != 1,
                  "Invalid number of arguments to function!");

  DOCHECK_CONTEXT(mech_context(mech), !HasC3(mech),
                  "This unit is not equipped with C3!");
  DOCHECK_CONTEXT(mech_context(mech), C3Destroyed(mech),
                  "Your C3 system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), AnyECMDisturbed(mech),
                  "Your C3 system is not currently operational!");

  validateC3Network(mech);

  /* Clear our C3 Network */
  if (!strcmp(args[0], "-")) {
    if (MechC3NetworkSize(mech) <= 0) {
      mech_notify(mech, MECHALL, "You are not connected to a C3 network!");

      return;
    }

    clearC3Network(mech, 1);

    mech_notify(mech, MECHALL, "You disconnect from the C3 network.");

    return;
  }

  /* Well, if we're here then we wanna connect to a network */
  /* Let's check to see if we're already in one... can't be in two at the same
   * time */
  DOCHECK_CONTEXT(mech_context(mech), MechC3NetworkSize(mech) > 0,
                  "You are already in a C3 network!");

  /* Find who we're trying to connect to */
  refTarget = FindTargetDBREFFromMapNumber(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), refTarget);

  if (target)
    LOS = mech_los_check(mech, target, MechX(target), MechY(target), range);
  else
    refTarget = 0;

  DOCHECK_CONTEXT(mech_context(mech), (refTarget < 1) || !LOS,
                  "That is not a valid targetID. Try again.");
  DOCHECK_CONTEXT(mech_context(mech), MechTeam(mech) != MechTeam(target),
                  "You can't use the C3 network of unfriendly units!");
  DOCHECK_CONTEXT(mech_context(mech), mech_dbref(mech) == mech_dbref(target),
                  "You can't connect to yourself!");
  DOCHECK_CONTEXT(mech_context(mech), Destroyed(target),
                  "That unit is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), !Started(target),
                  "That unit is not started!");
  DOCHECK_CONTEXT(mech_context(mech), !HasC3(target),
                  "That unit does not appear to be equipped with C3!");

  /* validate the network of our target */
  validateC3Network(target);

  /* Let's see how much can actually fit in this network, based on the number of
   * masters and slaves */
  maxC3Size = countMaxC3Units(mech, MechC3Network(target),
                              MechC3NetworkSize(target), target);

  DOCHECK_CONTEXT(mech_context(mech),
                  maxC3Size < (MechC3NetworkSize(target) + 1),
                  "That unit's C3 network is operating at maximum capacity!");

  /* Connect us up */
  mech_notify(mech, MECHALL,
              tprintf("You connect to %s's C3 network.",
                      mech_to_mech_display_id(mech, target).text));

  addMechToC3Network(target, mech);
}

void mech_c3_message(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !HasC3(mech),
                  "This unit is not equipped with C3!");
  DOCHECK_CONTEXT(mech_context(mech), C3Destroyed(mech),
                  "Your C3 system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), AnyECMDisturbed(mech),
                  "Your C3 system is not currently operational!");

  validateC3Network(mech);

  DOCHECK_CONTEXT(mech_context(mech), MechC3NetworkSize(mech) <= 0,
                  "There are no other units in your C3 network!");

  skipws(buffer);
  DOCHECK_CONTEXT(mech_context(mech), !*buffer,
                  "What do you want to send on the C3 Network?");

  sendNetworkMessage(player, mech, buffer, 1);
}

void mech_c3_targets(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !HasC3(mech),
                  "This unit is not equipped with C3!");
  DOCHECK_CONTEXT(mech_context(mech), C3Destroyed(mech),
                  "Your C3 system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), AnyECMDisturbed(mech),
                  "Your C3 system is not currently operational!");

  validateC3Network(mech);

  DOCHECK_CONTEXT(mech_context(mech), MechC3NetworkSize(mech) <= 0,
                  "There are no other units in your C3 network!");

  showNetworkTargets(player, mech, 1);
}

void mech_c3_network(DbRef player, Mech *mech, char *buffer) {
  cch(MECH_USUALO);

  DOCHECK_CONTEXT(mech_context(mech), !HasC3(mech),
                  "This unit is not equipped with C3!");
  DOCHECK_CONTEXT(mech_context(mech), C3Destroyed(mech),
                  "Your C3 system is destroyed!");
  DOCHECK_CONTEXT(mech_context(mech), AnyECMDisturbed(mech),
                  "Your C3 system is not currently operational!");

  validateC3Network(mech);

  DOCHECK_CONTEXT(mech_context(mech), MechC3NetworkSize(mech) <= 0,
                  "There are no other units in your C3 network!");

  showNetworkData(player, mech, 1);
}
