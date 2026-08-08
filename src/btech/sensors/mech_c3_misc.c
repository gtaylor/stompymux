/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "command_handlers_api.h"
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

Mech *mech_network_temporary_unit(BtechContext *context, int wIdx,
                                  const DbRef *myNetwork, int networkSize) {
  Mech *tempMech;
  DbRef refOtherMech;

  if ((wIdx > networkSize) || (wIdx < 0))
    return NULL;

  refOtherMech = c3_network_value(myNetwork, wIdx);

  if (refOtherMech > 0) {
    tempMech = btech_context_get_mech(context, refOtherMech);

    if (!tempMech)
      return NULL;

    if (mech_is_destroyed(tempMech))
      return NULL;

    return tempMech;
  }

  return NULL;
}

Mech *mech_network_unit(Mech *mech, int wIdx, bool tCheckECM,
                        bool tCheckStarted, bool tCheckUncon, bool tIsC3) {
  Mech *tempMech;
  DbRef refOtherMech;
  int networkSize;

  networkSize =
      tIsC3 ? mech_c3_network_size(mech) : mech_c3i_network_size(mech);

  if ((wIdx >= networkSize) || (wIdx < 0))
    return NULL;

  refOtherMech = tIsC3 ? mech_c3_network_node(mech, wIdx)
                       : mech_c3i_network_node(mech, wIdx);

  if (refOtherMech > 0) {
    tempMech = btech_context_get_mech(mech_context(mech), refOtherMech);

    if (!tempMech)
      return NULL;

    if (mech_team(tempMech) != mech_team(mech))
      return NULL;

    if (mech_map_dbref(tempMech) != mech_map_dbref(mech))
      return NULL;

    if (mech_is_destroyed(tempMech))
      return NULL;

    if (tIsC3) {
      if (!mech_has_c3(tempMech)) /* Sanity check */
        return NULL;

      if (mech_condition_summary(tempMech).c3_destroyed)
        return NULL;
    } else {
      if (!mech_has_c3i(tempMech)) /* Sanity check */
        return NULL;

      if (mech_condition_summary(tempMech).c3i_destroyed)
        return NULL;
    }

    if (tCheckECM)
      if (mech_is_any_ecm_disturbed(tempMech))
        return NULL;

    if (tCheckStarted)
      if (!mech_is_started(tempMech))
        return NULL;

    if (tCheckUncon)
      if (mech_pilot_is_unconscious(tempMech))
        return NULL;

    return tempMech;
  }

  return NULL;
}

