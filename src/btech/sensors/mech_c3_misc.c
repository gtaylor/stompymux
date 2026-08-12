/* Implements BattleTech sensor mechanics for unit c3 misc. */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_c3_api.h"
#include "mech_c3_misc_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_contacts_api.h"
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
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/objects/attrs.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/server/server_control.h"
#include "mux/support/alloc.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"

#define TARG_LOS_NONE 0
#define TARG_LOS_CLEAR 1
#define TARG_LOS_SOMETHING 2

#define DEBUG_C3 0

typedef struct C3ContactLine {
  float sort_range;
  char text[120];
} C3ContactLine;

static DbRef *c3_network_slot(DbRef *network, int index) {
  return checked_storage_at(network, C3_NETWORK_SIZE, sizeof(*network),
                            (size_t)index);
}

static DbRef c3_network_value(const DbRef *network, int index) {
  return *(const DbRef *)checked_storage_at_const(
      network, C3_NETWORK_SIZE, sizeof(*network), (size_t)index);
}

static C3ContactLine *c3_contact_line(C3ContactLine *lines, int index) {
  return checked_storage_at(lines, BATTLE_MAP_UNIT_CAPACITY, sizeof(*lines),
                            (size_t)index);
}

static bool mech_has_c3(const Mech *mech) {
  return mech_technology_flags(mech) & (C3_MASTER_TECH | C3_SLAVE_TECH);
}

static bool mech_has_c3i(const Mech *mech) {
  return mech_technology_flags_secondary(mech) & C3I_TECH;
}

Mech *mech_network_temporary_unit(BtechContext *context, int w_idx,
                                  const DbRef *my_network, int network_size) {
  Mech *temp_mech;
  DbRef ref_other_mech;

  if ((w_idx > network_size) || (w_idx < 0))
    return NULL;

  ref_other_mech = c3_network_value(my_network, w_idx);

  if (ref_other_mech > 0) {
    temp_mech = btech_context_get_mech(context, ref_other_mech);

    if (!temp_mech)
      return NULL;

    if (mech_is_destroyed(temp_mech))
      return NULL;

    return temp_mech;
  }

  return NULL;
}

Mech *mech_network_unit(Mech *mech, int w_idx, bool t_check_ecm,
                        bool t_check_started, bool t_check_uncon,
                        bool t_is_c3) {
  Mech *temp_mech;
  DbRef ref_other_mech;
  int network_size;

  network_size =
      t_is_c3 ? mech_c3_network_size(mech) : mech_c3i_network_size(mech);

  if ((w_idx >= network_size) || (w_idx < 0))
    return NULL;

  ref_other_mech = t_is_c3 ? mech_c3_network_node(mech, w_idx)
                           : mech_c3i_network_node(mech, w_idx);

  if (ref_other_mech > 0) {
    temp_mech = btech_context_get_mech(mech_context(mech), ref_other_mech);

    if (!temp_mech)
      return NULL;

    if (mech_team(temp_mech) != mech_team(mech))
      return NULL;

    if (mech_map_dbref(temp_mech) != mech_map_dbref(mech))
      return NULL;

    if (mech_is_destroyed(temp_mech))
      return NULL;

    if (t_is_c3) {
      if (!mech_has_c3(temp_mech)) /* Sanity check */
        return NULL;

      if (mech_condition_summary(temp_mech).c3_destroyed)
        return NULL;
    } else {
      if (!mech_has_c3i(temp_mech)) /* Sanity check */
        return NULL;

      if (mech_condition_summary(temp_mech).c3i_destroyed)
        return NULL;
    }

    if (t_check_ecm)
      if (mech_is_any_ecm_disturbed(temp_mech))
        return NULL;

    if (t_check_started)
      if (!mech_is_started(temp_mech))
        return NULL;

    if (t_check_uncon)
      if (mech_pilot_is_unconscious(temp_mech))
        return NULL;

    return temp_mech;
  }

  return NULL;
}

