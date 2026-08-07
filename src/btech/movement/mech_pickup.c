/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 2002 Dr. Martin Brumm
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "btech_channel.h"
#include "btech_event.h"
#include "map_terrain.h"
#include "mech_lifecycle.h"
#include <math.h>
#include <string.h>

#include "btconfig.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "crit_api.h"
#include "eject_api.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"

static bool mech_carries_club(const Mech *mech) {
  return mech_section_carries_club(mech, RARM) ||
         mech_section_carries_club(mech, LARM);
}

static void mech_towing_target_prepare(Mech *target) {
  mech_swarm_target_set(target, -1);
  if (mech_class(target) == CLASS_MECH) {
    mech_fallen_set(target, true);
    mech_torso_twist_set(target, MECH_TORSO_CENTER);
    mech_arms_center(target);
    mech_event_cancel(target, EVENT_STAND);
  }
  mech_towed_set(target, true);
  mech_hull_down_set(target, false);
  mech_dug_in_set(target, false);
}

void mech_pickup(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *target;
  DbRef target_num;
  BattleMap *newmap;
  int argc, through_ice;
  char *args[4];
  BtechContext *context = mech_context(mech);

  if (player != GOD)
    if (!common_checks(player, mech, MECH_USUAL))
      return;
  argc = mech_parseattributes(buffer, args, 1);
  if (mech_condition_summary(mech).fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot tow while fortified.");
    return;
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot tow currently in this movement mode!");
    return;
  }
#endif
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments.");
    return;
  }
  target_num = FindTargetDBREFFromMapNumber(mech, args[0]);
  if (target_num == -1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is not in your line of sight.");
    return;
  }
  target = btech_context_get_mech(context, target_num);
  if (!target ||
      !mech_los_check(mech, target, mech_position_x(target),
                      mech_position_y(target), mech_range_to(mech, target))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is not in your line of sight.");
    return;
  }
  if (mech_condition_summary(target).fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your target is fortified and cannot be towed.");
    return;
  }
  if (mech_technology_flags_secondary(target) & CARRIER_TECH &&
      !(mech_technology_flags_secondary(mech) & CARRIER_TECH)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot handle the mass on that carrier.");
    return;
  }
  if (mech_carries_club(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pickup while you're carrying a club!");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pickup while jumping!");
    return;
  }
  if (mech_is_jumping(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "What are you going to do? Grab it from mid air?");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are in no position to pick anything up!");
    return;
  }
  if (mech_position_z(mech) > mech_position_z(target) + 3) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are too high above the target.");
    return;
  }
  if (mech_position_z(mech) < mech_position_z(target) - 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are too far below the target.");
    return;
  }
  if (mech_position_x(mech) != mech_position_x(target) ||
      mech_position_y(mech) != mech_position_y(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be in the same hex!");
    return;
  }
  if (mech_position_z(target) <= 0 &&
      mech_real_terrain_get(target) == BATTLE_TERRAIN_BRIDGE &&
      mech_position_z(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be under the bridge to pick up this unit.");
    return;
  }
  if (mech_is_towed(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target's already being towed by someone!");
    return;
  }
  if (mech_condition_summary(target).swarm_target == mech_dbref(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't grab hold!");
    return;
  }
  if (mech_tonnage(mech) < 5 ||
      (!is_in_character(btech_context_database(context), mech_dbref(target)) &&
       !mech_is_towable(target))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't tow that!");
    return;
  }
  if (mech_condition_summary(target).hidden) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You cannot pickup hiding targets....");
    return;
  }
  if (mech_event_count(target, EVENT_VEHICLEBURN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't tow a burning unit!");
    return;
  }
  if (mech_is_out_of_control(target)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "You can't tow a unit that is still OODing. Wait until it lands!");
    return;
  }
  if (mech_class(target) == CLASS_MW) {
    pickup_mw(mech, target);
    return;
  } else {
    if (mech_class(mech) == CLASS_MECH) {
      if (mech_movement_type(mech) == MOVE_QUAD) {
        mech_notify(mech, MECHALL, "You've got four left feet, you can't tow!");
        return;
      }
      if (mech_section_is_destroyed(mech, LARM)) {
        mech_notify(mech, MECHALL,
                    "Your left arm is destroyed, you can't pick up anything.");
        return;
      }
      if (mech_section_is_destroyed(mech, RARM)) {
        mech_notify(mech, MECHALL,
                    "Your right arm is destroyed, you can't pick up anything.");
        return;
      }
      if (!(mech_critical_is_operational_special(mech, RARM, 3,
                                                 HAND_OR_FOOT_ACTUATOR) &&
            mech_critical_is_operational_special(mech, RARM, 0,
                                                 SHOULDER_OR_HIP)) &&
          !(mech_critical_is_operational_special(mech, LARM, 3,
                                                 HAND_OR_FOOT_ACTUATOR) &&
            mech_critical_is_operational_special(mech, LARM, 0,
                                                 SHOULDER_OR_HIP))) {
        mech_notify(mech, MECHALL,
                    "You need functioning arm to pick things up!");
        return;
      }
    } else if (!(mech_technology_flags(mech) & SALVAGE_TECH)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "You can't pick that up in this MECH/VEHICLE");
      return;
    }
  }
  if (mech_carried_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are already carrying a Mech");
    return;
  }
  if (fabsf(mech_current_speed(mech)) > 1.0F ||
      fabsf(mech_vertical_speed(mech)) > 1.0F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are moving too fast to attempt a pickup.");
    return;
  }
  if (mech_is_dropship(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pick that up!");
    return;
  }
  if (mech_movement_type(target) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That's simply immobile!");
    return;
  }
  if (mech_team(mech) != mech_team(target) && mech_is_started(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pick that up!");
    return;
  }

  /* Not on the same team, unit is !destroyed, don't allow.. Prevents picking up
   * from heat shutdown, etc */
  /* Allow Team 0 (Administrative Team for Box Drops, etc) */
  if (mech_team(mech) != mech_team(target) && !mech_is_destroyed(target) &&
      !mech_is_started(target) && mech_team(target) != 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pick that up!");
    return;
  }

  if (mech_event_count(target, EVENT_MOVE) &&
      !mech_event_count(target, EVENT_FALL) && !mech_is_out_of_control(target))
    mech_event_cancel(target, EVENT_MOVE);

  if (mech_event_count(target, EVENT_MOVE)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't pick up a moving target!");
    return;
  }

  mech_printf(target, MECHALL, "%s attaches his tow lines to you.",
              mech_to_mech_display_id(target, mech).text);
  mech_printf(mech, MECHALL, "You attach your tow lines to %s.",
              mech_to_mech_display_id(mech, target).text);
  if (mech_carried_dbref(target) > 0)
    mech_dropoff(GOD, target, "");
  if ((newmap = btech_context_get_map(context, mech_map_dbref(target))))
    mech_los_broadcast_unit(mech, target, "picks up %s!");
  mech_carried_dbref_set(mech, mech_dbref(target));
  mech_towing_target_prepare(target);

  through_ice = mech_real_terrain_get(target) == BATTLE_TERRAIN_ICE &&
                mech_position_z(mech) >= 0 && mech_position_z(target) < 0;
  mech_position_mirror(target, mech, 0);
  MarkForLOSUpdate(target);
  mech_flood(target);
  if (through_ice) {
    if (mech_position_z(mech) == 0 && mech_movement_type(mech) != MOVE_HOVER)
      drop_thru_ice(mech);
    else
      break_thru_ice(mech);
  }
  if (!mech_is_destroyed(target))
    mech_power_down(target);

  /* Adjust the speed involved */
  mech_speed_correct(mech);

  /* Send emit for triggers/debugging */
  btech_channel_send(
      context, BTECH_CHANNEL_MECH_DEBUG, "%s",
      tprintf("#%ld has picked up #%ld", mech_dbref(mech), mech_dbref(target)));
}

void mech_attachcables(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *towMech;
  Mech *target;
  DbRef towMech_num;
  DbRef target_num;
  int argc;
  char *args[3];
  char mechName[SBUF_SIZE];
  char towMechName[SBUF_SIZE];
  char targetName[SBUF_SIZE];
  BtechContext *context = mech_context(mech);

  if (player != GOD)
    if (!common_checks(player, mech, MECH_USUAL))
      return;

  argc = mech_parseattributes(buffer, args, 2);
  if (argc != 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments.");
    return;
  }

  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't attach cables while floating in the air!");
    return;
  }

  /* Check the towing unit. */
  towMech_num = FindTargetDBREFFromMapNumber(mech, args[0]);
  if (towMech_num == -1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is not in your line of sight.");
    return;
  }
  towMech = btech_context_get_mech(context, towMech_num);
  if (!towMech ||
      !mech_los_check(mech, towMech, mech_position_x(towMech),
                      mech_position_y(towMech), mech_range_to(mech, towMech))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is not in your line of sight.");
    return;
  }
  if (mech_position_x(mech) != mech_position_x(towMech) ||
      mech_position_y(mech) != mech_position_y(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be in the same hex as the towing unit!");
    return;
  }
  if (mech_is_jumping(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is currently flying through the air!");
    return;
  }
  if (mech_position_z(mech) != mech_position_z(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must be on the same elevation as the towing unit!");
    return;
  }
  if (mech_carried_dbref(towMech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is towing someone else!");
    return;
  }
  if (mech_is_towed(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is already being towed by someone!");
    return;
  }
  if (mech_class(towMech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not tow!");
    return;
  }
  if (mech_movement_type(towMech) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not tow!");
    return;
  }
  if (mech_tonnage(towMech) < 5) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not tow!");
    return;
  }
  if (mech_is_destroyed(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Destroyed units can not tow!");
    return;
  }
  if (mech_tonnage(towMech) < 5 ||
      !is_in_character(btech_context_database(context), mech_dbref(towMech))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not tow!");
    return;
  }
  if (mech_event_count(towMech, EVENT_VEHICLEBURN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not attach tow cables to a burning unit!");
    return;
  }
  if (mech_technology_flags(towMech) & SALVAGE_TECH) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "That is a dedicated towing unit and can pick up the target itself!");
    return;
  }
  if (fabsf(mech_current_speed(towMech)) > 0.0F ||
      fabsf(mech_vertical_speed(towMech)) > 0.0F) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The towing unit is moving to fast for you to grab the tow cables!");
    return;
  }
  if (mech_team(towMech) != mech_team(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not grab the tow cables from that unit!");
    return;
  }
  if (mech_class(towMech) != CLASS_MECH &&
      mech_class(towMech) != CLASS_VEH_GROUND) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not tow!");
    return;
  }

  /* Check the target */
  target_num = FindTargetDBREFFromMapNumber(mech, args[1]);
  if (target_num == -1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is not in your line of sight.");
    return;
  }
  target = btech_context_get_mech(context, target_num);
  if (!target ||
      !mech_los_check(mech, target, mech_position_x(target),
                      mech_position_y(target), mech_range_to(mech, target))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is not in your line of sight.");
    return;
  }
  if (mech_position_x(mech) != mech_position_x(target) ||
      mech_position_y(mech) != mech_position_y(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be in the same hex as the target!");
    return;
  }
  if (mech_is_jumping(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is currently flying through the air!");
    return;
  }
  if (mech_position_z(mech) != mech_position_z(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must be on the same elevation as the target!");
    return;
  }
  if (mech_carried_dbref(target) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is towing someone else!");
    return;
  }
  if (mech_is_towed(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is already being towed by someone!");
    return;
  }
  if (!is_in_character(btech_context_database(context), mech_dbref(target)) &&
      !mech_is_towable(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not be towed!");
    return;
  }
  if (mech_class(target) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not be towed!");
    return;
  }
  if (mech_movement_type(target) == MOVE_NONE) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not be towed!");
    return;
  }
  if (mech_is_dropship(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not be towed!");
    return;
  }
  if (mech_event_count(target, EVENT_VEHICLEBURN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not attach tow cables to a burning unit!");
    return;
  }
  if (fabsf(mech_current_speed(target)) > 0.0F ||
      fabsf(mech_vertical_speed(target)) > 0.0F) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The target is moving to fast for you to attach the tow cables!");
    return;
  }
  if (mech_team(target) != mech_team(mech) && mech_is_started(target)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit can not be towed!");
    return;
  }

  if (mech_event_count(target, EVENT_MOVE) &&
      !mech_event_count(target, EVENT_FALL) && !mech_is_out_of_control(target))
    mech_event_cancel(target, EVENT_MOVE);

  if (mech_event_count(target, EVENT_MOVE)) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "The target is moving to fast for you to attach the tow cables!");
    return;
  }

  strcpy(mechName, mech_display_id(mech).text);
  strcpy(towMechName, mech_display_id(towMech).text);
  strcpy(targetName, mech_display_id(target).text);

  mech_printf(target, MECHALL, "%s attaches tow lines from %s to you.",
              mechName, towMechName);
  mech_printf(towMech, MECHALL, "%s attaches your tow lines to %s.", mechName,
              targetName);
  mech_printf(mech, MECHALL, "You attach %s's tow lines to %s.", towMechName,
              targetName);

  mech_los_broadcast(mech, tprintf("attaches tow cables from %s to %s!",
                                   towMechName, targetName));

  mech_carried_dbref_set(towMech, mech_dbref(target));
  mech_towing_target_prepare(target);

  if (!mech_is_destroyed(target))
    mech_power_down(target);

  /* Adjust the speed involved */
  mech_speed_correct(towMech);
}

void mech_detachcables(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *towMech;
  Mech *target;
  DbRef towMech_num;
  BattleMap *newmap;
  DbRef aRef;
  int argc;
  char *args[2];
  char mechName[SBUF_SIZE];
  char towMechName[SBUF_SIZE];
  char targetName[SBUF_SIZE];
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUAL))
    return;

  argc = mech_parseattributes(buffer, args, 1);
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments.");
    return;
  }

  towMech_num = FindTargetDBREFFromMapNumber(mech, args[0]);
  if (towMech_num == -1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is not in your line of sight.");
    return;
  }
  towMech = btech_context_get_mech(context, towMech_num);
  if (!towMech ||
      !mech_los_check(mech, towMech, mech_position_x(towMech),
                      mech_position_y(towMech), mech_range_to(mech, towMech))) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That towing unit is not in your line of sight.");
    return;
  }
  if (mech_position_x(mech) != mech_position_x(towMech) ||
      mech_position_y(mech) != mech_position_y(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be in the same hex as the towing unit!");
    return;
  }
  if (mech_position_z(mech) != mech_position_z(towMech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You must be on the same elevation as the towing unit!");
    return;
  }
  if (mech_carried_dbref(towMech) <= 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That unit is not towing anyone!");
    return;
  }

  aRef = mech_carried_dbref(towMech);
  mech_carried_dbref_set(towMech, -1);
  target = btech_context_get_mech(context, aRef);
  if (!target) {
    mecha_notify(btech_context_evaluation(context), player,
                 "The towed unit was invalid!");
    return;
  }
  mech_towed_set(target, false);

  strcpy(mechName, mech_display_id(mech).text);
  strcpy(towMechName, mech_display_id(towMech).text);
  strcpy(targetName, mech_display_id(target).text);

  mech_printf(mech, MECHALL, "You detach %s's tow lines from %s.", towMechName,
              targetName);
  mech_printf(towMech, MECHALL, "%s detaches your tow lines from %s.", mechName,
              targetName);
  mech_notify(target, MECHALL, "You have been released from towing.");

  mech_event_cancel(target, EVENT_MOVE);
  mech_movement_stop(target);

  mech_los_broadcast(mech, tprintf("detaches %s's tow cables from %s!",
                                   towMechName, targetName));

  if ((newmap = btech_context_get_map(context, mech_map_dbref(target)))) {
    mech_position_hex_z_set(
        target, battle_map_hex_elevation(newmap, mech_position_x(towMech),
                                         mech_position_y(towMech)));
  }

  mech_speed_correct(towMech);
}

void mech_dropoff(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *target;
  BattleMap *newmap;
  DbRef aRef;
  int x, y;
  BtechContext *context = mech_context(mech);

  if (player != GOD)
    if (!common_checks(player, mech, MECH_USUAL))
      return;

  if (mech_carried_dbref(mech) <= 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You aren't carrying a mech!");
    return;
  }
  aRef = mech_carried_dbref(mech);
  mech_carried_dbref_set(mech, -1);
  target = btech_context_get_mech(context, aRef);
  if (!target) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You were towing invalid target!");
    return;
  }
  mech_towed_set(target, false);
  mech_notify(mech, MECHALL, "You drop the mech you were carrying.");
  mech_notify(target, MECHALL, "You have been released from towing.");

  mech_event_cancel(target, EVENT_MOVE);
  mech_movement_stop(target);

  if ((newmap = btech_context_get_map(context, mech_map_dbref(target)))) {
    mech_los_broadcast_unit(mech, target, "drops %s!");
    if ((x = mech_position_z(target)) >
        ((y = battle_map_hex_elevation(newmap, mech_position_x(target),
                                       mech_position_y(target))) +
         2)) {
      mech_notify(mech, MECHALL,
                  "Maybe you should have done this closer to the ground.");
      mech_notify(
          target, MECHALL,
          "You wish he had done that a might bit closer to the ground.");
      mech_los_broadcast(target, "falls through the sky.");
      mech_event_schedule(target, EVENT_FALL, mech_fall_event, FALL_TICK, -1);
    } else {
      if (map_terrain_get(newmap, mech_position_x(mech),
                          mech_position_y(mech)) == BATTLE_TERRAIN_ICE)
        mech_position_hex_z_set(target, 0);
      else
        mech_position_hex_z_set(
            target, battle_map_hex_elevation(newmap, mech_position_x(mech),
                                             mech_position_y(mech)));
    }
  }
  mech_speed_correct(mech);
  btech_channel_send(context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                     tprintf("#%ld has dropped off #%ld", mech_dbref(mech),
                             mech_dbref(target)));
}
