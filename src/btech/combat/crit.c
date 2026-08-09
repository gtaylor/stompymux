/* Implements BattleTech combat mechanics for crit. */

#include <math.h>

#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_position_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "registry_api.h"
#include "section_types.h"

void mech_speed_correct(Mech *mech) {
  float maxspeed = mech_cargo_maximum_speed(mech, mech_maximum_speed(mech));
  float direction = 1.0F;

  if (mech_maximum_speed(mech) < 0.0F)
    mech_maximum_speed_set(mech, 0.0F);
  mech_cargo_weight_recalculate(mech);
  if (mech_desired_speed(mech) < -0.1F) {
    maxspeed = maxspeed * 2.0F / 3.0F;
    direction = -1.0F;
  }
  if (fabsf(mech_desired_speed(mech)) > maxspeed)
    mech_desired_speed_set(mech, maxspeed * direction);

  if (fabsf(mech_current_speed(mech)) > maxspeed)
    mech_current_speed_set(mech, maxspeed * direction);
}

void mech_explosion_apply(Mech *wounded, Mech *attacker) {
  int j;
  Mech *target;
  DbRef i, tmpnext;
  DbRef from;
  BtechContext *context = mech_context(wounded);
  GameDatabase *database = btech_context_database(context);

  from = mech_dbref(wounded);

  SAFE_DOLIST(database, i, tmpnext, game_object_contents(database, from)) {
    if (is_good_obj(database, i) && is_xcode(database, i)) {
      if ((target = btech_context_get_mech(context, i))) {
        if (mech_class(target) == CLASS_BSUIT) {
          mech_contents_kill_if_in_character(target);
          discard_mw(target);
        }
      }
    }
  }

  mech_contents_kill_if_in_character(wounded);
  for (j = 0; j < NUM_SECTIONS; j++) {
    if (mech_section_original_internal(wounded, j) &&
        !mech_section_is_destroyed(wounded, j))
      mech_section_destroy(wounded, attacker, wounded == attacker ? 0 : 1, j);
  }
}

void mech_arm_actuator_criticals_normalize(Mech *objMech, int wLoc,
                                           int wCritType) {
  switch (special_from_equipment_index(wCritType)) {
  case SHOULDER_OR_HIP:
    /* +4 to BTH with weapons in arm */
    mech_section_base_to_hit_set(objMech, wLoc, 4);
    break;

  case UPPER_ACTUATOR:
  case LOWER_ACTUATOR:
    /* +1 BTH */
    mech_section_base_to_hit_add(objMech, wLoc, 1);
    break;
  }
}

void mech_leg_actuator_criticals_normalize(Mech *objMech, int wLoc,
                                           int wCritType) {
  switch (special_from_equipment_index(wCritType)) {
  case SHOULDER_OR_HIP:
    /*
       speed cut in half
       +2 to pskill rolls
       2nd crit == zero speed on bipeds, but not on quads. Cut current speed in
       half again
     */
    mech_max_speed_divide(objMech, 2);
    mech_pilot_skill_modifier_add(objMech, 2);
    break;

  case UPPER_ACTUATOR:
  case LOWER_ACTUATOR:
  case HAND_OR_FOOT_ACTUATOR:
    /*
       -1 walking
       +1 to pskill rolls
       +1 to BTHs with this leg
     */
    mech_max_speed_lower(objMech, MP1);
    mech_section_base_to_hit_add(objMech, wLoc, 1);
    mech_pilot_skill_modifier_add(objMech, 1);
    break;
  }
}

void mech_section_actuator_criticals_normalize(Mech *objMech, int wLoc) {
  int wCritType;
  int tIsArm = 0;
  int tHasShoulderOrHipCrit = 0;
  int i;

  if (!mech_is_quad(objMech) && ((wLoc == LARM) || (wLoc == RARM)))
    tIsArm = 1;

  /* reset the BTHs for this section */
  mech_section_base_to_hit_set(objMech, wLoc, 0);

  /* Let's first check to see if we have a shoulder or hip crit. If we do, then
   * we ignore all the other mods */
  for (i = 0; i < NUM_CRITICALS; i++) {
    wCritType = mech_critical_part_type(objMech, wLoc, i);

    if (mech_critical_is_destroyed(objMech, wLoc, i)) {
      if (equipment_is_special(wCritType)) {
        switch (special_from_equipment_index(wCritType)) {
        case SHOULDER_OR_HIP:
          tHasShoulderOrHipCrit = 1;

          if (tIsArm)
            mech_arm_actuator_criticals_normalize(objMech, wLoc, wCritType);
          else
            mech_leg_actuator_criticals_normalize(objMech, wLoc, wCritType);

          break;
        }
      }
    }

    if (tHasShoulderOrHipCrit)
      break;
  }

  /* Now, we only check the rest of the crits if we don't have a shoulder/hip */
  if (!tHasShoulderOrHipCrit) {

    for (i = 0; i < NUM_CRITICALS; i++) {
      wCritType = mech_critical_part_type(objMech, wLoc, i);

      if (mech_critical_is_destroyed(objMech, wLoc, i)) {

        if (equipment_is_special(wCritType)) {

          switch (special_from_equipment_index(wCritType)) {
          case UPPER_ACTUATOR:
          case LOWER_ACTUATOR:
          case HAND_OR_FOOT_ACTUATOR:
            if (tIsArm)
              mech_arm_actuator_criticals_normalize(objMech, wLoc, wCritType);
            else
              mech_leg_actuator_criticals_normalize(objMech, wLoc, wCritType);

            break;
          }
        }
      }
    }
  }

  mech_speed_correct(objMech);
}