void mech_network_build_temporary(Mech *mech, DbRef *my_network,
                                  int *network_size, bool t_check_ecm,
                                  bool t_check_started, bool t_check_uncon,
                                  bool t_is_c3) {
  int temp_network_size = 0;
  int base_network_size;
  Mech *other_mech;
  DbRef my_temp_network[C3_NETWORK_SIZE];
  int i;

  /* Re-init the network */
  for (i = 0; i < C3_NETWORK_SIZE; i++)
    *c3_network_slot(my_network, i) = -1;

  *network_size = 0;

  base_network_size =
      t_is_c3 ? mech_c3_network_size(mech) : mech_c3i_network_size(mech);

  if (base_network_size == 0)
    return;

  /*
   * Build the base netork of all the mechs that fit the criteria we passed in
   */
  for (i = 0; i < base_network_size; i++) {
    other_mech = mech_network_unit(mech, i, t_check_ecm, t_check_started,
                                   t_check_uncon, t_is_c3);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    *c3_network_slot(my_temp_network, temp_network_size) =
        mech_dbref(other_mech);
    temp_network_size++;
  }

  /*
   * Once we're here, we're done with the C3i stuff, but we need to make sure
   * that this is a valid C3 network still. For example, we may have lost a
   * master due to death or something else, so we need to make sure we have
   * enough masters left to actually do something.
   *
   * A valid network is one where there are MIN((((NUM_MASTERS * 4) -
   * NUM_MASTERS) + ((MY_MASTERS * 4) - MY_MASTERS), 11) units in the network
   */
  if (t_is_c3) {
    if (temp_network_size > 0)
      temp_network_size =
          mech_c3_network_trim(mech, my_temp_network, temp_network_size);
  }

  for (i = 0; i < temp_network_size; i++)
    *c3_network_slot(my_network, i) = c3_network_value(my_temp_network, i);

  *network_size = temp_network_size;
}

void mech_network_send_message(DbRef player, Mech *mech, const char *msg,
                               bool t_is_c3) {
  int i;
  Mech *other_mech;
  MechDisplayId display_id = mech_display_id(mech);
  const char *c = display_id.text;
  char buf[LBUF_SIZE] = {0};
  int network_size;
  DbRef my_network[C3_NETWORK_SIZE];

  mech_network_build_temporary(mech, my_network, &network_size, 1, 1, 1,
                               t_is_c3);

  for (i = 0; i < network_size; i++) {
    other_mech = mech_network_temporary_unit(mech_context(mech), i, my_network,
                                             network_size);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    (void)snprintf(buf, LBUF_SIZE, "[bold]%s/%s: %s[reset]",
                   (t_is_c3 ? "C3" : "C3i"), c, msg);
    mech_notify(other_mech, MECHALL, buf);
  }

  (void)snprintf(buf, LBUF_SIZE, "[bold]%s/You: %s[reset]",
                 (t_is_c3 ? "C3" : "C3i"), msg);
  mech_notify(mech, MECHALL, buf);
}

