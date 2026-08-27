/* Implements autonomous physical attacks. */

#include <stdio.h>
#include <string.h>

#include "autopilot.h"
#include "autopilot_autogun_api.h"
#include "autopilot_combat_policy_api.h"
#include "equipment_types.h"
#include "map_coordinates.h"
#include "map_los_api.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "registry_api.h"
#include "section_types.h"

static void format_physical_command(char buffer[static LBUF_SIZE], char side,
                                    const Mech *target) {
  MechUnitId id = mech_unit_id(target);
  (void)snprintf(buffer, LBUF_SIZE, "%c %c%c", side, id.first, id.second);
}

typedef struct AutogunPhysicalTargetSelection {
  Mech *physical_target;
  Mech *last_target;
} AutogunPhysicalTargetSelection;

static float autogun_physical_range(const Mech *mech, const Mech *target) {
  return map_real_range(&(MapRealSegment){
      .start = {.x = mech_position_real_x(mech),
                .y = mech_position_real_y(mech)},
      .end = {.x = mech_position_real_x(target),
              .y = mech_position_real_y(target)},
  });
}

static AutogunPhysicalTargetSelection
autogun_physical_target_select(Mech *mech, BattleMap *map, Mech *target) {
  const float RANGE = autogun_physical_range(mech, target);
  if (RANGE < 1.0F)
    return (AutogunPhysicalTargetSelection){target, target};
  if (RANGE <= AUTO_GUN_PHYSICAL_RANGE_MIN)
    return (AutogunPhysicalTargetSelection){nullptr, target};

  Mech *last_target = target;
  for (int slot = 0; slot < battle_map_unit_count(map); slot++) {
    if (slot == mech_map_slot(mech))
      continue;
    const DbRef DBREF = battle_map_unit_dbref(map, slot);
    if (DBREF <= 0)
      continue;
    last_target = btech_context_get_mech(mech_context(mech), DBREF);
    if (last_target == nullptr || mech_is_destroyed(last_target) ||
        mech_condition_summary(last_target).combat_safe ||
        mech_team(last_target) == mech_team(mech))
      continue;
    if (autogun_physical_range(mech, last_target) < 1.0F)
      return (AutogunPhysicalTargetSelection){last_target, last_target};
  }
  return (AutogunPhysicalTargetSelection){nullptr, last_target};
}

typedef struct AutogunPhysicalArcs {
  int original;
  int aligned;
} AutogunPhysicalArcs;

static AutogunPhysicalArcs autogun_physical_align(Mech *mech,
                                                  const Mech *target) {
  mech_torso_twist_set(mech, MECH_TORSO_CENTER);
  if (mech_technology_flags(mech) & FLIPABLE_ARMS)
    mech_arms_center(mech);

  const int ORIGINAL = in_weapon_arc(mech, mech_position_real_x(target),
                                     mech_position_real_y(target));
  if (ORIGINAL & LSIDEARC) {
    mech_torso_twist_set(mech, MECH_TORSO_LEFT);
  } else if (ORIGINAL & RSIDEARC) {
    mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
  } else if (ORIGINAL & REARARC) {
    const int RELATIVE_BEARING =
        mech_heading_degrees(mech) -
        map_bearing(
            &(MapRealSegment){.start = {.x = mech_position_real_x(mech),
                                        .y = mech_position_real_y(mech)},
                              .end = {.x = mech_position_real_x(target),
                                      .y = mech_position_real_y(target)}});
    if (RELATIVE_BEARING > 120 && RELATIVE_BEARING < 180)
      mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
    else if (RELATIVE_BEARING > 180 && RELATIVE_BEARING < 240)
      mech_torso_twist_set(mech, MECH_TORSO_LEFT);
  }
  return (AutogunPhysicalArcs){
      .original = ORIGINAL,
      .aligned = in_weapon_arc(mech, mech_position_real_x(target),
                               mech_position_real_y(target))};
}

