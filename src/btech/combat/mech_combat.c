/* Implements BattleTech combat mechanics for unit combat. */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "command_handlers_api.h"
#include "map.h"
#include "map_obj_api.h"
#include "map_terrain.h"
#include "mech_build_api.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_condition_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_spot_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"

/*
Optional firing modes:
Autocannons:
Rapid-Fire:
std and light ACs only
fires like ultra
fails on roll of 2-4
failure results in 1 round in chamber exploding
explosion does not cause MW damage
MGs
Rapid-Fire
once set can't unset (lame?)
roll 1d6
result is heat generated
result is damage inflicted
ammo == damage * 3
LRMs
Hotloading
no min range
roll 3d6 for number of missiles hit and take lowest two rolls
can not un-hotload
critical hit to launcher destroys launcher and does damage equal to one flight
of missiles roll 2d6. On roll of 2-5 launcher explosion triggers ammo boom of
launcher's ammo that's in the same loc as the launcher. PPC Disengage field
inhibitor removes min range roll 2d6 for feed back check and refer to chart
below. If failure, mech takes 10 points of internal damage to loc of PPC Target
distance Avoid feedback on: 1 10+ 2 6+ 3 3+
*/
void mech_target(DbRef player, Mech *mech, char *buffer) {
  BtechContext *context = mech_context(mech);
  Mech *target;
  char *args[5];
  int argc;
  char section[50];
  UnitClass type;
  MechMovementType movement_type;
  int index;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  argc = mech_parseattributes(buffer, args, 5);
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments to function!");
    return;
  }
  if (!strcmp(args[0], "-")) {
    mech_targeting_aim_reset(mech);
    mecha_notify(btech_context_evaluation(context), player,
                 "Targetting disabled.");
    return;
  }
  if (mech_target_dbref(mech) < 0) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Error: You need to be locked onto something to target its part!");
    return;
  }
  target = btech_context_find_object(context, mech_target_dbref(mech));
  if (!target) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Error: You need to be locked onto something to target its part!");
    return;
  }
  type = mech_class(target);
  movement_type = mech_movement_type(target);
  index = armor_section_from_string(type, movement_type, args[0]);
  if (index < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid location!");
    return;
  }
  mech_targeting_aim_set(
      mech, (MechAimSelection){.section = index, .unit_class = type});
  armor_string_from_index(index, section, type, movement_type);
  notify_printf(btech_context_evaluation(context), player, "%s targetted.",
                section);
}

/* Varying messages based on the distance to foe, and size of your vehicle
   vs size of the guy targeting you: */

/*-20 (or less), -15 to 15, and 20+ ton difference (targetertons - yourtons)*/

/*Distance: <9, <20, rest */

/* Idea: Tonseverity + 3 * distseverity */
static const char *const SS_MESSAGES[] = {
    "You feel you'll have your hands full before too long..",
    "You have a bad feeling about this..",
    "You feel a homicidal maniac is about to pounce on you!",

    "You think something is amiss..",
    "You have a slightly bad feeling about this..",
    "You think someone thinks ill of you..",

    "Something makes you somewhat feel uneasy..",
    "Something makes you definitely feel uneasy..",
    "Something makes you feel out of your element.."};

static int sixth_sense_distance_severity(float range) {
  if (range < 9)
    return 0;
  return range < 20 ? 1 : 2;
}

static int sixth_sense_tonnage_severity(int difference) {
  if (difference <= -20)
    return 0;
  return difference >= 20 ? 2 : 1;
}

static void mech_ss_event(MuxEvent *ev) {
  Mech *mech = (Mech *)ev->data;
  const int I = (int)(intptr_t)ev->data2;

  if (mech_pilot_is_unconscious(mech))
    return;
  if (!mech_has_active_pilot(mech))
    return;
  const char *const *message = (const char *const *)checked_storage_at_const(
      (const void *)SS_MESSAGES, sizeof(SS_MESSAGES) / sizeof(*SS_MESSAGES),
      sizeof(*SS_MESSAGES), (size_t)bounded(0, I, 8));
  mech_notify(mech, MECHPILOT, *message);
}

void mech_sixth_sense_check(Mech *mech, Mech *target) {
  float r;
  int d;

  if (!mech_has_sixth_sense(target) || mech_is_observer(mech))
    return;
  if (mech_is_destroyed(target))
    return;
  if (btech_random_roll(mech_context(mech)) > 8)
    return;
  r = mech_range_to(mech, target);
  d = (mech_real_tonnage(mech) - mech_real_tonnage(target)) / 1024;
  mech_event_schedule(target, EVENT_SS, mech_ss_event,
                      btech_random_range_int(mech_context(mech), 1, 3),
                      (3L * (long)sixth_sense_distance_severity(r)) +
                          (long)sixth_sense_tonnage_severity(d));
}