void mech_network_show_targets(DbRef player, Mech *mech, bool t_is_c3) {
  BattleMap *obj_map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i, j, bearing;
  Mech *other_mech;
  float real_range, c3_range;
  char buff[LBUF_SIZE];
  const char *mech_name;
  char move_type[30];
  char c_status1, c_status2, c_status3, c_status4, c_status5;
  char weaponarc;
  int los_flag;
  int arc;
  int w_see_target = TARG_LOS_NONE;
  int w_c3_see_target = TARG_LOS_NONE;
  int t_show_status_info = 0;
  C3ContactLine contacts[BATTLE_MAP_UNIT_CAPACITY];
  int buffindex = 0;
  int network_size;
  DbRef my_network[C3_NETWORK_SIZE];
  DbRef c3_ref;

  mech_network_build_temporary(mech, my_network, &network_size, 1, 1, 0,
                               t_is_c3);

  /*
   * Send then a 'contacts' style report. This is different from the
   * normal contacts since it has a 'physical' range in it too.
   */
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "%s Contacts:", t_is_c3 ? "C3" : "C3i");

  for (i = 0; i < battle_map_unit_count(obj_map); i++) {
    const DbRef OTHER_DBREF = battle_map_unit_dbref(obj_map, i);
    if (!(OTHER_DBREF != mech_dbref(mech) && OTHER_DBREF != -1))
      continue;

    other_mech = btech_context_get_mech(mech_context(mech), OTHER_DBREF);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    real_range = mech_range_to(mech, other_mech);
    los_flag = mech_los_check(mech, other_mech, mech_position_x(other_mech),
                              mech_position_y(other_mech), real_range);

    /*
     * If we do see them, let's make sure it's not just a 'something'
     */
    if (los_flag) {
      if (mech_los_check_unblocked(mech, other_mech,
                                   mech_position_x(other_mech),
                                   mech_position_y(other_mech), 0.0))
        w_see_target = TARG_LOS_CLEAR;
      else
        w_see_target = TARG_LOS_SOMETHING;
    } else {
      w_see_target = TARG_LOS_NONE;
    }

    /*
     * If I don't see it, let's see if someone else in the network does
     */
    if (w_see_target != TARG_LOS_CLEAR)
      w_c3_see_target = mech_network_visibility(&(MechNetworkVisibilityRequest){
          .observer = mech, .target = other_mech, .is_c3 = t_is_c3});

    /* If noone sees it, we continue */
    if (!w_see_target && !w_c3_see_target)
      continue;

    /* Get our network range */
    c3_range = mech_network_range_with_members(
        mech, other_mech, real_range, my_network, network_size, &c3_ref);

    /* Figure out if we show the info or not... ie, do we actually 'see' it */
    if ((w_see_target != TARG_LOS_CLEAR) &&
        (w_c3_see_target != TARG_LOS_CLEAR)) {
      t_show_status_info = 0;
      mech_name = "something";
    } else {
      t_show_status_info = 1;
      mech_name = btech_attribute_read(mech_context(other_mech)->database,
                                       mech_dbref(other_mech), A_MECHNAME,
                                       (char[LBUF_SIZE]){0});
    }

    bearing = map_bearing(
        &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                    .y = mech_position_real_y(mech)},
                          .end = {.x = mech_position_real_x(other_mech),
                                  .y = mech_position_real_y(other_mech)}});
    strlcpy(move_type, get_move_type_id(mech_movement_type(other_mech)),
            sizeof(move_type));

    /* Get our weapon arc */
    arc = in_weapon_arc(mech, mech_position_real_x(other_mech),
                        mech_position_real_y(other_mech));
    weaponarc = mech_contact_weapon_arc(arc);

    /* Now get our status chars */
    if (!t_show_status_info) {
      c_status1 = ' ';
      c_status2 = ' ';
      c_status3 = ' ';
      c_status4 = ' ';
      c_status5 = ' ';
    } else {
      c_status1 = mech_contact_status_character(mech, other_mech, 1);
      c_status2 = mech_contact_status_character(mech, other_mech, 2);
      c_status3 = mech_contact_status_character(mech, other_mech, 3);
      c_status4 = mech_contact_status_character(mech, other_mech, 4);
      c_status5 = mech_contact_status_character(mech, other_mech, 5);
    }

    /* Now, build the string */
    (void)snprintf(
        buff, sizeof(buff),
        "%s%c%c%c[%s]%c %-11.11s x:%3d y:%3d z:%3d r:%4.1f c:%4.1f b:%3d "
        "s:%5.1f h:%3d S:%c%c%c%c%c%s",
        mech_dbref(other_mech) == mech_target_dbref(mech) ? "[fg=red bold]"
        : (t_show_status_info && mech_team(mech) != mech_team(other_mech))
            ? "[fg=yellow bold]"
            : "",
        (los_flag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
        (los_flag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
        mech_id(other_mech,
                mech_team(mech) == mech_team(other_mech) || !t_show_status_info)
            .text,
        move_type[0], mech_name, mech_position_x(other_mech),
        mech_position_y(other_mech), mech_position_z(other_mech),
        (double)real_range, (double)c3_range, bearing,
        (double)mech_current_speed(other_mech),
        mech_heading_degrees(other_mech), c_status1, c_status2, c_status3,
        c_status4, c_status5,
        (mech_dbref(other_mech) == mech_target_dbref(mech) ||
         mech_team(mech) != mech_team(other_mech))
            ? "[reset]"
            : "");

    C3ContactLine *contact = c3_contact_line(contacts, buffindex++);
    contact->sort_range =
        real_range + (mech_is_destroyed(other_mech) ? 10000.0F : 0.0F);
    (void)snprintf(contact->text, sizeof(contact->text), "%s", buff);
  }

  /* print a sorted list of detected mechs */
  /* use the ever-popular bubble sort */
  for (i = 0; i < (buffindex - 1); i++)
    for (j = (i + 1); j < buffindex; j++)
      if (c3_contact_line(contacts, j)->sort_range >
          c3_contact_line(contacts, i)->sort_range) {
        C3ContactLine temporary = *c3_contact_line(contacts, i);
        *c3_contact_line(contacts, i) = *c3_contact_line(contacts, j);
        *c3_contact_line(contacts, j) = temporary;
      }

  for (i = 0; i < buffindex; i++)
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 c3_contact_line(contacts, i)->text);

  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "End %s Contact List", t_is_c3 ? "C3" : "C3i");
}