void mech_network_build_temporary(Mech *mech, DbRef *myNetwork,
                                  int *networkSize, bool tCheckECM,
                                  bool tCheckStarted, bool tCheckUncon,
                                  bool tIsC3) {
  int tempNetworkSize = 0;
  int baseNetworkSize;
  Mech *otherMech;
  DbRef myTempNetwork[C3_NETWORK_SIZE];
  int i;

  /* Re-init the network */
  for (i = 0; i < C3_NETWORK_SIZE; i++)
    *c3_network_slot(myNetwork, i) = -1;

  *networkSize = 0;

  baseNetworkSize =
      tIsC3 ? mech_c3_network_size(mech) : mech_c3i_network_size(mech);

  if (baseNetworkSize == 0)
    return;

  /*
   * Build the base netork of all the mechs that fit the criteria we passed in
   */
  for (i = 0; i < baseNetworkSize; i++) {
    otherMech = mech_network_unit(mech, i, tCheckECM, tCheckStarted,
                                  tCheckUncon, tIsC3);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    *c3_network_slot(myTempNetwork, tempNetworkSize) = mech_dbref(otherMech);
    tempNetworkSize++;
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
  if (tIsC3) {
    if (tempNetworkSize > 0)
      tempNetworkSize =
          mech_c3_network_trim(mech, myTempNetwork, tempNetworkSize);
  }

  for (i = 0; i < tempNetworkSize; i++)
    *c3_network_slot(myNetwork, i) = c3_network_value(myTempNetwork, i);

  *networkSize = tempNetworkSize;
}

void mech_network_send_message(DbRef player, Mech *mech, const char *msg,
                               bool tIsC3) {
  int i;
  Mech *otherMech;
  MechDisplayId display_id = mech_display_id(mech);
  const char *c = display_id.text;
  char buf[LBUF_SIZE] = {0};
  int networkSize;
  DbRef myNetwork[C3_NETWORK_SIZE];

  mech_network_build_temporary(mech, myNetwork, &networkSize, 1, 1, 1, tIsC3);

  for (i = 0; i < networkSize; i++) {
    otherMech = mech_network_temporary_unit(mech_context(mech), i, myNetwork,
                                            networkSize);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    snprintf(buf, LBUF_SIZE, "[bold]%s/%s: %s[reset]", (tIsC3 ? "C3" : "C3i"),
             c, msg);
    mech_notify(otherMech, MECHALL, buf);
  }

  snprintf(buf, LBUF_SIZE, "[bold]%s/You: %s[reset]", (tIsC3 ? "C3" : "C3i"),
           msg);
  mech_notify(mech, MECHALL, buf);
}

void mech_network_show_targets(DbRef player, Mech *mech, bool tIsC3) {
  BattleMap *objMap =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int i, j, bearing;
  Mech *otherMech;
  float realRange, c3Range;
  char buff[LBUF_SIZE];
  const char *mech_name;
  char move_type[30];
  char cStatus1, cStatus2, cStatus3, cStatus4, cStatus5;
  char weaponarc;
  int losFlag;
  int arc;
  int wSeeTarget = TARG_LOS_NONE;
  int wC3SeeTarget = TARG_LOS_NONE;
  int tShowStatusInfo = 0;
  C3ContactLine contacts[BATTLE_MAP_UNIT_CAPACITY];
  int buffindex = 0;
  int networkSize;
  DbRef myNetwork[C3_NETWORK_SIZE];
  DbRef c3Ref;

  mech_network_build_temporary(mech, myNetwork, &networkSize, 1, 1, 0, tIsC3);

  /*
   * Send then a 'contacts' style report. This is different from the
   * normal contacts since it has a 'physical' range in it too.
   */
  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "%s Contacts:", tIsC3 ? "C3" : "C3i");

  for (i = 0; i < battle_map_unit_count(objMap); i++) {
    const DbRef other_dbref = battle_map_unit_dbref(objMap, i);
    if (!(other_dbref != mech_dbref(mech) && other_dbref != -1))
      continue;

    otherMech = btech_context_get_mech(mech_context(mech), other_dbref);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    tShowStatusInfo = 0;
    realRange = mech_range_to(mech, otherMech);
    losFlag = mech_los_check(mech, otherMech, mech_position_x(otherMech),
                             mech_position_y(otherMech), realRange);

    /*
     * If we do see them, let's make sure it's not just a 'something'
     */
    if (losFlag) {
      if (mech_los_check_unblocked(mech, otherMech, mech_position_x(otherMech),
                                   mech_position_y(otherMech), 0.0))
        wSeeTarget = TARG_LOS_CLEAR;
      else
        wSeeTarget = TARG_LOS_SOMETHING;
    } else
      wSeeTarget = TARG_LOS_NONE;

    /*
     * If I don't see it, let's see if someone else in the network does
     */
    if (wSeeTarget != TARG_LOS_CLEAR)
      wC3SeeTarget = mech_network_visibility(mech, otherMech, tIsC3);

    /* If noone sees it, we continue */
    if (!wSeeTarget && !wC3SeeTarget)
      continue;

    /* Get our network range */
    c3Range = mech_network_range_with_members(mech, otherMech, realRange,
                                              myNetwork, networkSize, &c3Ref);

    /* Figure out if we show the info or not... ie, do we actually 'see' it */
    if ((wSeeTarget != TARG_LOS_CLEAR) && (wC3SeeTarget != TARG_LOS_CLEAR)) {
      tShowStatusInfo = 0;
      mech_name = "something";
    } else {
      tShowStatusInfo = 1;
      mech_name = btech_attribute_read(mech_context(otherMech)->database,
                                       mech_dbref(otherMech), A_MECHNAME,
                                       (char[LBUF_SIZE]){0});
    }

    bearing = FindBearing(
        mech_position_real_x(mech), mech_position_real_y(mech),
        mech_position_real_x(otherMech), mech_position_real_y(otherMech));
    strlcpy(move_type, GetMoveTypeID(mech_movement_type(otherMech)),
            sizeof(move_type));

    /* Get our weapon arc */
    arc = InWeaponArc(mech, mech_position_real_x(otherMech),
                      mech_position_real_y(otherMech));
    weaponarc = mech_contact_weapon_arc(arc);

    /* Now get our status chars */
    if (!tShowStatusInfo) {
      cStatus1 = ' ';
      cStatus2 = ' ';
      cStatus3 = ' ';
      cStatus4 = ' ';
      cStatus5 = ' ';
    } else {
      cStatus1 = mech_contact_status_character(mech, otherMech, 1);
      cStatus2 = mech_contact_status_character(mech, otherMech, 2);
      cStatus3 = mech_contact_status_character(mech, otherMech, 3);
      cStatus4 = mech_contact_status_character(mech, otherMech, 4);
      cStatus5 = mech_contact_status_character(mech, otherMech, 5);
    }

    /* Now, build the string */
    snprintf(buff, sizeof(buff),
             "%s%c%c%c[%s]%c %-11.11s x:%3d y:%3d z:%3d r:%4.1f c:%4.1f b:%3d "
             "s:%5.1f h:%3d S:%c%c%c%c%c%s",
             mech_dbref(otherMech) == mech_target_dbref(mech) ? "[fg=red bold]"
             : (tShowStatusInfo && mech_team(mech) != mech_team(otherMech))
                 ? "[fg=yellow bold]"
                 : "",
             (losFlag & BATTLE_MAP_LOS_SEEN_PRIMARY) ? 'P' : ' ',
             (losFlag & BATTLE_MAP_LOS_SEEN_SECONDARY) ? 'S' : ' ', weaponarc,
             mech_id(otherMech, mech_team(mech) == mech_team(otherMech) ||
                                    !tShowStatusInfo)
                 .text,
             move_type[0], mech_name, mech_position_x(otherMech),
             mech_position_y(otherMech), mech_position_z(otherMech),
             (double)realRange, (double)c3Range, bearing,
             (double)mech_current_speed(otherMech),
             mech_heading_degrees(otherMech), cStatus1, cStatus2, cStatus3,
             cStatus4, cStatus5,
             (mech_dbref(otherMech) == mech_target_dbref(mech) ||
              mech_team(mech) != mech_team(otherMech))
                 ? "[reset]"
                 : "");

    C3ContactLine *contact = c3_contact_line(contacts, buffindex++);
    contact->sort_range =
        realRange + (mech_is_destroyed(otherMech) ? 10000.0F : 0.0F);
    snprintf(contact->text, sizeof(contact->text), "%s", buff);
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
                "End %s Contact List", tIsC3 ? "C3" : "C3i");
}

