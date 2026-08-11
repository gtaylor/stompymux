#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical.h"
#include "mech_physical_api.h"
#include "mech_physical_internal.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
#include <stddef.h>

static int physical_forward_arc(Mech *mech, const Mech *target) {
  MechConditionSummary condition = mech_condition_summary(mech);
  mech_torso_twist_set(mech, MECH_TORSO_CENTER);
  int arc = in_weapon_arc(mech, mech_position_real_x(target),
                          mech_position_real_y(target));
  if (condition.torso_left)
    mech_torso_twist_set(mech, MECH_TORSO_LEFT);
  else if (condition.torso_right)
    mech_torso_twist_set(mech, MECH_TORSO_RIGHT);
  return arc;
}

void physical_attack_resolve(const PhysicalAttackRequest *request) {
  Mech *mech = request->mech;
  const int DAMAGEWEIGHT = request->damage_weight;
  int base_to_hit = request->base_to_hit;
  const PhysicalAttackType ATTACK_TYPE = request->attack_type;
  const int ARGC = request->argument_count;
  char **args = request->arguments;
  BattleMap *mech_map = request->map;
  const int SECT = request->section;
  Mech *target;
  float range;
  float max_range = 1;
  char target_id[2];
  DbRef targetnum;
  int roll, swarming_us;
  char location[20];
  int iwa;
  int rbase_to_hit, glance = 0;

  /*
   * Common Checks
   */

  // Since we can punch with two arms, often back to back, we want to run
  // these generic checks in mech_punch() -BEFORE- PhysicalAttack() is called
  // twice (if we have two working arms).
  if (ATTACK_TYPE == PA_PUNCH || ATTACK_TYPE == PA_CLAW) {
    // Do Nothing
  } else {
    if (!phys_common_checks(mech))
      return;
  }

  /* BTH Adjustments for crits to limbs - seperate one for
   * club because it checks for both limbs */
  if ((ATTACK_TYPE == PA_PUNCH) || (ATTACK_TYPE == PA_KICK) ||
      (ATTACK_TYPE == PA_AXE) || (ATTACK_TYPE == PA_SWORD) ||
      (ATTACK_TYPE == PA_CLAW) || (ATTACK_TYPE == PA_MACE) ||
      (ATTACK_TYPE == PA_SAW)) {

    if (mech_critical_is_nonfunctional(mech, SECT, 1) ||
        mech_critical_part_type(mech, SECT, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      base_to_hit += 2;
    }

    if (mech_critical_is_nonfunctional(mech, SECT, 2) ||
        mech_critical_part_type(mech, SECT, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      base_to_hit += 2;
    }

    /* Hand/Foot crits only affect punch/kick since with the other attacks
     * are not allowed if they're broken */
    if ((ATTACK_TYPE == PA_PUNCH) || (ATTACK_TYPE == PA_KICK)) {
      if (mech_critical_is_nonfunctional(mech, SECT, 3) ||
          mech_critical_part_type(mech, SECT, 3) !=
              special_equipment_index(HAND_OR_FOOT_ACTUATOR)) {
        base_to_hit += 1;
      }
    }

  } else if (ATTACK_TYPE == PA_CLUB) {

    /* Only check lower/upper acts since without shoulder or hand you can't
     * club */
    if (mech_critical_is_nonfunctional(mech, RARM, 1) ||
        mech_critical_part_type(mech, SECT, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      base_to_hit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, RARM, 2) ||
        mech_critical_part_type(mech, SECT, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      base_to_hit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, LARM, 1) ||
        mech_critical_part_type(mech, SECT, 1) !=
            special_equipment_index(UPPER_ACTUATOR)) {
      base_to_hit += 2;
    }
    if (mech_critical_is_nonfunctional(mech, LARM, 2) ||
        mech_critical_part_type(mech, SECT, 2) !=
            special_equipment_index(LOWER_ACTUATOR)) {
      base_to_hit += 2;
    }
  }

  // All weapons must be cycled in the target limb.
  if (mech_section_has_recycling_weapon(mech, SECT)) {
    armor_string_from_index(SECT, location, mech_class(mech),
                            mech_movement_type(mech));
    mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                location);
    return;
  }
  // Figure out what to do with the arguments provided with the physical
  // command.
  switch (ARGC) {
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
    char **first_slot = (char **)checked_storage_at((void *)args, (size_t)ARGC,
                                                    sizeof(*args), 0);
    target_id[0] = *checked_string_suffix(*first_slot, 0);
    target_id[1] = *checked_string_suffix(*first_slot, 1);
    targetnum = find_target_dbref_from_map_number(mech, target_id);
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
  swarming_us = (mech_swarm_target(target) == mech_dbref(mech) ? 1 : 0);

  /*
   * Common checks.
   */

  // If we're attacking something while fallen that isn't swarming us,
  // no-go it. Kicks/trips are automatically stopped.
  if (mech_condition_summary(mech).fallen &&
      (ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_TRIP)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                physical_attack_verb(
                    &(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));

    return;
    // If we are fallen AND
    //   The target is not a BSuit AND We're not punching
  } else if (mech_condition_summary(mech).fallen &&
             (mech_class(target) != CLASS_VEH_GROUND &&
              mech_class(target) != CLASS_BSUIT)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                physical_attack_verb(
                    &(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));

    return;
  } else if (mech_condition_summary(mech).fallen &&
             mech_class(target) == CLASS_BSUIT && !swarming_us) {
    mech_notify(
        mech, MECHALL,
        "You may only physical suits that are swarming you while prone.");
    return;
  } else if (mech_condition_summary(mech).fallen &&
             mech_class(target) == CLASS_VEH_GROUND &&
             ATTACK_TYPE != PA_PUNCH) {
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
    max_range = 0.5;

  if (range >= max_range) {
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
  if (ATTACK_TYPE == PA_PUNCH && (mech_class(target) == CLASS_VEH_GROUND) &&
      !mech_condition_summary(mech).fallen) {
    mech_notify(mech, MECHALL,
                "You can't punch vehicles unless you are prone!");
    return;
  }

  // As per BMR, can only trip mechs.
  if (ATTACK_TYPE == PA_TRIP && mech_class(target) != CLASS_MECH) {
    mech_notify(mech, MECHALL, "You can only trip mechs!");
    return;
  }

  // Can't trip mechs that are fallen or in the process of standing.
  if (ATTACK_TYPE == PA_TRIP && (mech_condition_summary(target).fallen ||
                                 mech_event_count(target, EVENT_STAND))) {
    mech_notify(mech, MECHALL, "Your target is already down!");
    return;
  }

  // We're attacking a ground/naval unit.
  if (mech_movement_type(target) != MOVE_VTOL &&
      mech_movement_type(target) != MOVE_FLY) {

    if ((ATTACK_TYPE != PA_KICK && ATTACK_TYPE != PA_TRIP) &&
        (mech_position_z(mech) >= mech_position_z(target))) {
      int is_too_low = 0; // Track whether we're too low or not.

      // If it's a fallen mech, too low.
      if (mech_class(target) == CLASS_MECH &&
          mech_condition_summary(target).fallen)
        is_too_low = 1;

      /* Target is to low to punch */
      if ((mech_class(target) == CLASS_MECH) &&
          (mech_position_z(mech) > mech_position_z(target)) &&
          (ATTACK_TYPE == PA_PUNCH)) {
        is_too_low = 1;
      }

      // If it's not a mech, bsuit, or DS, too low.
      if (mech_class(target) != CLASS_MECH &&
          mech_class(target) != CLASS_BSUIT && !mech_is_dropship(target))
        is_too_low = 1;

      // If it's a ground vehicle and we're fallen, then we can
      // punch as per BMR.
      if (ATTACK_TYPE == PA_PUNCH && mech_class(target) == CLASS_VEH_GROUND &&
          mech_condition_summary(mech).fallen)
        is_too_low = 0;

      // If it's a suit that's not on us, we can't physical it.
      if (mech_class(target) == CLASS_BSUIT && mech_swarm_target(target) > 0) {
        mech_printf(mech, MECHALL,
                    "You can't directly physical suits that are swarmed or "
                    "mounted on another mech!");
        return;
      } // end if() - Disallow physicals on swarmed/mounted suits.

      if (is_too_low == 1) {
        mech_printf(mech, MECHALL,
                    "The target is too low in elevation for you to %s.",
                    physical_attack_verb(
                        &(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));
        return;
      } // end if() - Check isTooLow
    } // end if() - Target is too low checks.

    if ((ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_TRIP) &&
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

    if ((ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_TRIP) &&
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

    if ((ATTACK_TYPE != PA_KICK) &&
        mech_position_z(target) - mech_position_z(mech) > 3) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  physical_attack_verb(
                      &(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));
    }

    if ((ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_TRIP) &&
        mech_position_z(mech) != mech_position_z(target)) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  physical_attack_verb(
                      &(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));
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
  if (ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_TRIP) {

    iwa = physical_forward_arc(mech, target);

    if (!(iwa & FORWARDARC)) {
      mech_notify(mech, MECHALL, "Target is not in your 'real' forward arc!");
      return;
    }

  } else { // We're punching, clubbing, or other sharp things.

    iwa = in_weapon_arc(mech, mech_position_real_x(target),
                        mech_position_real_y(target));

    if (ATTACK_TYPE == PA_CLUB) {
      // Clubs are a frontal attack. Go off of the forward arc, don't
      // take arms into account.
      if (!(iwa & FORWARDARC) && swarming_us != 1) {
        mech_notify(mech, MECHALL, "Target is not in your forward arc!");
        return;
      }
    } else {
      // For other attacks, check on a per-arm basis.
      if (SECT == RARM) {
        // We're attacking with right arm. Forward or right will do.
        if (!((iwa & FORWARDARC) || (iwa & RSIDEARC) || swarming_us)) {
          mech_notify(mech, MECHALL,
                      "Target is not in your forward or right side arc!");
          return;
        }
      } else {
        // We're attacking with left arm. Forward or left will do.
        if (!((iwa & FORWARDARC) || (iwa & LSIDEARC)) || swarming_us) {
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
  base_to_hit += has_bool_advantage(mech_context(mech), mech_pilot_dbref(mech),
                                    "melee_specialist")
                     ? min(0, mech_attacker_movement_modifier(mech) - 1)
                     : mech_attacker_movement_modifier(mech);

  base_to_hit += mech_target_movement_modifier(mech, target, 0.0);

  // BSuits get +1 BTH
  base_to_hit += mech_class(target) == CLASS_BSUIT ? 1 : 0;

  // Kicking a BSuit is +3 BTH
  base_to_hit +=
      ((mech_class(target) == CLASS_BSUIT) && (ATTACK_TYPE == PA_KICK)) ? 3 : 0;

#ifdef BT_MOVEMENT_MODES
  // A dodging unit is +2, requires maneuvering_ace.
  if (mech_condition_summary(target).dodging)
    base_to_hit += 2;
#endif

  // Saws get a +1 BTH.
  if (ATTACK_TYPE == PA_SAW)
    base_to_hit += 1;

  // Maces get a +2 BTH.
  if (ATTACK_TYPE == PA_MACE)
    base_to_hit += 2;

  // Claws get a +1 BTH.
  if (ATTACK_TYPE == PA_CLAW)
    base_to_hit += 1;

  // If we're axing or chopping a bsuit, add +3 to BTH, else (punching) +5.
  if (ATTACK_TYPE != PA_PUNCH && mech_class(target) == CLASS_BSUIT &&
      mech_swarm_target(target) > 0)
    base_to_hit += (ATTACK_TYPE != PA_PUNCH) ? 3 : 5;

  // As per BMR, can only physical bsuits with punches, axes, or swords.
  // Added saw since it's the same idea.
  if (ATTACK_TYPE == PA_KICK && mech_class(target) == CLASS_BSUIT &&
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
    base_to_hit += 2;
  } else if (mech_real_terrain_get(target) == HEAVY_FOREST) {
    base_to_hit += 2;
  } else if (mech_real_terrain_get(target) == LIGHT_FOREST) {
    base_to_hit += 1;
  }

  roll = btech_random_roll(mech_context(mech));

  // Carry out the attack.
  mech_printf(
      mech, MECHALL, "You try to %s %s.  BTH:  %d,\tRoll:  %d",
      physical_attack_verb(&(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}),
      mech_to_mech_display_id(mech, target).text, base_to_hit, roll);

  mech_printf(
      target, MECHSTARTED, "%s tries to %s you!",
      mech_to_mech_display_id(target, mech).text,
      physical_attack_verb(&(PhysicalVerbRequest){.attack_type = ATTACK_TYPE}));

  // We send to MechAttacks channel
  btech_channel_send(mech_context(mech), BTECH_CHANNEL_MECH_ATTACKS, "%s",
                     tprintf("#%li attacks #%li (%s) (%i/%i)", mech_dbref(mech),
                             mech_dbref(target),
                             physical_attack_verb(&(PhysicalVerbRequest){
                                 .attack_type = ATTACK_TYPE}),
                             base_to_hit, roll));

  // Set the appropriate section(s) to recycle.
  mech_set_recycle_limb(mech, SECT, PHYSICAL_RECYCLE_TIME);

  /*
   * Attack-specific recycles and flags.
   */
  if (ATTACK_TYPE == PA_AXE || ATTACK_TYPE == PA_SWORD ||
      ATTACK_TYPE == PA_SAW || ATTACK_TYPE == PA_MACE)
    mech_section_configuration_add(mech, SECT, AXED);

  if (ATTACK_TYPE == PA_PUNCH)
    mech_section_configuration_remove(mech, SECT, AXED);

  // Clubbing recycles both arms.
  if (ATTACK_TYPE == PA_CLUB)
    mech_set_recycle_limb(mech, LARM, PHYSICAL_RECYCLE_TIME);

  rbase_to_hit = base_to_hit;
  if (btech_context_glancing_blow_mode(mech_context(mech)) == 2)
    rbase_to_hit = base_to_hit - 1;
  // We've successfully hit the target.
  if (roll >= rbase_to_hit) {
    phys_succeed(mech, target, ATTACK_TYPE);
    if (btech_context_glancing_blows_enabled(mech_context(mech)) &&
        (roll == rbase_to_hit)) {
      mech_los_broadcast(target, "is nicked by a glancing blow!");
      mech_notify(target, MECHALL, "You are nicked by a glancing blow!");
      glance = 1;
    }
    if (ATTACK_TYPE == PA_CLUB) {
      int club_loc = -1;

      if (mech_section_carries_club(mech, RARM))
        club_loc = RARM;
      else if (mech_section_carries_club(mech, LARM))
        club_loc = LARM;

      if (club_loc > -1) {
        mech_notify(mech, MECHALL, "Your club shatters on contact.");
        mech_los_broadcast(mech, "'s club shatters with a loud *CRACK*!");

        mech_section_special_remove(mech, club_loc, CARRYING_CLUB);
      }
    } // End if() - Club shattering

    // Do the deed - Damage the victim. If we're tripping, we don't do
    // damage but try to make a skill roll.
    if (ATTACK_TYPE != PA_TRIP)
      physical_damage_resolve(
          &(PhysicalDamageRequest){.attacker = mech,
                                   .target = target,
                                   .weight_divisor = DAMAGEWEIGHT,
                                   .attack_type = ATTACK_TYPE,
                                   .section = SECT,
                                   .glancing_damage = glance});
    else
      physical_trip(mech, target);

  } else { // We have failed!
    phys_fail(mech, target, ATTACK_TYPE);

    if (mech_class(target) == CLASS_BSUIT &&
        mech_swarm_target(target) == mech_dbref(mech)) {

      if (!made_pilot_skill_roll(mech, 4)) {
        mech_notify(mech, MECHALL,
                    "Uh oh. You miss the little buggers, but hit yourself!");
        mech_los_broadcast(mech, "misses, and hits itself!");

        physical_damage_resolve(
            &(PhysicalDamageRequest){.attacker = mech,
                                     .target = mech,
                                     .weight_divisor = DAMAGEWEIGHT,
                                     .attack_type = ATTACK_TYPE,
                                     .section = SECT,
                                     .glancing_damage = glance});
      } // If we really screw up against suits swarmed on ourselves,
      // nail us for damage.
    } // end if() - Suit + Swarmed + Physical + Self Damage checks

    /* Removed fall check for clubs -- Power_Shaper 09/25/06 */
    if (ATTACK_TYPE == PA_KICK || ATTACK_TYPE == PA_MACE) {
      int fail_roll = (ATTACK_TYPE == PA_KICK ? 0 : 2);

      mech_notify(mech, MECHALL, "You miss and try to remain standing!");

      // We fail the piloting skill roll and flop on our face.
      if (!made_pilot_skill_roll(mech, fail_roll)) {
        mech_notify(mech, MECHALL, "You lose your balance and fall down!");
        mech_fall(mech, 1, 1);
      } // end if() - Miss/fall.
    } // end if() - Miss kick/club and risk falling.
  } // end if() - Physical failure handling.
} // end PhysicalAttack()