void mech_network_show_status(DbRef player, Mech *mech, bool t_is_c3) {
  int i, bearing;
  Mech *other_mech;
  float range;
  char buff[LBUF_SIZE];
  const char *mech_name;
  char move_type[30];
  int network_size;
  DbRef my_network[C3_NETWORK_SIZE];

  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "%s Network Status:", t_is_c3 ? "C3" : "C3i");

  mech_network_build_temporary(mech, my_network, &network_size, 1, 1, 0,
                               t_is_c3);

  for (i = 0; i < network_size; i++) {
    other_mech = mech_network_temporary_unit(mech_context(mech), i, my_network,
                                             network_size);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    range = mech_range_to(mech, other_mech);
    bearing = map_bearing(
        &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                    .y = mech_position_real_y(mech)},
                          .end = {.x = mech_position_real_x(other_mech),
                                  .y = mech_position_real_y(other_mech)}});

    strlcpy(move_type, get_move_type_id(mech_movement_type(other_mech)),
            sizeof(move_type));

    mech_name = btech_attribute_read(mech_context(other_mech)->database,
                                     mech_dbref(other_mech), A_MECHNAME,
                                     (char[LBUF_SIZE]){0});

    (void)snprintf(buff, sizeof(buff),
                   "[fg=yellow bold][%s][reset]%c %-12.12s x:%3d y:%3d z:%3d "
                   "r:%4.1f "
                   "b:%3d s:%5.1f "
                   "h:%3d a: %3d i: %3d[reset]",
                   mech_id(other_mech, true).text, move_type[0], mech_name,
                   mech_position_x(other_mech), mech_position_y(other_mech),
                   mech_position_z(other_mech), (double)range, bearing,
                   (double)mech_current_speed(other_mech),
                   mech_heading_degrees(other_mech),
                   get_remaining_armor_percent(other_mech),
                   get_remaining_internal_percent(other_mech));

    mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
  }

  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "End %s Network Status", t_is_c3 ? "C3" : "C3i");
}

int mech_network_visibility(const MechNetworkVisibilityRequest *request) {
  Mech *mech = request->observer;
  Mech *mech_target = request->target;
  const bool T_IS_C3 = request->is_c3;
  int los = TARG_LOS_NONE;
  float range = 0.0;
  int i;
  int network_size;
  DbRef my_network[C3_NETWORK_SIZE];
  Mech *other_mech;

  mech_network_build_temporary(mech, my_network, &network_size, 1, 1, 0,
                               T_IS_C3);

  if (network_size == 0)
    return TARG_LOS_NONE;

  for (i = 0; i < network_size; i++) {
    other_mech = mech_network_temporary_unit(mech_context(mech), i, my_network,
                                             network_size);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    if (other_mech == mech_target)
      continue;

    range = mech_range_to(other_mech, mech_target);
    los = mech_los_check(other_mech, mech_target, mech_position_x(mech_target),
                         mech_position_y(mech_target), range);

    if (los) {
      if (!mech_los_check_unblocked(other_mech, mech_target,
                                    mech_position_x(mech_target),
                                    mech_position_y(mech_target), range)) {
        los = TARG_LOS_SOMETHING;
      } else {
        los = TARG_LOS_CLEAR;
        break;
      }
    }
  }

  return los;
}

