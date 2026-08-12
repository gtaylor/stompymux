
/* Implements C3 targeting-network support. */

#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "mech_api_types.h"
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
#include "section_types.h"

static DbRef *c3_network_slot(DbRef *network, int index) {
  return checked_storage_at(network, C3_NETWORK_SIZE, sizeof(*network),
                            (size_t)index);
}

static DbRef c3_network_value(const DbRef *network, int index) {
  return *(const DbRef *)checked_storage_at_const(
      network, C3_NETWORK_SIZE, sizeof(*network), (size_t)index);
}

#define C3_POS_IN_NETWORK (-1)
#define C3_POS_NO_ROOM (-2)

#define C3_MASTER_MECH_SIZE 5
#define C3_MASTER_OTHER_SIZE 1

static bool mech_has_c3(const Mech *mech) {
  return mech_technology_flags(mech) & (C3_MASTER_TECH | C3_SLAVE_TECH);
}

int mech_c3_master_slot_count(const Mech *mech) {
  if (mech_class(mech) == CLASS_MECH)
    return C3_MASTER_MECH_SIZE;
  return C3_MASTER_OTHER_SIZE;
}

bool mech_c3_master_slot_is_working(Mech *mech,
                                    CriticalSlotReference reference) {
  const int SECTION = reference.section;
  const int SLOT = reference.critical;
  int x = 0;
  int y, t;
  int wc_working_slots = 0;
  int w_start_check = 0;
  int t_do_bump;

  w_start_check = max(0, SLOT - (mech_c3_master_slot_count(mech) - 1));

  while (x < crits_in_loc(mech, SECTION)) {
    t_do_bump = 0;

    t = mech_critical_part_type(mech, SECTION, x);
    if (t) {
      if (special_from_equipment_index(t) == C3_MASTER) {
        if (x < w_start_check) {
          t_do_bump = 1;
        } else {
          /* We're within range of our slot, if not already on it */
          for (y = x; y < (x + mech_c3_master_slot_count(mech)); y++) {
            if (y != SLOT) {
              if (!mech_critical_is_nonfunctional(mech, SECTION, y))
                wc_working_slots++;
            }
          }
        }
      }
    }

    if (t_do_bump)
      x += mech_c3_master_slot_count(mech);
    else
      x++;
  }

  return (wc_working_slots == (mech_c3_master_slot_count(mech) - 1));
}

int mech_c3_working_master_count(Mech *mech) {
  int x, y, t;
  int wc_slots;
  int wc_working_slots;
  int wc_masters = 0;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting working C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wc_slots = 0;
    wc_working_slots = 0;

    for (y = 0; y < crits_in_loc(mech, x); y++) {
      t = mech_critical_part_type(mech, x, y);
      if (t) {
        if (special_from_equipment_index(t) == C3_MASTER) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wc_slots++;

          if (!mech_critical_is_nonfunctional(mech, x, y)) {
            mech_network_debug(mech_context(mech),
                               "......and the slot is functional.");
            wc_working_slots++;
          }
        }
      }

      if (wc_slots == mech_c3_master_slot_count(mech)) {
        mech_network_debug(
            mech_context(mech),
            tprintf("...found enough slots for a C3Master for %ld.",
                    mech_dbref(mech)));
        wc_slots = 0;

        if (wc_working_slots == mech_c3_master_slot_count(mech)) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...there is even enough working slots to make the "
                      "computer work on %ld.",
                      mech_dbref(mech)));
          wc_masters++;
        }
      }
    }
  }

  mech_network_debug(mech_context(mech),
                     tprintf("Found %d working C3 masters on %ld", wc_masters,
                             mech_dbref(mech)));

  return wc_masters;
}

