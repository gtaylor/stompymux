
/* Implements C3i targeting-network support. */

#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "context_internal.h" // IWYU pragma: keep
#include "equipment_types.h"
#include "mech_api_types.h"
#include "mech_c3_misc_api.h"
#include "mech_c3_network_internal.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
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
#include "registry_api.h"

static DbRef *c3i_network_slot(DbRef *network, int index) {
  return checked_storage_at(network, C3I_NETWORK_SIZE, sizeof(*network),
                            (size_t)index);
}

static DbRef c3i_network_value(const DbRef *network, int index) {
  return *(const DbRef *)checked_storage_at_const(
      network, C3I_NETWORK_SIZE, sizeof(*network), (size_t)index);
}

static bool mech_has_c3i(const Mech *mech) {
  return (mech_technology_flags_secondary(mech) & C3I_TECH) != 0;
}

int mech_c3i_free_network_position(const MechNetworkLink *link) {
  Mech *mech = link->owner;
  int i;
  DbRef other_ref;

  mech_c3i_network_validate(mech);

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    other_ref = mech_c3i_network_node(mech, i);

    if (other_ref > 0) {
      if (other_ref == mech_dbref(link->member))
        return C3_POS_IN_NETWORK;
    } else {
      return i;
    }
  }

  return C3_POS_NO_ROOM;
}

void mech_c3i_network_replicate(Mech *mech_src, Mech *mech_dest) {
  int i;
  DbRef other_ref;

  mech_c3i_network_clear(mech_dest, 0);

  mech_c3i_network_node_set(mech_dest, 0, mech_dbref(mech_src));
  mech_c3i_network_size_set(mech_dest, 1);

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    other_ref = mech_c3i_network_node(mech_src, i);

    if (other_ref != mech_dbref(mech_dest)) {
      const int DESTINATION_SIZE = mech_c3i_network_size(mech_dest);
      mech_c3i_network_node_set(mech_dest, DESTINATION_SIZE, other_ref);
      mech_c3i_network_size_set(mech_dest, DESTINATION_SIZE + 1);
    }
  }

  mech_c3i_network_validate(mech_dest);
}

void mech_c3i_network_add(Mech *mech, Mech *mech_to_add) {
  Mech *other_mech;
  Mech *other_notify_mech;
  DbRef other_ref;
  int i;
  int w_pos = -1;

  /* Find a position to add the new mech into my network */
  w_pos = mech_c3i_free_network_position(
      &(MechNetworkLink){.owner = mech, .member = mech_to_add});

  /* If we have a number that's less than 0, then we have an invalid position.
   * Either we're already in the network or there's not enough room */
  if (w_pos < 0)
    return;

  /* Well, we have a valid position, so let's put this mech in the network */
  mech_c3i_network_node_set(mech, w_pos, mech_dbref(mech_to_add));
  mech_c3i_network_size_set(mech, mech_c3i_network_size(mech) + 1);

  mech_printf(mech, MECHALL, "%s connects to your C3i network.",
              mech_to_mech_display_id(mech, mech_to_add).text);

  /* Now let's replicate the new network across the system so that everyone has
   * the same network settings */
  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    other_ref = mech_c3i_network_node(mech, i);

    other_mech = mech_network_unit(mech, i, false, false, false, false);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
      continue;

    if (other_ref != mech_dbref(mech_to_add)) {
      other_notify_mech = mech_network_unit(mech, i, true, true, true, false);

      if (other_notify_mech) {
        mech_printf(
            other_notify_mech, MECHALL, "%s connects to your C3i network.",
            mech_to_mech_display_id(other_notify_mech, mech_to_add).text);
      }
    }

    mech_c3i_network_replicate(mech, other_mech);
  }

  /* Last, but not least, one final validation of the network */
  mech_c3i_network_validate(mech);
}

void mech_c3i_network_remove_reference(DbRef ref_to_clear, Mech *mech) {
  int i;

  if (!mech_c3i_network_size(mech))
    return;

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    if (mech_c3i_network_node(mech, i) == ref_to_clear)
      mech_c3i_network_node_set(mech, i, -1);
  }

  mech_c3i_network_validate(mech);
}

