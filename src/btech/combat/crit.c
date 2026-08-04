/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 */

#include <math.h>
#include <stdio.h>
#include <string.h>

#include "btconfig.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "crit_api.h"
#include "econ_cmds_api.h"
#include "eject_api.h"
#include "failures.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_lifecycle.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_sensor.h"
#include "mech_tag_api.h"
#include "mech_tech_commands_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "missile_hit_registry.h"
#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "random.h"
#include "registry_api.h"

void correct_speed(Mech *mech) {
  float maxspeed = MMaxSpeed(mech);
  int neg = 1;

  if (MechMaxSpeed(mech) < 0.0)
    MechMaxSpeed(mech) = 0.0;
  SetCargoWeight(mech);
  if (MechDesiredSpeed(mech) < -0.1) {
    maxspeed = maxspeed * 2.0 / 3.0;
    neg = -1;
  }
  if (fabs(MechDesiredSpeed(mech)) > maxspeed)
    MechDesiredSpeed(mech) = (float)maxspeed * neg;

  if (fabs(MechSpeed(mech)) > maxspeed)
    MechSpeed(mech) = (float)maxspeed * neg;
}

void explode_unit(Mech *wounded, Mech *attacker) {
  int j;
  Mech *target;
  DbRef i, tmpnext;
  DbRef from;

  from = wounded->mynum;

  SAFE_DOLIST(wounded->xcode.context->database, i, tmpnext,
              game_object_contents(wounded->xcode.context->database, from)) {
    if (is_good_obj(wounded->xcode.context->database, i) &&
        is_xcode(wounded->xcode.context->database, i)) {
      if ((target = btech_context_get_mech(wounded->xcode.context, i))) {
        if (MechType(target) == CLASS_BSUIT) {
          KillMechContentsIfIC(target);
          discard_mw(target);
        }
      }
    }
  }

  KillMechContentsIfIC(wounded);
  for (j = 0; j < NUM_SECTIONS; j++) {
    if (GetSectOInt(wounded, j) && !SectIsDestroyed(wounded, j))
      DestroySection(wounded, attacker, wounded == attacker ? 0 : 1, j);
  }
}

void NormalizeArmActuatorCrits(Mech *objMech, int wLoc, int wCritType) {
  switch (Special2I(wCritType)) {
  case SHOULDER_OR_HIP:
    /* +4 to BTH with weapons in arm */
    MechSections(objMech)[wLoc].basetohit = 4;
    break;

  case UPPER_ACTUATOR:
  case LOWER_ACTUATOR:
    /* +1 BTH */
    MechSections(objMech)[wLoc].basetohit += 1;
    break;
  }
}

void NormalizeLegActuatorCrits(Mech *objMech, int wLoc, int wCritType) {
  switch (Special2I(wCritType)) {
  case SHOULDER_OR_HIP:
    /*
       speed cut in half
       +2 to pskill rolls
       2nd crit == zero speed on bipeds, but not on quads. Cut current speed in
       half again
     */
    mech_max_speed_divide(objMech, 2);
    MechPilotSkillBase(objMech) += 2;
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
    MechSections(objMech)[wLoc].basetohit += 1;
    MechPilotSkillBase(objMech) += 1;
    break;
  }
}