/*
        This function will reset all pskill mods and BTH
        mods and attempt to 'correct' them as the current code
        is anything but correct.
*/

void mech_actuator_criticals_normalize(Mech *objMech) {
  int wLegsDestroyed = CountDestroyedLegs(objMech);
  MechConditionSummary condition = mech_condition_summary(objMech);

  /* reset us back to zero */
  mech_pilot_skill_modifier_set(objMech, 0);

  mech_max_speed_set(objMech, mech_template_maximum_speed(objMech));

  /*
     The problem here is all the calcs are based on running speed... ie, max
     speed. This is lame 'cause it makes EVERYTHING wrong. When you subtract 1
     point of speed, if should come off the walking speed and the running should
     be recal'd from there. Ah well, we leave it as it is now and fix it later.
   */

  /* If we have a gyro crit, add 3 to our skill */
  /* Hardened gyro is a +2 on first hit */
  if (mech_technology_flags_secondary(objMech) & HDGYRO_TECH) {
    if (condition.hardened_gyro_damaged) {
      if (condition.gyro_damaged) {
        mech_pilot_skill_modifier_add(objMech, 3);
      } else {
        mech_pilot_skill_modifier_add(objMech, 2);
      }
    }

  } else if (condition.gyro_damaged)
    mech_pilot_skill_modifier_add(objMech, 3);

  /*
     Let's add in the appropriate modifiers for a dead leg.
     ie. add 5 to the pskill BTH for each dead leg
   */
  if (wLegsDestroyed > 0) {
    if (mech_is_quad(objMech)) {
      mech_pilot_skill_modifier_add(objMech, 2); /* loose quad bonus */

      switch (wLegsDestroyed) {
      case 1:
        mech_max_speed_lower(objMech, MP1);
        break;

      case 2:
        mech_max_speed_set(objMech, MP1);
        mech_pilot_skill_modifier_add(objMech, 5);
        break;

      case 3:
      case 4:
        mech_max_speed_set(objMech, 0.0);
        mech_make_fall(objMech);
        ;
        break;
      }
    } else {
      if (wLegsDestroyed == 1) {
        mech_max_speed_set(objMech, MP1);
        mech_pilot_skill_modifier_add(objMech, 5);
      } else {
        mech_max_speed_set(objMech, 0.0);
        mech_pilot_skill_modifier_add(objMech, 10);
        mech_make_fall(objMech);
      }
    }
  }

  /*
          For BIPED (done)
                  Leg destroyed (done)
                          add +5 BTH to pskills (done)
                          immediate fall (done)
                          only 1 MP (done)
                          ignore damage in leg (done)
                  No legs (done)
                          +10 BTH to pskills -- like it matters (done)
                          no MP (done)
                          ignore damage in legs (done)

          For QUAD (done -- missing L3 mule kick)
                  No legs destroyed (done -- missing L3 mule kick)
                          -2 pskill BTH bonus (done)
                          no pskill to stand up (done)
                          no BTH mod when firing while down (done)
                          does not need to prop. Fires as though it was
     standing. (done) no torso twist (done) no punch, club, axe, sword, push
     (done) forward kick if no hip crits (done) L3: can mule kick with rear
     levels at anyone in rear arc (pending decision) One leg destroyed (done)
                          auto-fall (done)
                          no lateral (done)
                          loose -2 pskill BTH bonus (done)
                          must make pskill to stand (done)
                          must prop with a forward leg and adds +2 BTH when
     firing (done) -1 MP (done) With two legs destroyed (done) Act as 1 legged
     BIPED (done) immediate fall (done) +5 BTH to pskills (done) 1 MP (done)
                  With 3 or more legs destroyed (done)
                          No movement (done)
                          auto fall (done)
                          MP of 0 (done)
                          Can not prop to fire (done)
  */

  /* Now, normalize our legs and arms */
  if (!IsLegDestroyed(objMech, LARM))
    mech_section_actuator_criticals_normalize(objMech, LARM);

  if (!IsLegDestroyed(objMech, RARM))
    mech_section_actuator_criticals_normalize(objMech, RARM);

  if (!IsLegDestroyed(objMech, LLEG))
    mech_section_actuator_criticals_normalize(objMech, LLEG);

  if (!IsLegDestroyed(objMech, RLEG))
    mech_section_actuator_criticals_normalize(objMech, RLEG);

  /*
     Once were done, we just gotta fix one thing.
     If both of our hips are marked as destroyed (on a BIPED) then we set our
     speed to zero.
   */
  if (condition.hip_destroyed) {
    mech_max_speed_set(objMech, 0.0);
    mech_make_fall(objMech);
  }

  mech_speed_correct(objMech);
}