int mech_c3_total_master_count(Mech *mech) {
  int x, y, t;
  int wc_slots;
  int wc_masters = 0;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting total C3 masters for %ld", mech_dbref(mech)));

  for (x = 0; x < NUM_SECTIONS; x++) {
    wc_slots = 0;

    for (y = 0; y < crits_in_loc(mech, x); y++) {
      t = mech_critical_part_type(mech, x, y);
      if (t) {
        if (special_from_equipment_index(t) == C3_MASTER) {
          mech_network_debug(
              mech_context(mech),
              tprintf("...found a C3Master slot at section %d, slot %d on %ld.",
                      x, y, mech_dbref(mech)));

          wc_slots++;
        }
      }

      if (wc_slots == mech_c3_master_slot_count(mech)) {
        mech_network_debug(
            mech_context(mech),
            tprintf("...found enough slots for a C3Master for %ld.",
                    mech_dbref(mech)));

        wc_slots = 0;
        wc_masters++;
      }
    }
  }

  mech_network_debug(mech_context(mech),
                     tprintf("Found %d total C3 masters on %ld", wc_masters,
                             mech_dbref(mech)));

  return wc_masters;
}

int mech_c3_maximum_network_size(Mech *mech, const DbRef *my_temp_network,
                                 int temp_network_size, Mech *targ_mech) {
  DbRef other_ref;
  Mech *other_mech;
  int i;
  int wc_c3_masters = 0;
  int my_masters = 0;
  int max_c3_size;

  mech_network_debug(
      mech_context(mech),
      tprintf("Counting max C3 units in %ld's network", mech_dbref(mech)));

  if (targ_mech)
    mech_network_debug(
        mech_context(mech),
        tprintf("...using %ld as an additional mech", mech_dbref(targ_mech)));

  /* First we iterate over the list and find all the masters */
  for (i = 0; i < temp_network_size; i++) {
    other_ref = c3_network_value(my_temp_network, i);
    other_mech = btech_context_get_mech(mech_context(mech), other_ref);

    if (!other_mech)
      continue;

    wc_c3_masters += mech_c3_working_masters(other_mech);

    mech_network_debug(mech_context(mech),
                       tprintf("...for %ld, we add %d masters",
                               mech_dbref(other_mech),
                               mech_c3_working_masters(other_mech)));
  }

  /* Let's find out the max number of mechs in this network. Make sure we add in
   * any slaves we can control */
  max_c3_size = (wc_c3_masters * 4) - wc_c3_masters;

  mech_network_debug(mech_context(mech),
                     tprintf("...we now have a max size of %d", max_c3_size));

  my_masters = mech_c3_working_masters(mech);

  if (my_masters > 0)
    max_c3_size += (my_masters * 4) - my_masters;

  mech_network_debug(
      mech_context(mech),
      tprintf("...and after adding in my masters, we now have a max size of %d",
              max_c3_size));

  /* Let's see if a 2nd mech has been supplied to us */
  if (targ_mech) {
    my_masters = mech_c3_working_masters(targ_mech);

    if (my_masters > 0)
      max_c3_size += (my_masters * 4) - my_masters;
  }

  max_c3_size = min(max_c3_size, 11);

  mech_network_debug(mech_context(mech),
                     tprintf("...final max size of %d", max_c3_size));

  return max_c3_size;
}