void autogun_physical_attack(Autopilot *autopilot, Mech *mech, BattleMap *map,
                             Mech *target) {
  Mech *physical_target;
  char buffer[LBUF_SIZE];
  int elevation_diff;
  int what_arc;
  int new_arc;
  bool is_section_destroyed[4];
  bool section_hasbusyweap[4];
  int rleg_bth;
  int lleg_bth;
  bool is_rarm_ready;
  bool is_larm_ready;

  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - Start Physical Attack Stage");

  const AutogunPhysicalTargetSelection SELECTION =
      autogun_physical_target_select(mech, map, target);
  physical_target = SELECTION.physical_target;
  target = SELECTION.last_target;

  /* Now nail it with a physical attack but only if we see it */
  if (physical_target && battle_map_unit_is_seen(map, mech, physical_target)) {

    /* Log It */
    autopilot_autogun_log(autopilot,
                          "Autogun - Attempting physical attack against"
                          " target #%ld",
                          mech_dbref(physical_target));

    /* Calculate elevation difference */
    elevation_diff = mech_position_z(mech) - mech_position_z(target);

    /* Are we a biped Mech */
    if ((mech_class(mech) == CLASS_MECH) &&
        (mech_movement_type(mech) == MOVE_BIPED)) {

      const AutogunPhysicalArcs ARCS =
          autogun_physical_align(mech, physical_target);
      what_arc = ARCS.original;
      new_arc = ARCS.aligned;

      /* Check to see what sections are destroyed */
      is_section_destroyed[0] = mech_section_is_destroyed(mech, RARM);
      is_section_destroyed[1] = mech_section_is_destroyed(mech, LARM);
      is_section_destroyed[2] = mech_section_is_destroyed(mech, RLEG);
      is_section_destroyed[3] = mech_section_is_destroyed(mech, LLEG);

      /* Check to see if the sections have a busy weapon */
      section_hasbusyweap[0] = sect_has_busy_weap(mech, RARM);
      section_hasbusyweap[1] = sect_has_busy_weap(mech, LARM);
      section_hasbusyweap[2] = sect_has_busy_weap(mech, RLEG);
      section_hasbusyweap[3] = sect_has_busy_weap(mech, LLEG);

      /* Try weapon physical attacks */

      /* Right Arm */
      if (!is_section_destroyed[0] && !section_hasbusyweap[0] &&
          !mech_limbs_are_recycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & RSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        format_physical_command(buffer, 'r', physical_target);

        if (have_axe(mech, RARM))
          mech_axe(autopilot->mynum, mech, buffer);
        /* else if (have_mace(mech, RARM)) */
        /*! \todo {Add in mace code here} */
        else if (have_sword(mech, RARM))
          mech_sword(autopilot->mynum, mech, buffer);
      }

      /* Left Arm */
      if (!is_section_destroyed[1] && !section_hasbusyweap[1] &&
          !mech_limbs_are_recycling(mech) &&
          ((new_arc & FORWARDARC) || (new_arc & LSIDEARC)) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        format_physical_command(buffer, 'l', physical_target);

        if (have_axe(mech, LARM))
          mech_axe(autopilot->mynum, mech, buffer);
        /* else if (have_mace(mech, RARM)) */
        /*! \todo {Add in mace code here} */
        else if (have_sword(mech, LARM))
          mech_sword(autopilot->mynum, mech, buffer);
      }

      /* Try and kick but only if we got two legs, one of them
       * doesn't have a cycling weapon and the target is in the
       * front arc */
      if ((!section_hasbusyweap[2] || !section_hasbusyweap[3]) &&
          !is_section_destroyed[2] && !is_section_destroyed[3] &&
          (what_arc & FORWARDARC) && !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the RLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[2]) {
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RLEG, .critical = 0},
                  .special = SHOULDER_OR_HIP}))
            rleg_bth += 3;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RLEG, .critical = 1},
                  .special = UPPER_ACTUATOR}))
            rleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RLEG, .critical = 2},
                  .special = LOWER_ACTUATOR}))
            rleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RLEG, .critical = 3},
                  .special = HAND_OR_FOOT_ACTUATOR}))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the LLEG for any crits or weaps cycling */
        if (!section_hasbusyweap[3]) {
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LLEG, .critical = 0},
                  .special = SHOULDER_OR_HIP}))
            lleg_bth += 3;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LLEG, .critical = 1},
                  .special = UPPER_ACTUATOR}))
            lleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LLEG, .critical = 2},
                  .special = LOWER_ACTUATOR}))
            lleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LLEG, .critical = 3},
                  .special = HAND_OR_FOOT_ACTUATOR}))
            lleg_bth++;
        } else {
          lleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        const AutopilotPhysicalSide KICK = autopilot_physical_choose_leg(
            (!section_hasbusyweap[2]) != 0, rleg_bth,
            (!section_hasbusyweap[3]) != 0, lleg_bth);
        if (KICK == AUTOPILOT_PHYSICAL_RIGHT) {
          format_physical_command(buffer, 'r', physical_target);
        } else if (KICK == AUTOPILOT_PHYSICAL_LEFT) {
          format_physical_command(buffer, 'l', physical_target);
        }
        if (KICK != AUTOPILOT_PHYSICAL_NONE)
          mech_kick(autopilot->mynum, mech, buffer);
      }

      /* Finally try to punch */
      if (((!is_section_destroyed[0] && !section_hasbusyweap[0]) ||
           (!is_section_destroyed[1] && !section_hasbusyweap[1])) &&
          !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == -1)) {

        is_rarm_ready = false;
        is_larm_ready = false;

        if (!is_section_destroyed[0] && !section_hasbusyweap[0] &&
            ((new_arc & FORWARDARC) || (new_arc & RSIDEARC))) {

          /* We can use the right arm */
          is_rarm_ready = true;
        }

        if (!is_section_destroyed[1] && !section_hasbusyweap[1] &&
            ((new_arc & FORWARDARC) || (new_arc & LSIDEARC))) {

          /* We can use the left arm */
          is_larm_ready = true;
        }

        const AutopilotPhysicalSide PUNCH =
            autopilot_physical_choose_punch(is_rarm_ready, is_larm_ready);
        if (PUNCH == AUTOPILOT_PHYSICAL_BOTH) {
          format_physical_command(buffer, 'b', physical_target);
        } else if (PUNCH == AUTOPILOT_PHYSICAL_RIGHT) {
          format_physical_command(buffer, 'r', physical_target);
        } else if (PUNCH == AUTOPILOT_PHYSICAL_LEFT) {
          format_physical_command(buffer, 'l', physical_target);
        }

        /* Now punch */
        if (PUNCH != AUTOPILOT_PHYSICAL_NONE)
          mech_punch(autopilot->mynum, mech, buffer);
      }

    } else if ((mech_class(mech) == CLASS_MECH) &&
               (mech_movement_type(mech) == MOVE_QUAD)) {

      /* Quad Mech - Right now only supporting kicking front style for quad */
      /* Remember, the RARM becomes the Front RLEG and the LARM becomes the
       * Front LLEG */

      /* Find direction of bad guy */
      what_arc = in_weapon_arc(mech, mech_position_real_x(physical_target),
                               mech_position_real_y(physical_target));

      /* Check to see what sections are destroyed */
      is_section_destroyed[0] = mech_section_is_destroyed(mech, RARM);
      is_section_destroyed[1] = mech_section_is_destroyed(mech, LARM);
      is_section_destroyed[2] = mech_section_is_destroyed(mech, RLEG);
      is_section_destroyed[3] = mech_section_is_destroyed(mech, LLEG);

      /* Check to see if the sections have a busy weapon */
      memset(section_hasbusyweap, 0, sizeof(section_hasbusyweap));
      section_hasbusyweap[0] = sect_has_busy_weap(mech, RARM);
      section_hasbusyweap[1] = sect_has_busy_weap(mech, LARM);
      // section_hasbusyweap[2] = SectHasBusyWeap(mech, RLEG);
      // section_hasbusyweap[3] = SectHasBusyWeap(mech, LLEG);

      /* Try and kick but only if we got two legs, one of them
       * doesn't have a cycling weapon and the target is in the
       * front arc */
      if ((!section_hasbusyweap[0] || !section_hasbusyweap[1]) &&
          !is_section_destroyed[0] && !is_section_destroyed[1] &&
          !is_section_destroyed[2] && !is_section_destroyed[3] &&
          (what_arc & FORWARDARC) && !mech_limbs_are_recycling(mech) &&
          (elevation_diff == 0 || elevation_diff == 1)) {

        rleg_bth = 0;
        lleg_bth = 0;

        /* Check the Front Right Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[0]) {
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RARM, .critical = 0},
                  .special = SHOULDER_OR_HIP}))
            rleg_bth += 3;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RARM, .critical = 1},
                  .special = UPPER_ACTUATOR}))
            rleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RARM, .critical = 2},
                  .special = LOWER_ACTUATOR}))
            rleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = RARM, .critical = 3},
                  .special = HAND_OR_FOOT_ACTUATOR}))
            rleg_bth++;
        } else {
          rleg_bth = 99;
        }

        /* Check the Front Left Leg for any crits or weaps cycling */
        if (!section_hasbusyweap[1]) {
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LARM, .critical = 0},
                  .special = SHOULDER_OR_HIP}))
            lleg_bth += 3;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LARM, .critical = 1},
                  .special = UPPER_ACTUATOR}))
            lleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LARM, .critical = 2},
                  .special = LOWER_ACTUATOR}))
            lleg_bth++;
          if (!mech_critical_is_operational_special(&(CriticalSpecialCheck){
                  .mech = mech,
                  .slot = {.section = LARM, .critical = 3},
                  .special = HAND_OR_FOOT_ACTUATOR}))
            lleg_bth++;
        } else {
          lleg_bth = 99;
        }

        /* Now kick depending on which one would be better
         * to kick with */
        const AutopilotPhysicalSide KICK = autopilot_physical_choose_leg(
            (!section_hasbusyweap[0]) != 0, rleg_bth,
            (!section_hasbusyweap[1]) != 0, lleg_bth);
        if (KICK == AUTOPILOT_PHYSICAL_RIGHT) {
          format_physical_command(buffer, 'r', physical_target);
        } else if (KICK == AUTOPILOT_PHYSICAL_LEFT) {
          format_physical_command(buffer, 'l', physical_target);
        }
        if (KICK != AUTOPILOT_PHYSICAL_NONE)
          mech_kick(autopilot->mynum, mech, buffer);
      }

    } else if (mech_class(mech) == CLASS_BSUIT) {

      /* Are we a BSuit */

    } else {

      /* Eventually add code here maybe for other physical attacks for
       * tanks perhaps */
    }
  }

  /* End of physical attack */
  /* Log It */
  autopilot_autogun_log(autopilot, "Autogun - End Physical Attack Stage");
}
