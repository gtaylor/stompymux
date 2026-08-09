
/* Implements C3 targeting-network support. */

#include <string.h>

#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_c3_api.h"
#include "mech_c3_misc_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_network_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

static DbRef *c3_network_slot(DbRef *network, int index) {
  return checked_storage_at(network, C3_NETWORK_SIZE, sizeof(*network),
                            (size_t)index);
}

static DbRef c3_network_value(const DbRef *network, int index) {
  return *(const DbRef *)checked_storage_at_const(
      network, C3_NETWORK_SIZE, sizeof(*network), (size_t)index);
}

#define C3_POS_IN_NETWORK -1
#define C3_POS_NO_ROOM -2

#define C3_MASTER_MECH_SIZE 5
#define C3_MASTER_OTHER_SIZE 1

static bool mech_has_c3(const Mech *mech) {
  return mech_technology_flags(mech) & (C3_MASTER_TECH | C3_SLAVE_TECH);
}

int mech_c3_master_slot_count(const Mech *mech) {
  if (mech_class(mech) == CLASS_MECH)
    return C3_MASTER_MECH_SIZE;
  else
    return C3_MASTER_OTHER_SIZE;
}

bool mech_c3_master_slot_is_working(Mech *mech, int section, int slot) {
  int x = 0;
  int y, t;
  int wcWorkingSlots = 0;
  int wStartCheck = 0;
  int tDoBump;

  wStartCheck = MAX(0, slot - (mech_c3_master_slot_count(mech) - 1));

  while (x < CritsInLoc(mech, section)) {
    tDoBump = 0;

    if ((t = mech_critical_part_type(mech, section, x))) {
      if (special_from_equipment_index(t) == C3_MASTER) {
        if (x < wStartCheck) {
          tDoBump = 1;
        } else {
          /* We're within range of our slot, if not already on it */
          for (y = x; y < (x + mech_c3_master_slot_count(mech)); y++) {
            if (y != slot) {
              if (!mech_critical_is_nonfunctional(mech, section, y))
                wcWorkingSlots++;
            }
          }
        }
      }
    }

    if (tDoBump)
      x += mech_c3_master_slot_count(mech);
    else
      x++;
  }

  return (wcWorkingSlots == (mech_c3_master_slot_count(mech) - 1));
}

int mech_c3_working_master_count(Mech *mech) {
  int x, y, t;
  int wcSlots;
  int wcWorkingSlots;
  int wcMasters = 0;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting working C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wcSlots = 0;
    wcWorkingSlots = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++) {
      if ((t = mech_critical_part_type(mech, x, y))) {
        if (special_from_equipment_index(t) == C3_MASTER) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wcSlots++;

          if (!mech_critical_is_nonfunctional(mech, x, y)) {
            mech_network_debug(mech_context(mech),
                               "......and the slot is functional.");
            wcWorkingSlots++;
          }
        }
      }

      if (wcSlots == mech_c3_master_slot_count(mech)) {
        mech_network_debug(
            mech_context(mech),
            tprintf("...found enough slots for a C3Master for %ld.",
                    mech_dbref(mech)));
        wcSlots = 0;

        if (wcWorkingSlots == mech_c3_master_slot_count(mech)) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...there is even enough working slots to make the "
                      "computer work on %ld.",
                      mech_dbref(mech)));
          wcMasters++;
        }
      }
    }
  }

  mech_network_debug(mech_context(mech),
                     tprintf("Found %d working C3 masters on %ld", wcMasters,
                             mech_dbref(mech)));

  return wcMasters;
}

int mech_c3_total_master_count(Mech *mech) {
  int x, y, t;
  int wcSlots;
  int wcMasters = 0;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting total C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wcSlots = 0;

    for (y = 0; y < CritsInLoc(mech, x); y++) {
      if ((t = mech_critical_part_type(mech, x, y))) {
        if (special_from_equipment_index(t) == C3_MASTER) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wcSlots++;
        }
      }

      if (wcSlots == mech_c3_master_slot_count(mech)) {
        mech_network_debug(
            mech_context(mech),
            tprintf("...found enough slots for a C3Master for %ld.",
                    mech_dbref(mech)));

        wcSlots = 0;
        wcMasters++;
      }
    }
  }

  mech_network_debug(
      mech_context(mech),
      tprintf("Found %d total C3 masters on %ld", wcMasters, mech_dbref(mech)));

  return wcMasters;
}