void mech_set_target(DbRef player, Mech *mech, char *buffer) {
  Mech *target;
  BtechContext *context = mech_context(mech);
  BattleMap *mech_map;
  char *args[5];
  char target_id[2];
  int argc;
  int los = 1;
  int newx;
  int newy;
  DbRef targetref;
  MechStatus mode;

  if (!common_checks(player, mech, MECH_USUALO))
    return;

  argc = mech_parseattributes(buffer, args, 5);
  switch (argc) {
  case 1:
    char *const *first_argument = (char *const *)checked_storage_at_const(
        (const void *)args, sizeof(args) / sizeof(*args), sizeof(*args), 0);
    mech_map = btech_context_get_map(context, mech_map_dbref(mech));
    if (**first_argument == '-') {
      mech_targeting_target_clear(mech);
      mech_notify(mech, MECHALL, "All locks cleared.");
      mech_stop_lock(mech);
      if (mech_spotter_dbref(mech) == mech_dbref(mech))
        mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
      return;
    }
    target_id[0] = *checked_string_suffix(*first_argument, 0);
    target_id[1] = *checked_string_suffix(*first_argument, 1);
    targetref = find_target_dbref_from_map_number(mech, target_id);
    target = btech_context_get_mech(context, targetref);
    if (target)
      los =
          mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), mech_range_to(mech, target));
    else
      targetref = -1;
    if (targetref == -1 || !los) {
      mecha_notify(btech_context_evaluation(context), player,
                   "That is not a valid targetID. Try again.");
      return;
    }

    if (mech_condition_summary(mech).swarm_target > 0) {
      if (mech_condition_summary(mech).swarm_target != mech_dbref(target)) {
        mech_notify(
            mech, MECHALL,
            "You're a bit too busy holding on for dear life to lock a target!");
        return;
      }
    }

    mech_printf(mech, MECHALL, "Target set to %s.",
                mech_to_mech_display_id(mech, target).text);
    mech_stop_lock(mech);
    mech_targeting_unit_set(mech, targetref);
    mech_sixth_sense_check(mech, target);
    if (!btech_context_overrides_weapon_arcs(context))
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, SENSOR_LOCK_TICK,
                          0);
    break;
  case 2:
    /* Targetted a square */
    if (mech_condition_summary(mech).swarm_target > 0) {
      mech_notify(
          mech, MECHALL,
          "You're a bit too busy holding on for dear life to lock a target!");
      return;
    }

    mech_map = btech_context_get_map(context, mech_map_dbref(mech));
    if (!parse_int_checked(args[0], &newx) ||
        !parse_int_checked(args[1], &newy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    if (!battle_map_coordinate_is_valid(mech_map, newx, newy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Illegal coordinates!");
      return;
    }
    mech_targeting_hex_xy_set(mech, newx, newy);
    if (mech_spotter_dbref(mech) == mech_dbref(mech))
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    mech_target_hex_z_set(mech, battle_map_hex_elevation(mech_map, newx, newy));
    notify_printf(btech_context_evaluation(context), player,
                  "Target coordinates set at (X,Y) %d, %d", newx, newy);
    mech_stop_lock(mech);
    mech_targeting_lock_mode_add(mech, MECH_STATUS_LOCK_TARGET);
    if (!btech_context_overrides_weapon_arcs(context))
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, SENSOR_LOCK_TICK,
                          0);
    break;
  case 3:
    /* Targetted a square w/ special mode (hex / building) */
    if (mech_condition_summary(mech).swarm_target > 0) {
      mech_notify(
          mech, MECHALL,
          "You're a bit too busy holding on for dear life to lock a target!");
      return;
    }

    if (strlen(args[2]) > 1) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid lock mode!");
      return;
    }
    switch (ascii_to_upper(*checked_string_suffix(args[2], 0))) {
    case 'B':
      mode = MECH_STATUS_LOCK_BUILDING;
      break;
    case 'I':
      mode = MECH_STATUS_LOCK_HEX_IGN;
      break;
    case 'C':
      mode = MECH_STATUS_LOCK_HEX_CLR;
      break;
    case 'H':
      mode = MECH_STATUS_LOCK_HEX;
      break;
    default:
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid mode selected!");
      return;
    }
    mech_map = btech_context_get_map(context, mech_map_dbref(mech));
    if (!parse_int_checked(args[0], &newx) ||
        !parse_int_checked(args[1], &newy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid coordinates!");
      return;
    }
    if (!battle_map_coordinate_is_valid(mech_map, newx, newy)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Illegal coordinates!");
      return;
    }
    mech_targeting_hex_xy_set(mech, newx, newy);
    if (mech_spotter_dbref(mech) == mech_dbref(mech))
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    mech_target_hex_z_set(mech, battle_map_hex_elevation(mech_map, newx, newy));
    switch ((int)mode) {
    case MECH_STATUS_LOCK_HEX:
      notify_printf(btech_context_evaluation(context), player,
                    "Target coordinates set to hex at (X,Y) %d, %d", newx,
                    newy);
      break;
    case MECH_STATUS_LOCK_HEX_CLR:
      notify_printf(btech_context_evaluation(context), player,
                    "Target coordinates set to clearing hex at (X,Y) %d, %d",
                    newx, newy);
      break;
    case MECH_STATUS_LOCK_HEX_IGN:
      notify_printf(btech_context_evaluation(context), player,
                    "Target coordinates set to igniting hex at (X,Y) %d, %d",
                    newx, newy);
      break;
    default:
      notify_printf(btech_context_evaluation(context), player,
                    "Target coordinates set to building at (X,Y) %d, %d", newx,
                    newy);
      break;
    }

    mech_stop_lock(mech);
    mech_targeting_lock_mode_add(mech, mode);
    if (!btech_context_overrides_weapon_arcs(context))
      mech_event_schedule(mech, EVENT_LOCK, mech_lock_event, SENSOR_LOCK_TICK,
                          0);
  }
}

/*
 * Fire weapon command handler
 */