int mech_c3_network_trim(Mech *mech, DbRef *my_temp_network,
                         int temp_network_size) {
  DbRef other_ref;
  Mech *other_mech;
  int i;
  int new_network_size;
  int max_c3_size = 0; /* This is calc'd based on the number of masters */
  DbRef new_network[C3_NETWORK_SIZE];

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 TRIM: Trimming %ld's C3 network", mech_dbref(mech)));

  /* Initialize our data */
  new_network_size = temp_network_size;

  for (i = 0; i < C3_NETWORK_SIZE; i++)
    *c3_network_slot(new_network, i) = -1;

  /* Get our count of max units */
  max_c3_size = mech_c3_maximum_network_size(mech, my_temp_network,
                                             temp_network_size, nullptr);

  mech_network_debug(mech_context(mech),
                     tprintf("C3 TRIM: Max C3 size: %d", max_c3_size));
  mech_network_debug(mech_context(mech), tprintf("C3 TRIM: Current C3 size: %d",
                                                 temp_network_size));

  /* Now we see if our network is oversized */
  if (max_c3_size < temp_network_size) {
    new_network_size = 0;

    /* First put our masters in */
    for (i = 0; i < temp_network_size; i++) {
      other_ref = c3_network_value(my_temp_network, i);
      other_mech = btech_context_get_mech(mech_context(mech), other_ref);

      if (!other_mech)
        continue;

      if (mech_c3_working_masters(other_mech) > 0)
        *c3_network_slot(new_network, new_network_size++) = other_ref;
    }

    /* Next we put in slaves up to the max amount */
    if (new_network_size < max_c3_size) {
      for (i = 0; i < temp_network_size; i++) {
        other_ref = c3_network_value(my_temp_network, i);
        other_mech = btech_context_get_mech(mech_context(mech), other_ref);

        if (!other_mech)
          continue;

        if (mech_c3_working_masters(other_mech) == 0)
          *c3_network_slot(new_network, new_network_size++) = other_ref;

        if (new_network_size >= max_c3_size)
          break;
      }
    }

    /* Now, refill our other temp network */
    for (i = 0; i < new_network_size; i++)
      *c3_network_slot(my_temp_network, i) = c3_network_value(new_network, i);
  }

  return new_network_size;
}

int mech_c3_free_network_position(const MechNetworkLink *link) {
  Mech *mech = link->owner;
  int i;
  DbRef other_ref;

  mech_c3_network_validate(mech);

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    other_ref = mech_c3_network_node(mech, i);

    if (other_ref > 0) {
      if (other_ref == mech_dbref(link->member))
        return C3_POS_IN_NETWORK;
    } else
      return i;
  }

  return C3_POS_NO_ROOM;
}

void mech_c3_network_replicate(Mech *mech_src, Mech *mech_dest) {
  int i;
  DbRef other_ref;

  mech_network_debug(mech_context(mech_src),
                     tprintf("C3 REPLICATE: %ld's C3 network to %ld",
                             mech_dbref(mech_src), mech_dbref(mech_dest)));

  mech_c3_network_clear(mech_dest, 0);

  mech_c3_network_node_set(mech_dest, 0, mech_dbref(mech_src));
  mech_c3_network_size_set(mech_dest, 1);

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    other_ref = mech_c3_network_node(mech_src, i);

    if (other_ref != mech_dbref(mech_dest)) {
      const int DESTINATION_SIZE = mech_c3_network_size(mech_dest);
      mech_c3_network_node_set(mech_dest, DESTINATION_SIZE, other_ref);
      mech_c3_network_size_set(mech_dest, DESTINATION_SIZE + 1);
    }
  }

  mech_c3_network_validate(mech_dest);
}

void mech_c3_network_add(Mech *mech, Mech *mech_to_add) {
  Mech *other_mech;
  Mech *other_notify_mech;
  DbRef other_ref;
  int i;
  int w_pos = -1;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 ADD: %ld to the C3 network of %ld",
                             mech_dbref(mech_to_add), mech_dbref(mech)));

  /* Find a position to add the new mech into my network */
  w_pos = mech_c3_free_network_position(
      &(MechNetworkLink){.owner = mech, .member = mech_to_add});

  /* If we have a number that's less than 0, then we have an invalid position.
   * Either we're already in the network or there's not enough room */
  if (w_pos < 0)
    return;

  /* Well, we have a valid position, so let's put this mech in the network */
  mech_network_debug(mech_context(mech),
                     tprintf("C3 ADD: Position to add to %ld's network is %d",
                             mech_dbref(mech), w_pos));

  mech_c3_network_node_set(mech, w_pos, mech_dbref(mech_to_add));
  mech_c3_network_size_set(mech, mech_c3_network_size(mech) + 1);

  mech_notify(mech, MECHALL,
              tprintf("%s connects to your C3 network.",
                      mech_to_mech_display_id(mech, mech_to_add).text));

  /* Now let's replicate the new network across the system so that everyone has
   * the same network settings */
  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    other_ref = mech_c3_network_node(mech, i);

    other_mech = mech_network_unit(mech, i, 0, 0, 0, 1);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
      continue;

    if (other_ref != mech_dbref(mech_to_add)) {
      other_notify_mech = mech_network_unit(mech, i, 1, 1, 1, 1);

      if (other_notify_mech)
        mech_notify(
            other_notify_mech, MECHALL,
            tprintf(
                "%s connects to your C3 network.",
                mech_to_mech_display_id(other_notify_mech, mech_to_add).text));
    }

    mech_c3_network_replicate(mech, other_mech);
  }

  /* Last, but not least, one final validation of the network */
  mech_c3_network_validate(mech);
}