int mech_c3_maximum_network_size(Mech *mech, const DbRef *myTempNetwork,
                                 int tempNetworkSize, Mech *targMech) {
  DbRef otherRef;
  Mech *otherMech;
  int i;
  int wcC3Masters = 0;
  int myMasters = 0;
  int maxC3Size;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting max C3 units in %ld's network", mech_dbref(mech)));

  if (targMech)
    mech_network_debug(
        mech_context(mech),
        tprintf("...using %ld as an additional mech", mech_dbref(targMech)));

  /* First we iterate over the list and find all the masters */
  for (i = 0; i < tempNetworkSize; i++) {
    otherRef = c3_network_value(myTempNetwork, i);
    otherMech = btech_context_get_mech(mech_context(mech), otherRef);

    if (!otherMech)
      continue;

    wcC3Masters += mech_c3_working_masters(otherMech);

    mech_network_debug(mech_context(mech),
                       tprintf("...for %ld, we add %d masters",
                               mech_dbref(otherMech),
                               mech_c3_working_masters(otherMech)));
  }

  /* Let's find out the max number of mechs in this network. Make sure we add in
   * any slaves we can control */
  maxC3Size = (wcC3Masters * 4) - wcC3Masters;

  mech_network_debug(mech_context(mech),
                     tprintf("...we now have a max size of %d", maxC3Size));

  myMasters = mech_c3_working_masters(mech);

  if (myMasters > 0)
    maxC3Size += (myMasters * 4) - myMasters;

  mech_network_debug(
      mech_context(mech),
      tprintf("...and after adding in my masters, we now have a max size of %d",
              maxC3Size));

  /* Let's see if a 2nd mech has been supplied to us */
  if (targMech) {
    myMasters = mech_c3_working_masters(targMech);

    if (myMasters > 0)
      maxC3Size += (myMasters * 4) - myMasters;
  }

  maxC3Size = MIN(maxC3Size, 11);

  mech_network_debug(mech_context(mech),
                     tprintf("...final max size of %d", maxC3Size));

  return maxC3Size;
}

int mech_c3_network_trim(Mech *mech, DbRef *myTempNetwork,
                         int tempNetworkSize) {
  DbRef otherRef;
  Mech *otherMech;
  int i;
  int newNetworkSize;
  int maxC3Size = 0; /* This is calc'd based on the number of masters */
  DbRef newNetwork[C3_NETWORK_SIZE];

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 TRIM: Trimming %ld's C3 network", mech_dbref(mech)));

  /* Initialize our data */
  newNetworkSize = tempNetworkSize;

  for (i = 0; i < C3_NETWORK_SIZE; i++)
    *c3_network_slot(newNetwork, i) = -1;

  /* Get our count of max units */
  maxC3Size = mech_c3_maximum_network_size(mech, myTempNetwork, tempNetworkSize,
                                           nullptr);

  mech_network_debug(mech_context(mech),
                     tprintf("C3 TRIM: Max C3 size: %d", maxC3Size));
  mech_network_debug(mech_context(mech),
                     tprintf("C3 TRIM: Current C3 size: %d", tempNetworkSize));

  /* Now we see if our network is oversized */
  if (maxC3Size < tempNetworkSize) {
    newNetworkSize = 0;

    /* First put our masters in */
    for (i = 0; i < tempNetworkSize; i++) {
      otherRef = c3_network_value(myTempNetwork, i);
      otherMech = btech_context_get_mech(mech_context(mech), otherRef);

      if (!otherMech)
        continue;

      if (mech_c3_working_masters(otherMech) > 0)
        *c3_network_slot(newNetwork, newNetworkSize++) = otherRef;
    }

    /* Next we put in slaves up to the max amount */
    if (newNetworkSize < maxC3Size) {
      for (i = 0; i < tempNetworkSize; i++) {
        otherRef = c3_network_value(myTempNetwork, i);
        otherMech = btech_context_get_mech(mech_context(mech), otherRef);

        if (!otherMech)
          continue;

        if (mech_c3_working_masters(otherMech) == 0)
          *c3_network_slot(newNetwork, newNetworkSize++) = otherRef;

        if (newNetworkSize >= maxC3Size)
          break;
      }
    }

    /* Now, refill our other temp network */
    for (i = 0; i < newNetworkSize; i++)
      *c3_network_slot(myTempNetwork, i) = c3_network_value(newNetwork, i);
  }

  return newNetworkSize;
}

int mech_c3_free_network_position(Mech *mech, Mech *mechToAdd) {
  int i;
  DbRef otherRef;

  mech_c3_network_validate(mech);

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = mech_c3_network_node(mech, i);

    if (otherRef > 0) {
      if (otherRef == mech_dbref(mechToAdd))
        return C3_POS_IN_NETWORK;
    } else
      return i;
  }

  return C3_POS_NO_ROOM;
}

