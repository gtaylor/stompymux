#include "map_units_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_notify_api.h"
#include "mech_physical_internal.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"

static int physical_forward_arc(Mech *mech, const Mech *target) {
  MechConditionSummary condition = mech_condition_summary(mech);
  mech_torso_twist_set(mech, MECH_TORSO_CENTER);
  int arc = InWeaponArc(mech, mech_position_real_x(target),
                        mech_position_real_y(target));
  if (condition.torso_left)
    mech_torso_twist_set(mech, MECH_TORSO_LEFT);
  else if (condition.torso_right)
    mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
  return arc;
}

void PhysicalAttack(Mech *mech, int damageweight, int baseToHit, int AttackType,
                    int argc, char **args, BattleMap *mech_map, int sect) {
  Mech *target;
  float range;
  float maxRange = 1;
  char targetID[2];
  int targetnum, roll, swarmingUs;
  char location[20];
  int iwa;
  int RbaseToHit, glance = 0;

  /*
   * Common Checks
   */

  // Since we can punch with two arms, often back to back, we want to run
  // these generic checks in mech_punch() -BEFORE- PhysicalAttack() is called
  // twice (if we have two working arms).
  if (AttackType == PA_PUNCH || AttackType == PA_CLAW) {
    // Do Nothing
  } else {
    if (!phys_common_checks(mech))
      return;
  }

  /* BTH Adjustments for crits to limbs - seperate one for
   * club because it checks for both limbs */
  if ((AttackType == PA_PUNCH) || (AttackType == PA_KICK) ||
      (AttackType == PA_AXE) || (AttackType == PA_SWORD) ||
      (AttackType == PA_CLAW) || (AttackType == PA_MACE) ||
      (AttackType == PA_SAW)) {

    if (mech_critical_is_nonfunctional(mech, sect, 1) ||
        mech_critical_part_type(mech, sect, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }

    if (mech_critical_is_nonfunctional(mech, sect, 2) ||
        mech_critical_part_type(mech, sect, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }

    /* Hand/Foot crits only affect punch/kick since with the other attacks
     * are not allowed if they're broken */
    if ((AttackType == PA_PUNCH) || (AttackType == PA_KICK)) {
      if (mech_critical_is_nonfunctional(mech, sect, 3) ||
          mech_critical_part_type(mech, sect, 3) !=
              special_equipment_index(HAND_OR_FOOT_ACTUATOR)) {
        baseToHit += 1;
      }
    }

  } else if (AttackType == PA_CLUB) {

    /* Only check lower/upper acts since without shoulder or hand you can't
     * club */
    if (mech_critical_is_nonfunctional(mech, RARM, 1) ||
        mech_critical_part_type(mech, sect, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, RARM, 2) ||
        mech_critical_part_type(mech, sect, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, LARM, 1) ||
        mech_critical_part_type(mech, sect, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, LARM, 2) ||
        mech_critical_part_type(mech, sect, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }
  }

  // All weapons must be cycled in the target limb.
  if (mech_section_has_recycling_weapon(mech, sect)) {
    ArmorStringFromIndex(sect, location, mech_class(mech),
                         mech_movement_type(mech));
    mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                location);
    return;
  }
  // Figure out what to do with the arguments provided with the physical
  // command.
  switch (argc) {
  case -1:
  case 0:
    // No argument
    if (mech_target_dbref(mech) == -1) {
      mech_notify(mech, MECHALL, "You do not have a target set!");
      return;
    }

    // Populate target variable with current lock.
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(mech));
    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      return;
    }

    break;
  default:
    // In this case, default means user has specified an argument
    // with the physical attack.

    // Populate target variable from user input.
    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);
    target = btech_context_get_mech(mech_context(mech), targetnum);

    if (targetnum == -1) {
      mech_notify(mech, MECHALL, "Target is not in line of sight!");
      return;
    }
    if (!target) {
      mech_notify(mech, MECHALL, "Invalid default target!");
      return;
    }
  } // end switch() - argc checking

  // Is the target swarming us?
  swarmingUs = (mech_swarm_target(target) == mech_dbref(mech) ? 1 : 0);

  /*
   * Common checks.
   */

  // If we're attacking something while fallen that isn't swarming us,
  // no-go it. Kicks/trips are automatically stopped.
  if (mech_condition_summary(mech).fallen &&
      (AttackType == PA_KICK || AttackType == PA_TRIP)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                phys_form(AttackType, 0));

    return;
    // If we are fallen AND
    //   The target is not a BSuit AND We're not punching
  } else if (mech_condition_summary(mech).fallen &&
             (mech_class(target) != CLASS_VEH_GROUND &&
              mech_class(target) != CLASS_BSUIT)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                phys_form(AttackType, 0));

    return;
  } else if (mech_condition_summary(mech).fallen &&
             mech_class(target) == CLASS_BSUIT && !swarmingUs) {
    mech_notify(
        mech, MECHALL,
        "You may only physical suits that are swarming you while prone.");
    return;
  } else if (mech_condition_summary(mech).fallen &&
             mech_class(target) == CLASS_VEH_GROUND && AttackType != PA_PUNCH) {
    mech_notify(mech, MECHALL, "You may only punch vehicles while prone.");
    return;
  } // end if() - Physical while fallen.

  range = mech_range_to(mech, target);

  if (!mech_los_check_unblocked(mech, target, mech_position_x(target),
                                mech_position_y(target), range)) {
    mech_notify(mech, MECHALL, "Target is not in line of sight!");
    return;
  }

  // BSuits have to be <= 0.5 hexes to attack units.
  if ((mech_class(target) == CLASS_BSUIT) || (mech_class(target) == CLASS_MW))
    maxRange = 0.5;

  if (range >= maxRange) {
    mech_notify(mech, MECHALL, "Target out of range!");
    return;
  }

  if (mech_is_jumping(target)) {
    mech_notify(mech, MECHALL,
                "You can't perform physical attacks on airborne mechs!");
    return;
  }

  if (battle_map_blocks_physical_attacks(mech_map)) {
    mech_notify(mech, MECHALL, "You cannot perform physical attacks here!");
    return;
  }

  if (mech_team(target) == mech_team(mech) &&
      mech_condition_summary(mech).friendly_fire_safety) {
    mech_notify(mech, MECHALL,
                "You can't attack a teammate with FFSafeties on!");
    return;
  }

  if (mech_team(target) == mech_team(mech) &&
      battle_map_blocks_friendly_fire(mech_map)) {
    mech_notify(mech, MECHALL, "Friendly Fire? I don't think so...");
    return;
  }

  if (mech_class(target) == CLASS_MW &&
      !mech_condition_summary(mech).player_killer) {
    mech_notify(
        mech, MECHALL,
        "That's a living, breathing person! Switch off the safety first, "
        "if you really want to assassinate the target.");
    return;
  }

  if (mech_condition_summary(mech).stunned) {
    mech_notify(mech, MECHALL,
                "You are still recovering from your stunning experience!");
    return;
  }
  /*
   * Attack-Specific checks.
   */
  if (AttackType == PA_PUNCH && (mech_class(target) == CLASS_VEH_GROUND) &&
      !mech_condition_summary(mech).fallen) {
    mech_notify(mech, MECHALL,
                "You can't punch vehicles unless you are prone!");
    return;
  }

  // As per BMR, can only trip mechs.
  if (AttackType == PA_TRIP && mech_class(target) != CLASS_MECH) {
    mech_notify(mech, MECHALL, "You can only trip mechs!");
    return;
  }

  // Can't trip mechs that are fallen or in the process of standing.
  if (AttackType == PA_TRIP && (mech_condition_summary(target).fallen ||
                                mech_event_count(target, EVENT_STAND))) {
    mech_notify(mech, MECHALL, "Your target is already down!");
    return;
  }

  // We're attacking a ground/naval unit.
  if (mech_movement_type(target) != MOVE_VTOL &&
      mech_movement_type(target) != MOVE_FLY) {

    if ((AttackType != PA_KICK && AttackType != PA_TRIP) &&
        (mech_position_z(mech) >= mech_position_z(target))) {
      int isTooLow = 0; // Track whether we're too low or not.

      // If it's a fallen mech, too low.
      if (mech_class(target) == CLASS_MECH &&
          mech_condition_summary(target).fallen)
        isTooLow = 1;

      /* Target is to low to punch */
      if ((mech_class(target) == CLASS_MECH) &&
          (mech_position_z(mech) > mech_position_z(target)) &&
          (AttackType == PA_PUNCH)) {
        isTooLow = 1;
      }

      // If it's not a mech, bsuit, or DS, too low.
      if (mech_class(target) != CLASS_MECH &&
          mech_class(target) != CLASS_BSUIT && !mech_is_dropship(target))
        isTooLow = 1;

      // If it's a ground vehicle and we're fallen, then we can
      // punch as per BMR.
      if (AttackType == PA_PUNCH && mech_class(target) == CLASS_VEH_GROUND &&
          mech_condition_summary(mech).fallen)
        isTooLow = 0;

      // If it's a suit that's not on us, we can't physical it.
      if (mech_class(target) == CLASS_BSUIT && mech_swarm_target(target) > 0) {
        mech_printf(mech, MECHALL,
                    "You can't directly physical suits that are swarmed or "
                    "mounted on another mech!");
        return;
      } // end if() - Disallow physicals on swarmed/mounted suits.

      if (isTooLow == 1) {
        mech_printf(mech, MECHALL,
                    "The target is too low in elevation for you to %s.",
                    phys_form(AttackType, 0));
        return;
      } // end if() - Check isTooLow
    } // end if() - Target is too low checks.

    if ((AttackType == PA_KICK || AttackType == PA_TRIP) &&
        mech_position_z(mech) < mech_position_z(target)) {
      mech_notify(mech, MECHALL,
                  "The target is too high in elevation for you to kick at.");
      return;
    }

    if (mech_position_z(mech) - mech_position_z(target) > 1 ||
        mech_position_z(target) - mech_position_z(mech) > 1) {
      mech_notify(mech, MECHALL,
                  "You can't attack, the elevation difference is too large.");
      return;
    }

    if ((AttackType == PA_KICK || AttackType == PA_TRIP) &&
        (mech_position_z(target) < mech_position_z(mech) &&
         (((mech_class(target) == CLASS_MECH) &&
           mech_condition_summary(target).fallen) ||
          (mech_class(target) == CLASS_VEH_GROUND) ||
          (mech_class(target) == CLASS_BSUIT) ||
          (mech_class(target) == CLASS_MW)))) {
      mech_notify(mech, MECHALL,
                  "The target is too low in elevation for you to kick.");
      return;
    }

  } else { // We're attacking a VTOL/Aero.

    if ((AttackType != PA_KICK) &&
        mech_position_z(target) - mech_position_z(mech) > 3) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  phys_form(AttackType, 0));
    }

    if ((AttackType == PA_KICK || AttackType == PA_TRIP) &&
        mech_position_z(mech) != mech_position_z(target)) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  phys_form(AttackType, 0));
      return;
    }

    if (!(mech_position_z(target) - mech_position_z(mech) > -1 &&
          mech_position_z(target) - mech_position_z(mech) < 4)) {
      mech_notify(mech, MECHALL,
                  "You can't attack, the elevation difference is too large.");
      return;
    }
  } // end if/else() - Ground/VTOL + Physical Z comparisons

  /* Check weapon arc! */
  /* Theoretically, physical attacks occur only to 'real' forward
     arc, not rottorsoed one, but we let it pass this time */
  /* This is wrong according to BMR
   *
   * Which states that the Torso twist is taken into account
   * as well as punching/axing/swords can attack in their
   * respective arcs - Dany
   *
   * So I went and changed it according to FASA rules */
  if (AttackType == PA_KICK || AttackType == PA_TRIP) {

    iwa = physical_forward_arc(mech, target);

    if (!(iwa & FORWARDARC)) {
      mech_notify(mech, MECHALL, "Target is not in your 'real' forward arc!");
      return;
    }

  } else { // We're punching, clubbing, or other sharp things.

    iwa = InWeaponArc(mech, mech_position_real_x(target),
                      mech_position_real_y(target));

    if (AttackType == PA_CLUB) {
      // Clubs are a frontal attack. Go off of the forward arc, don't
      // take arms into account.
      if (!(iwa & FORWARDARC) && swarmingUs != 1) {
        mech_notify(mech, MECHALL, "Target is not in your forward arc!");
        return;
      }
    } else {
      // For other attacks, check on a per-arm basis.
      if (sect == RARM) {
        // We're attacking with right arm. Forward or right will do.
        if (!((iwa & FORWARDARC) || (iwa & RSIDEARC) || swarmingUs)) {
          mech_notify(mech, MECHALL,
                      "Target is not in your forward or right side arc!");
          return;
        }
      } else {
        // We're attacking with left arm. Forward or left will do.
        if (!((iwa & FORWARDARC) || (iwa & LSIDEARC)) || swarmingUs) {
          mech_notify(mech, MECHALL,
                      "Target is not in your forward or left side arc!");
          return;
        }

      } // end

    } // end if/else() - club/punch arc check

  } // end if/else() - kick/punch arc check

  /**
   * Add in the movement modifiers
   */

  // If we have melee_specialist advantage, knock -1 off the BTH.
  baseToHit += HasBoolAdvantage(mech_context(mech), mech_pilot_dbref(mech),
                                "melee_specialist")
                   ? MIN(0, mech_attacker_movement_modifier(mech) - 1)
                   : mech_attacker_movement_modifier(mech);

  baseToHit += mech_target_movement_modifier(mech, target, 0.0);

  // BSuits get +1 BTH
  baseToHit += mech_class(target) == CLASS_BSUIT ? 1 : 0;

  // Kicking a BSuit is +3 BTH
  baseToHit +=
      ((mech_class(target) == CLASS_BSUIT) && (AttackType == PA_KICK)) ? 3 : 0;