void NormalizeLocActuatorCrits(Mech *objMech, int wLoc) {
  int wCritType;
  int tIsArm = 0;
  int tHasShoulderOrHipCrit = 0;
  int i;

  if (!MechIsQuad(objMech) && ((wLoc == LARM) || (wLoc == RARM)))
    tIsArm = 1;

  /* reset the BTHs for this section */
  MechSections(objMech)[wLoc].basetohit = 0;

  /* Let's first check to see if we have a shoulder or hip crit. If we do, then
   * we ignore all the other mods */
  for (i = 0; i < NUM_CRITICALS; i++) {
    wCritType = GetPartType(objMech, wLoc, i);

    if (PartIsDestroyed(objMech, wLoc, i)) {
      if (IsSpecial(wCritType)) {
        switch (Special2I(wCritType)) {
        case SHOULDER_OR_HIP:
          tHasShoulderOrHipCrit = 1;

          if (tIsArm)
            NormalizeArmActuatorCrits(objMech, wLoc, wCritType);
          else
            NormalizeLegActuatorCrits(objMech, wLoc, wCritType);

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
      wCritType = GetPartType(objMech, wLoc, i);

      if (PartIsDestroyed(objMech, wLoc, i)) {

        if (IsSpecial(wCritType)) {

          switch (Special2I(wCritType)) {
          case UPPER_ACTUATOR:
          case LOWER_ACTUATOR:
          case HAND_OR_FOOT_ACTUATOR:
            if (tIsArm)
              NormalizeArmActuatorCrits(objMech, wLoc, wCritType);
            else
              NormalizeLegActuatorCrits(objMech, wLoc, wCritType);

            break;
          }
        }
      }
    }
  }

  correct_speed(objMech);
}

/*
        This function will reset all pskill mods and BTH
        mods and attempt to 'correct' them as the current code
        is anything but correct.
*/

void NormalizeAllActuatorCrits(Mech *objMech) {
  int wLegsDestroyed = CountDestroyedLegs(objMech);

  /* reset us back to zero */
  MechPilotSkillBase(objMech) = 0;

  mech_max_speed_set(objMech, TemplateMaxSpeed(objMech));

  /*
     The problem here is all the calcs are based on running speed... ie, max
     speed. This is lame 'cause it makes EVERYTHING wrong. When you subtract 1
     point of speed, if should come off the walking speed and the running should
     be recal'd from there. Ah well, we leave it as it is now and fix it later.
   */

  /* If we have a gyro crit, add 3 to our skill */
  /* Hardened gyro is a +2 on first hit */
  if (MechSpecials2(objMech) & HDGYRO_TECH) {
    if (MechCritStatus2(objMech) & HDGYRO_DAMAGED) {
      if (MechCritStatus(objMech) & GYRO_DAMAGED) {
        MechPilotSkillBase(objMech) += 3;
      } else {
        MechPilotSkillBase(objMech) += 2;
      }
    }

  } else if (MechCritStatus(objMech) & GYRO_DAMAGED)
    MechPilotSkillBase(objMech) += 3;

  /*
     Let's add in the appropriate modifiers for a dead leg.
     ie. add 5 to the pskill BTH for each dead leg
   */
  if (wLegsDestroyed > 0) {
    if (MechIsQuad(objMech)) {
      MechPilotSkillBase(objMech) += 2; /* loose quad bonus */

      switch (wLegsDestroyed) {
      case 1:
        mech_max_speed_lower(objMech, MP1);
        break;

      case 2:
        mech_max_speed_set(objMech, MP1);
        MechPilotSkillBase(objMech) += 5;
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
        MechPilotSkillBase(objMech) += 5;
      } else {
        mech_max_speed_set(objMech, 0.0);
        MechPilotSkillBase(objMech) += 10;
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
    NormalizeLocActuatorCrits(objMech, LARM);

  if (!IsLegDestroyed(objMech, RARM))
    NormalizeLocActuatorCrits(objMech, RARM);

  if (!IsLegDestroyed(objMech, LLEG))
    NormalizeLocActuatorCrits(objMech, LLEG);

  if (!IsLegDestroyed(objMech, RLEG))
    NormalizeLocActuatorCrits(objMech, RLEG);

  /*
     Once were done, we just gotta fix one thing.
     If both of our hips are marked as destroyed (on a BIPED) then we set our
     speed to zero.
   */
  if (MechCritStatus(objMech) & HIP_DESTROYED) {
    mech_max_speed_set(objMech, 0.0);
    mech_make_fall(objMech);
  }

  correct_speed(objMech);
}