void mech_c3_network_replicate(Mech *mechSrc, Mech *mechDest) {
  int i;
  DbRef otherRef;

  mech_network_debug(mech_context(mechSrc),
                     tprintf("C3 REPLICATE: %ld's C3 network to %ld",
                             mech_dbref(mechSrc), mech_dbref(mechDest)));

  mech_c3_network_clear(mechDest, 0);

  mech_c3_network_node_set(mechDest, 0, mech_dbref(mechSrc));
  mech_c3_network_size_set(mechDest, 1);

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = mech_c3_network_node(mechSrc, i);

    if (otherRef != mech_dbref(mechDest)) {
      const int destination_size = mech_c3_network_size(mechDest);
      mech_c3_network_node_set(mechDest, destination_size, otherRef);
      mech_c3_network_size_set(mechDest, destination_size + 1);
    }
  }

  mech_c3_network_validate(mechDest);
}

void mech_c3_network_add(Mech *mech, Mech *mechToAdd) {
  Mech *otherMech;
  Mech *otherNotifyMech;
  DbRef otherRef;
  int i;
  int wPos = -1;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 ADD: %ld to the C3 network of %ld",
                             mech_dbref(mechToAdd), mech_dbref(mech)));

  /* Find a position to add the new mech into my network */
  wPos = mech_c3_free_network_position(mech, mechToAdd);

  /* If we have a number that's less than 0, then we have an invalid position.
   * Either we're already in the network or there's not enough room */
  if (wPos < 0)
    return;

  /* Well, we have a valid position, so let's put this mech in the network */
  mech_network_debug(mech_context(mech),
                     tprintf("C3 ADD: Position to add to %ld's network is %d",
                             mech_dbref(mech), wPos));

  mech_c3_network_node_set(mech, wPos, mech_dbref(mechToAdd));
  mech_c3_network_size_set(mech, mech_c3_network_size(mech) + 1);

  mech_notify(mech, MECHALL,
              tprintf("%s connects to your C3 network.",
                      mech_to_mech_display_id(mech, mechToAdd).text));

  /* Now let's replicate the new network across the system so that everyone has
   * the same network settings */
  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherRef = mech_c3_network_node(mech, i);

    otherMech = mech_network_unit(mech, i, 0, 0, 0, 1);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    if (otherRef != mech_dbref(mechToAdd)) {
      otherNotifyMech = mech_network_unit(mech, i, 1, 1, 1, 1);

      if (otherNotifyMech)
        mech_notify(
            otherNotifyMech, MECHALL,
            tprintf("%s connects to your C3 network.",
                    mech_to_mech_display_id(otherNotifyMech, mechToAdd).text));
    }

    mech_c3_network_replicate(mech, otherMech);
  }

  /* Last, but not least, one final validation of the network */
  mech_c3_network_validate(mech);
}

void mech_c3_network_remove_reference(DbRef refToClear, Mech *mech) {
  int i;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 CLEAR: %ld from the C3 network of %ld",
                             refToClear, mech_dbref(mech)));

  if (!mech_c3_network_size(mech))
    return;

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    if (mech_c3_network_node(mech, i) == refToClear)
      mech_c3_network_node_set(mech, i, -1);
  }

  mech_c3_network_validate(mech);
}

void mech_c3_network_clear(Mech *mech, bool tClearFromOthers) {
  Mech *otherMech;
  int i;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 CLEAR: %ld's C3 network", mech_dbref(mech)));

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherMech = mech_network_unit(mech, i, 0, 0, 0, 1);

    mech_c3_network_node_set(mech, i, -1);

    if (tClearFromOthers) {
      if (!otherMech)
        continue;

      if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
        continue;

      mech_c3_network_remove_reference(mech_dbref(mech), otherMech);
    }
  }

  mech_c3_network_size_set(mech, 0);
}