void mech_c3i_network_clear(Mech *mech, int t_clear_from_others) {
  Mech *other_mech;
  int i;

  for (i = 0; i < C3I_NETWORK_SIZE; i++) {
    other_mech = mech_network_unit(mech, i, false, false, false, false);

    mech_c3i_network_node_set(mech, i, -1);

    if (t_clear_from_others) {
      if (!other_mech)
        continue;

      if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
        continue;

      mech_c3i_network_remove_reference(mech_dbref(mech), other_mech);
    }
  }

  mech_c3i_network_size_set(mech, 0);
}

void mech_c3i_network_validate(Mech *mech) {
  Mech *other_mech;
  DbRef my_temp_network[C3I_NETWORK_SIZE];
  int i;
  int network_size = 0;

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
    other_mech = mech_network_unit(mech, i, false, false, false, false);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(mech)->database, mech_dbref(other_mech)))
      continue;

    *c3i_network_slot(my_temp_network, network_size++) = mech_dbref(other_mech);
  }

  mech_c3i_network_clear(mech, 0);

  for (i = 0; i < network_size; i++)
    mech_c3i_network_node_set(mech, i, c3i_network_value(my_temp_network, i));

  mech_c3i_network_size_set(mech, network_size);
}

void mech_c3i_join_leave(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *target;
  char *args[2];
  DbRef ref_target;
  int los = 1;
  float range = 0.0;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (mech_parseattributes(buffer, args, 2) != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Invalid number of arguments to function!");
    return;
  }

  if (!mech_has_c3i(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3i!");
    return;
  }
  if (mech_condition_summary(mech).c3i_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is not currently operational!");
    return;
  }

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
  if (mech_c3i_network_size(mech) > 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You are already in a C3i network!");
    return;
  }

  /* Find who we're trying to connect to */
  ref_target = find_target_dbref_from_map_number(mech, args[0]);
  target = btech_context_get_mech(mech_context(mech), ref_target);

  if (target) {
    los = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), range);
  } else {
    ref_target = 0;
  }

  if ((ref_target < 1) || !los) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That is not a valid targetID. Try again.");
    return;
  }
  if (mech_team(mech) != mech_team(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You can't use the C3i network of unfriendly units!");
    return;
  }
  if (mech == target) {
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
  if (!mech_has_c3i(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit does not appear to be equipped with C3i!");
    return;
  }

  /* validate the network of our target */
  mech_c3i_network_validate(target);
  if (mech_c3i_network_size(target) >= C3I_NETWORK_SIZE) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That unit's C3i network is operating at maximum capacity!");
    return;
  }

  /* Connect us up */
  mech_printf(mech, MECHALL, "You connect to %s's C3i network.",
              mech_to_mech_display_id(mech, target).text);

  mech_c3i_network_add(target, mech);
}

void mech_c3i_message(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3i(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3i!");
    return;
  }
  if (mech_condition_summary(mech).c3i_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is not currently operational!");
    return;
  }

  mech_c3i_network_validate(mech);

  if (mech_c3i_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3i network!");
    return;
  }

  if (buffer != nullptr)
    buffer = checked_storage_at(buffer, strlen(buffer) + 1, sizeof(char),
                                strspn(buffer, " \t\r\n\f\v"));
  if (!buffer || !*buffer) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "What do you want to send on the C3i Network?");
    return;
  }

  mech_network_send_message(player, mech, buffer, false);
}

void mech_c3i_targets(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3i(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3i!");
    return;
  }
  if (mech_condition_summary(mech).c3i_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is not currently operational!");
    return;
  }

  mech_c3i_network_validate(mech);

  if (mech_c3i_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3i network!");
    return;
  }

  mech_network_show_targets(player, mech, false);
}

void mech_c3i_network(DbRef player, Mech *mech, char *buffer) {
  if (!common_checks(player, mech, MECH_USUALO))
    return;

  if (!mech_has_c3i(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "This unit is not equipped with C3i!");
    return;
  }
  if (mech_condition_summary(mech).c3i_destroyed) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is destroyed!");
    return;
  }
  if (mech_is_any_ecm_disturbed(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your C3i system is not currently operational!");
    return;
  }

  mech_c3i_network_validate(mech);

  if (mech_c3i_network_size(mech) <= 0) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "There are no other units in your C3i network!");
    return;
  }

  mech_network_show_status(player, mech, false);
}