void mech_network_show_status(DbRef player, Mech *mech, bool tIsC3) {
  int i, bearing;
  Mech *otherMech;
  float range;
  char buff[LBUF_SIZE];
  const char *mech_name;
  char move_type[30];
  int networkSize;
  DbRef myNetwork[C3_NETWORK_SIZE];

  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "%s Network Status:", tIsC3 ? "C3" : "C3i");

  mech_network_build_temporary(mech, myNetwork, &networkSize, 1, 1, 0, tIsC3);

  for (i = 0; i < networkSize; i++) {
    otherMech = mech_network_temporary_unit(mech_context(mech), i, myNetwork,
                                            networkSize);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    range = mech_range_to(mech, otherMech);
    bearing = FindBearing(
        mech_position_real_x(mech), mech_position_real_y(mech),
        mech_position_real_x(otherMech), mech_position_real_y(otherMech));

    strlcpy(move_type, GetMoveTypeID(mech_movement_type(otherMech)),
            sizeof(move_type));

    mech_name = btech_attribute_read(mech_context(otherMech)->database,
                                     mech_dbref(otherMech), A_MECHNAME,
                                     (char[LBUF_SIZE]){0});

    snprintf(buff, sizeof(buff),
             "[fg=yellow bold][%s][reset]%c %-12.12s x:%3d y:%3d z:%3d "
             "r:%4.1f "
             "b:%3d s:%5.1f "
             "h:%3d a: %3d i: %3d[reset]",
             mech_id(otherMech, true).text, move_type[0], mech_name,
             mech_position_x(otherMech), mech_position_y(otherMech),
             mech_position_z(otherMech), (double)range, bearing,
             (double)mech_current_speed(otherMech),
             mech_heading_degrees(otherMech),
             getRemainingArmorPercent(otherMech),
             getRemainingInternalPercent(otherMech));

    mecha_notify(btech_context_evaluation(mech_context(mech)), player, buff);
  }

  notify_printf(btech_context_evaluation(mech_context(mech)), player,
                "End %s Network Status", tIsC3 ? "C3" : "C3i");
}