float mech_network_range(Mech *mech, Mech *mech_target, float real_range,
                         DbRef *c3_ref, bool t_is_c3) {
  int network_size;
  DbRef my_network[C3_NETWORK_SIZE];

  if (t_is_c3) {
    if (mech_condition_summary(mech).c3_destroyed) {
      return real_range;
    }
  } else {
    if (mech_condition_summary(mech).c3i_destroyed) {
      mech_c3i_network_validate(mech);

      return real_range;
    }
  }

  if (mech_is_any_ecm_disturbed(mech))
    return real_range;

  mech_network_build_temporary(mech, my_network, &network_size, 1, 1, 0,
                               t_is_c3);

  return mech_network_range_with_members(mech, mech_target, real_range,
                                         my_network, network_size, c3_ref);
}

float mech_network_range_with_members(Mech *mech, Mech *mech_target,
                                      float real_range, const DbRef *my_network,
                                      int network_size, DbRef *c3_ref) {
  float c3_range = 0.0;
  float best_range = 0.0;
  int i;
  int in_los = 0;
  int map_x, map_y;
  float hex_x, hex_y, hex_z;
  Mech *other_mech;
  BattleMap *map;

  best_range = real_range;
  *c3_ref = 0;

  if (network_size == 0)
    return real_range;

  for (i = 0; i < network_size; i++) {
    other_mech = mech_network_temporary_unit(mech_context(mech), i, my_network,
                                             network_size);

    if (!other_mech)
      continue;

    if (!is_good_obj(mech_context(other_mech)->database,
                     mech_dbref(other_mech)))
      continue;

    if (mech_target) {
      if (other_mech == mech_target)
        continue;

      mech_network_debug(
          mech_context(mech),
          tprintf("C3RANGE-NETWORK (mech): Finding range from %ld to %ld.",
                  mech_dbref(mech), mech_dbref(mech_target)));

      c3_range = mech_range_to(other_mech, mech_target);
      in_los =
          mech_los_check(other_mech, mech_target, mech_position_x(mech_target),
                         mech_position_y(mech_target), c3_range);
    } else if ((mech_target_hex_x(mech) > 0) && (mech_target_hex_y(mech) > 0)) {
      map_x = mech_target_hex_x(mech);
      map_y = mech_target_hex_y(mech);
      map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

      mech_network_debug(
          mech_context(mech),
          tprintf("C3RANGE-NETWORK (hex): Finding range from %ld to %d %d.",
                  mech_dbref(mech), map_x, map_y));

      mech_target_hex_z_set(mech, battle_map_hex_elevation(map, map_x, map_y));
      const int TARGET_HEX_Z = mech_target_hex_z(mech);
      hex_z = ZSCALE * (float)TARGET_HEX_Z;
      map_coord_to_real_coord(map_x, map_y, &hex_x, &hex_y);

      c3_range = map_spatial_range(&(MapSpatialSegment){
          .start = {.x = mech_position_real_x(other_mech),
                    .y = mech_position_real_y(other_mech),
                    .z = mech_position_real_z(other_mech)},
          .end = {.x = hex_x, .y = hex_y, .z = hex_z},
      });
      in_los =
          mech_los_check_unblocked(other_mech, nullptr, map_x, map_y, c3_range);
    } else {
      continue;
    }

    if (in_los && (c3_range < best_range)) {
      best_range = c3_range;
      *c3_ref = mech_dbref(other_mech);
    }
  }

  return best_range;
}

void mech_network_debug(BtechContext *context, const char *msg) {
  if (DEBUG_C3)
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s", msg);
}