void mech_c3_network_validate(Mech *mech) {
  Mech *otherMech;
  DbRef myTempNetwork[C3_NETWORK_SIZE];
  int i;
  int networkSize = 0;

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 VALIDATE: %ld's C3 network", mech_dbref(mech)));

  if (!mech_has_c3(mech) || mech_is_destroyed(mech) ||
      mech_condition_summary(mech).c3_destroyed) {
    mech_c3_network_clear(mech, 1);

    return;
  }

  if (mech_c3_network_size(mech) < 0) {
    mech_c3_network_clear(mech, 1);

    return;
  }

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    otherMech = mech_network_unit(mech, i, 0, 0, 0, 1);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(otherMech)))
      continue;

    mech_network_debug(
        mech_context(mech),
        tprintf("C3 VALIDATE INFO: %ld is now in %ld's C3 network",
                mech_dbref(otherMech), mech_dbref(mech)));

    *c3_network_slot(myTempNetwork, networkSize) = mech_dbref(otherMech);
    networkSize++;
  }

  mech_c3_network_clear(mech, 0);

  for (i = 0; i < networkSize; i++)
    mech_c3_network_node_set(mech, i, c3_network_value(myTempNetwork, i));

  mech_c3_network_size_set(mech, networkSize);

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 VALIDATE INFO: (PreTrim) %ld's C3 network is %d elements",
              mech_dbref(mech), mech_c3_network_size(mech)));

  networkSize = mech_c3_network_trim(mech, myTempNetwork, networkSize);

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 VALIDATE INFO: (PostTrim) %ld's C3 network has been "
              "trimmed to %d elements",
              mech_dbref(mech), networkSize));

  if (networkSize != mech_c3_network_size(mech)) {
    mech_c3_network_clear(mech, 0);

    for (i = 0; i < networkSize; i++)
      mech_c3_network_node_set(mech, i, c3_network_value(myTempNetwork, i));

    mech_c3_network_size_set(mech, networkSize);
  }
}

void mech_c3_join_leave(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[2];
  DbRef refTarget;
  int LOS = 1;
  float range = 0.0;
  int maxC3Size = 0;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to function!");
    return;
  }

  if (!mech_has_c3(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3!");
    return;
  }
  if (mech_condition_summary(mech).c3_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is not currently operational!");
    return;
  }

  mech_c3_network_validate(mech);

  /* Clear our C3 Network */
  if (!strcmp(args[0], "-")) {
    if (mech_c3_network_size(mech) <= 0) {
      mech_notify(mech, MECHALL, "You are not connected to a C3 network!");

      return;
    }

    mech_c3_network_clear(mech, 1);

    mech_notify(mech, MECHALL, "You disconnect from the C3 network.");

    return;
  }

  /* Well, if we're here then we wanna connect to a network */
  /* Let's check to see if we're already in one... can't be in two at the same
   * time */
  if (mech_c3_network_size(mech) > 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are already in a C3 network!");
    return;
  }

  /* Find who we're trying to connect to */
  refTarget = FindTargetDBREFFromMapNumber(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), refTarget);

  if (target)
    LOS = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), range);
  else
    refTarget = 0;

  if ((refTarget < 1) || !LOS) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That is not a valid targetID. Try again.");
    return;
  }
  if (mech_team(mech) != mech_team(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't use the C3 network of unfriendly units!");
    return;
  }
  if (mech_dbref(mech) == mech_dbref(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't connect to yourself!");
    return;
  }
  if (mech_is_destroyed(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit is destroyed!");
    return;
  }
  if (!mech_is_started(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit is not started!");
    return;
  }
  if (!mech_has_c3(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit does not appear to be equipped with C3!");
    return;
  }

  /* validate the network of our target */
  mech_c3_network_validate(target);

  /* Let's see how much can actually fit in this network, based on the number of
   * masters and slaves */
  DbRef target_network[C3_NETWORK_SIZE];
  const int target_network_size = mech_c3_network_size(target);
  for (int i = 0; i < target_network_size; ++i)
    *c3_network_slot(target_network, i) = mech_c3_network_node(target, i);
  maxC3Size = mech_c3_maximum_network_size(mech, target_network,
                                           target_network_size, target);

  if (maxC3Size < (mech_c3_network_size(target) + 1)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit's C3 network is operating at maximum capacity!");
    return;
  }

  /* Connect us up */
  mech_notify(mech, MECHALL,
              tprintf("You connect to %s's C3 network.",
                      mech_to_mech_display_id(mech, target).text));

  mech_c3_network_add(target, mech);
}

void mech_c3_message(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3!");
    return;
  }
  if (mech_condition_summary(mech).c3_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is not currently operational!");
    return;
  }

  mech_c3_network_validate(mech);

  if (mech_c3_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3 network!");
    return;
  }

  if (buffer != nullptr)
    buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char),
                                strspn(buffer, " \t\r\n\f\v"));
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "What do you want to send on the C3 Network?");
    return;
  }

  mech_network_send_message(player, mech, buffer, 1);
}

void mech_c3_targets(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3!");
    return;
  }
  if (mech_condition_summary(mech).c3_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is not currently operational!");
    return;
  }

  mech_c3_network_validate(mech);

  if (mech_c3_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3 network!");
    return;
  }

  mech_network_show_targets(player, mech, 1);
}

void mech_c3_network(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3!");
    return;
  }
  if (mech_condition_summary(mech).c3_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3 system is not currently operational!");
    return;
  }

  mech_c3_network_validate(mech);

  if (mech_c3_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3 network!");
    return;
  }

  mech_network_show_status(player, mech, 1);
}