int mech_network_visibility(Mech *mech, Mech *mechTarget, bool tIsC3) {
  int los = TARG_LOS_NONE;
  float range = 0.0;
  int i;
  int networkSize;
  DbRef myNetwork[C3_NETWORK_SIZE];
  Mech *otherMech;

  mech_network_build_temporary(mech, myNetwork, &networkSize, 1, 1, 0, tIsC3);

  if (networkSize == 0)
    return TARG_LOS_NONE;

  for (i = 0; i < networkSize; i++) {
    otherMech = mech_network_temporary_unit(mech_context(mech), i, myNetwork,
                                            networkSize);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    if (otherMech == mechTarget)
      continue;

    range = mech_range_to(otherMech, mechTarget);
    los = mech_los_check(otherMech, mechTarget, mech_position_x(mechTarget),
                         mech_position_y(mechTarget), range);

    if (los) {
      if (!mech_los_check_unblocked(otherMech, mechTarget,
                                    mech_position_x(mechTarget),
                                    mech_position_y(mechTarget), range))
        los = TARG_LOS_SOMETHING;
      else {
        los = TARG_LOS_CLEAR;
        break;
      }
    }
  }

  return los;
}

float mech_network_range(Mech *mech, Mech *mechTarget, float realRange,
                         DbRef *c3Ref, bool tIsC3) {
  int networkSize;
  DbRef myNetwork[C3_NETWORK_SIZE];

  if (tIsC3) {
    if (mech_condition_summary(mech).c3_destroyed) {
      return realRange;
    }
  } else {
    if (mech_condition_summary(mech).c3i_destroyed) {
      mech_c3i_network_validate(mech);

      return realRange;
    }
  }

  if (mech_is_any_ecm_disturbed(mech))
    return realRange;

  mech_network_build_temporary(mech, myNetwork, &networkSize, 1, 1, 0, tIsC3);

  return mech_network_range_with_members(mech, mechTarget, realRange, myNetwork,
                                         networkSize, c3Ref);
}

float mech_network_range_with_members(Mech *mech, Mech *mechTarget,
                                      float realRange, const DbRef *myNetwork,
                                      int networkSize, DbRef *c3Ref) {
  float c3Range = 0.0;
  float bestRange = 0.0;
  int i;
  int inLOS = 0;
  int mapX, mapY;
  float hexX, hexY, hexZ;
  Mech *otherMech;
  BattleMap *map;

  bestRange = realRange;
  *c3Ref = 0;

  if (networkSize == 0)
    return realRange;

  for (i = 0; i < networkSize; i++) {
    otherMech = mech_network_temporary_unit(mech_context(mech), i, myNetwork,
                                            networkSize);

    if (!otherMech)
      continue;

    if (!is_good_obj(mech_context(otherMech)->database, mech_dbref(otherMech)))
      continue;

    if (mechTarget) {
      if (otherMech == mechTarget)
        continue;

      mech_network_debug(
          mech_context(mech),
          tprintf("C3RANGE-NETWORK (mech): Finding range from %ld to %ld.",
                  mech_dbref(mech), mech_dbref(mechTarget)));

      c3Range = mech_range_to(otherMech, mechTarget);
      inLOS = mech_los_check(otherMech, mechTarget, mech_position_x(mechTarget),
                             mech_position_y(mechTarget), c3Range);
    } else if ((mech_target_hex_x(mech) > 0) && (mech_target_hex_y(mech) > 0)) {
      mapX = mech_target_hex_x(mech);
      mapY = mech_target_hex_y(mech);
      map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));

      mech_network_debug(
          mech_context(mech),
          tprintf("C3RANGE-NETWORK (hex): Finding range from %ld to %d %d.",
                  mech_dbref(mech), mapX, mapY));

      mech_target_hex_z_set(mech, battle_map_hex_elevation(map, mapX, mapY));
      const int target_hex_z = mech_target_hex_z(mech);
      hexZ = ZSCALE * (float)target_hex_z;
      MapCoordToRealCoord(mapX, mapY, &hexX, &hexY);

      c3Range = FindRange(mech_position_real_x(otherMech),
                          mech_position_real_y(otherMech),
                          mech_position_real_z(otherMech), hexX, hexY, hexZ);
      inLOS = mech_los_check_unblocked(otherMech, nullptr, mapX, mapY, c3Range);
    } else {
      continue;
    }

    if (inLOS && (c3Range < bestRange)) {
      bestRange = c3Range;
      *c3Ref = mech_dbref(otherMech);
    }
  }

  return bestRange;
}

void mech_network_debug(BtechContext *context, const char *msg) {
  if (DEBUG_C3)
    btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s", msg);
}