void mech_c3_network_remove_reference(DbRef ref_to_clear, Mech *mech) {
  int i;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 CLEAR: %ld from the C3 network of %ld",
                             ref_to_clear, mech_dbref(mech)));

  if (!mech_c3_network_size(mech))
    return;

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    if (mech_c3_network_node(mech, i) == ref_to_clear)
      mech_c3_network_node_set(mech, i, -1);
  }

  mech_c3_network_validate(mech);
}

void mech_c3_network_clear(Mech *mech, bool t_clear_from_others) {
  Mech *other_mech;
  int i;

  mech_network_debug(mech_context(mech),
                     tprintf("C3 CLEAR: %ld's C3 network", mech_dbref(mech)));

  for (i = 0; i < C3_NETWORK_SIZE; i++) {
    other_mech = mech_network_unit(mech, i, 0, 0, 0, 1);

    mech_c3_network_node_set(mech, i, -1);

    if (t_clear_from_others) {
      if (!other_mech)
        continue;

      if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
        continue;

      mech_c3_network_remove_reference(mech_dbref(mech), other_mech);
    }
  }

  mech_c3_network_size_set(mech, 0);
}

void mech_c3_network_validate(Mech *mech) {
  Mech *other_mech;
  DbRef my_temp_network[C3_NETWORK_SIZE];
  int i;
  int network_size = 0;

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
    other_mech = mech_network_unit(mech, i, 0, 0, 0, 1);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
      continue;

    mech_network_debug(
        mech_context(mech),
        tprintf("C3 VALIDATE INFO: %ld is now in %ld's C3 network",
                mech_dbref(other_mech), mech_dbref(mech)));

    *c3_network_slot(my_temp_network, network_size) = mech_dbref(other_mech);
    network_size++;
  }

  mech_c3_network_clear(mech, 0);

  for (i = 0; i < network_size; i++)
    mech_c3_network_node_set(mech, i, c3_network_value(my_temp_network, i));

  mech_c3_network_size_set(mech, network_size);

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 VALIDATE INFO: (PreTrim) %ld's C3 network is %d elements",
              mech_dbref(mech), mech_c3_network_size(mech)));

  network_size = mech_c3_network_trim(mech, my_temp_network, network_size);

  mech_network_debug(
      mech_context(mech),
      tprintf("C3 VALIDATE INFO: (PostTrim) %ld's C3 network has been "
              "trimmed to %d elements",
              mech_dbref(mech), network_size));

  if (network_size != mech_c3_network_size(mech)) {
    mech_c3_network_clear(mech, 0);

    for (i = 0; i < network_size; i++)
      mech_c3_network_node_set(mech, i, c3_network_value(my_temp_network, i));

    mech_c3_network_size_set(mech, network_size);
  }
}

void mech_c3_join_leave(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[2];
  DbRef ref_target;
  int los = 1;
  float range = 0.0;
  int max_c3_size = 0;

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
  ref_target = find_target_dbref_from_map_number(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), ref_target);

  if (target)
    los = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), range);
  else
    ref_target = 0;

  if ((ref_target < 1) || !los) {
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
  const int TARGET_NETWORK_SIZE = mech_c3_network_size(target);
  for (int i = 0; i < TARGET_NETWORK_SIZE; ++i)
    *c3_network_slot(target_network, i) = mech_c3_network_node(target, i);
  max_c3_size = mech_c3_maximum_network_size(mech, target_network,
                                             TARGET_NETWORK_SIZE, target);

  if (max_c3_size < (mech_c3_network_size(target) + 1)) {
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