#ifdef BT_MOVEMENT_MODES
  // A dodging unit is +2, requires maneuvering_ace.
  if (mech_condition_summary(target).dodging)
    baseToHit += 2;
#endif

  // Saws get a +1 BTH.
  if (AttackType == PA_SAW)
    baseToHit += 1;

  // Maces get a +2 BTH.
  if (AttackType == PA_MACE)
    baseToHit += 2;

  // Claws get a +1 BTH.
  if (AttackType == PA_CLAW)
    baseToHit += 1;

  // If we're axing or chopping a bsuit, add +3 to BTH, else (punching) +5.
  if (AttackType != PA_PUNCH && mech_class(target) == CLASS_BSUIT &&
      mech_swarm_target(target) > 0)
    baseToHit += (AttackType != PA_PUNCH) ? 3 : 5;

  // As per BMR, can only physical bsuits with punches, axes, or swords.
  // Added saw since it's the same idea.
  if (AttackType == PA_KICK && mech_class(target) == CLASS_BSUIT &&
      mech_swarm_target(target) > 0) {
    mech_notify(
        mech, MECHALL,
        "You can't hit a swarmed suit with that, try a hand-held weapon!");
    return;
  }

  // Terrain mods - Courtesy of RST
  // Heavy & Light are from Total Warfare and
  // Smoke from MaxTech old BMR
  // Check Smoke first since it can sit on top of other terrain
  // Might want to check for Fire also at some point?
  if (mech_position_terrain(target) == SMOKE) {
    baseToHit += 2;
  } else if (mech_real_terrain_get(target) == HEAVY_FOREST) {
    baseToHit += 2;
  } else if (mech_real_terrain_get(target) == LIGHT_FOREST) {
    baseToHit += 1;
  }

  roll = btech_random_roll(mech_context(mech));

  // Carry out the attack.
  mech_printf(mech, MECHALL, "You try to %s %s.  BTH:  %d,\tRoll:  %d",
              phys_form(AttackType, 0),
              mech_to_mech_display_id(mech, target).text, baseToHit, roll);

  mech_printf(target, MECHSTARTED, "%s tries to %s you!",
              mech_to_mech_display_id(target, mech).text,
              phys_form(AttackType, 0));

  // We send to MechAttacks channel
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
                     tprintf("#%li attacks #%li (%s) (%i/%i)", mech_dbref(mech),
                             mech_dbref(target), phys_form(AttackType, 0),
                             baseToHit, roll));

  // Set the appropriate section(s) to recycle.
  mech_set_recycle_limb(mech, sect, PHYSICAL_RECYCLE_TIME);

  /*
   * Attack-specific recycles and flags.
   */
  if (AttackType == PA_AXE || AttackType == PA_SWORD || AttackType == PA_SAW ||
      AttackType == PA_MACE)
    mech_section_configuration_add(mech, sect, AXED);

  if (AttackType == PA_PUNCH)
    mech_section_configuration_remove(mech, sect, AXED);

  // Clubbing recycles both arms.
  if (AttackType == PA_CLUB)
    mech_set_recycle_limb(mech, LARM, PHYSICAL_RECYCLE_TIME);

  RbaseToHit = baseToHit;
  if (btech_context_glancing_blow_mode(mech_context(mech)) == 2)
    RbaseToHit = baseToHit - 1;
  // We've successfully hit the target.
  if (roll >= RbaseToHit) {
    phys_succeed(mech, target, AttackType);
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (roll == RbaseToHit)) {
      mech_los_broadcast(target, "is nicked by a glancing blow!");
      mech_notify(target, MECHALL, "You are nicked by a glancing blow!");
      glance = 1;
    }
    if (AttackType == PA_CLUB) {
      int clubLoc = -1;

      if (mech_section_carries_club(mech, RARM))
        clubLoc = RARM;
      else if (mech_section_carries_club(mech, LARM))
        clubLoc = LARM;

      if (clubLoc > -1) {
        mech_notify(mech, MECHALL, "Your club shatters on contact.");
        mech_los_broadcast(mech, "'s club shatters with a loud *CRACK*!");

        mech_section_special_remove(mech, clubLoc, CARRYING_CLUB);
      }
    } // End if() - Club shattering

    // Do the deed - Damage the victim. If we're tripping, we don't do
    // damage but try to make a skill roll.
    if (AttackType != PA_TRIP)
      PhysicalDamage(mech, target, damageweight, AttackType, sect, glance);
    else
      PhysicalTrip(mech, target);

  } else { // We have failed!
    phys_fail(mech, target, AttackType);

    if (mech_class(target) == CLASS_BSUIT &&
        mech_swarm_target(target) == mech_dbref(mech)) {

      if (!MadePilotSkillRoll(mech, 4)) {
        mech_notify(mech, MECHALL,
                    "Uh oh. You miss the little buggers, but hit yourself!");
        mech_los_broadcast(mech, "misses, and hits itself!");

        PhysicalDamage(mech, mech, damageweight, AttackType, sect, glance);
      } // If we really screw up against suits swarmed on ourselves,
      // nail us for damage.
    } // end if() - Suit + Swarmed + Physical + Self Damage checks

    /* Removed fall check for clubs -- Power_Shaper 09/25/06 */
    if (AttackType == PA_KICK || AttackType == PA_MACE) {
      int failRoll = (AttackType == PA_KICK ? 0 : 2);

      mech_notify(mech, MECHALL, "You miss and try to remain standing!");

      // We fail the piloting skill roll and flop on our face.
      if (!MadePilotSkillRoll(mech, failRoll)) {
        mech_notify(mech, MECHALL, "You lose your balance and fall down!");
        mech_fall(mech, 1, 1);
      } // end if() - Miss/fall.
    } // end if() - Miss kick/club and risk falling.
  } // end if() - Physical failure handling.
} // end PhysicalAttack()
