#include "mech_physical_internal.h"
void PhysicalAttack(Mech *mech, int damageweight, int baseToHit, int AttackType,
                    int argc, char **args, BattleMap *mech_map, int sect) {
  Mech *target;
  float range;
  float maxRange = 1;
  char targetID[2];
  int targetnum, roll, swarmingUs;
  char location[20];
  int ts = 0, iwa;
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

    if (PartIsNonfunctional(mech, sect, 1) ||
        GetPartType(mech, sect, 1) != I2Special(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }

    if (PartIsNonfunctional(mech, sect, 2) ||
        GetPartType(mech, sect, 2) != I2Special(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }

    /* Hand/Foot crits only affect punch/kick since with the other attacks
     * are not allowed if they're broken */
    if ((AttackType == PA_PUNCH) || (AttackType == PA_KICK)) {
      if (PartIsNonfunctional(mech, sect, 3) ||
          GetPartType(mech, sect, 3) != I2Special(HAND_OR_FOOT_ACTUATOR)) {
        baseToHit += 1;
      }
    }

  } else if (AttackType == PA_CLUB) {

    /* Only check lower/upper acts since without shoulder or hand you can't
     * club */
    if (PartIsNonfunctional(mech, RARM, 1) ||
        GetPartType(mech, sect, 1) != I2Special(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (PartIsNonfunctional(mech, RARM, 2) ||
        GetPartType(mech, sect, 2) != I2Special(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (PartIsNonfunctional(mech, LARM, 1) ||
        GetPartType(mech, sect, 1) != I2Special(UPPER_ACTUATOR)) {
      baseToHit += 2;
    }
    if (PartIsNonfunctional(mech, LARM, 2) ||
        GetPartType(mech, sect, 2) != I2Special(LOWER_ACTUATOR)) {
      baseToHit += 2;
    }
  }

  // All weapons must be cycled in the target limb.
  if (SectHasBusyWeap(mech, sect)) {
    ArmorStringFromIndex(sect, location, MechType(mech), MechMove(mech));
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
    DOCHECKMA(MechTarget(mech) == -1, "You do not have a target set!");

    // Populate target variable with current lock.
    target = btech_context_get_mech(mech->xcode.context, MechTarget(mech));
    DOCHECKMA(!target, "Invalid default target!");

    break;
  default:
    // In this case, default means user has specified an argument
    // with the physical attack.

    // Populate target variable from user input.
    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    targetnum = FindTargetDBREFFromMapNumber(mech, targetID);
    target = btech_context_get_mech(mech->xcode.context, targetnum);

    DOCHECKMA(targetnum == -1, "Target is not in line of sight!");
    DOCHECKMA(!target, "Invalid default target!");
  } // end switch() - argc checking

  // Is the target swarming us?
  swarmingUs = (MechSwarmTarget(target) == mech->mynum ? 1 : 0);

  /*
   * Common checks.
   */

  // If we're attacking something while fallen that isn't swarming us,
  // no-go it. Kicks/trips are automatically stopped.
  if (Fallen(mech) && (AttackType == PA_KICK || AttackType == PA_TRIP)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                phys_form(AttackType, 0));

    return;
    // If we are fallen AND
    //   The target is not a BSuit AND We're not punching
  } else if (Fallen(mech) && (MechType(target) != CLASS_VEH_GROUND &&
                              MechType(target) != CLASS_BSUIT)) {
    mech_printf(mech, MECHALL, "You can't %s from a prone position.",
                phys_form(AttackType, 0));

    return;
  } else if (Fallen(mech) && MechType(target) == CLASS_BSUIT && !swarmingUs) {
    mech_notify(
        mech, MECHALL,
        "You may only physical suits that are swarming you while prone.");
    return;
  } else if (Fallen(mech) && MechType(target) == CLASS_VEH_GROUND &&
             AttackType != PA_PUNCH) {
    mech_notify(mech, MECHALL, "You may only punch vehicles while prone.");
    return;
  } // end if() - Physical while fallen.

  range = FaMechRange(mech, target);

  DOCHECKMA(!mech_los_check_unblocked(mech, target, MechX(target),
                                      MechY(target), range),
            "Target is not in line of sight!");

  // BSuits have to be <= 0.5 hexes to attack units.
  if ((MechType(target) == CLASS_BSUIT) || (MechType(target) == CLASS_MW))
    maxRange = 0.5;

  DOCHECKMA(range >= maxRange, "Target out of range!");

  DOCHECKMA(Jumping(target),
            "You can't perform physical attacks on airborne mechs!");

  DOCHECKMA(MapNoPhysicals(mech_map),
            "You cannot perform physical attacks here!");

  DOCHECKMA(MechTeam(target) == MechTeam(mech) && MechNoFriendlyFire(mech),
            "You can't attack a teammate with FFSafeties on!");

  DOCHECKMA(MechTeam(target) == MechTeam(mech) && MapNoFriendlyFire(mech_map),
            "Friendly Fire? I don't think so...");

  DOCHECKMA(MechType(target) == CLASS_MW && !MechPKiller(mech),
            "That's a living, breathing person! Switch off the safety first, "
            "if you really want to assassinate the target.");

  DOCHECKMA(MechCritStatus(mech) & MECH_STUNNED,
            "You are still recovering from your stunning experience!");
  /*
   * Attack-Specific checks.
   */
  DOCHECKMA(AttackType == PA_PUNCH && (MechType(target) == CLASS_VEH_GROUND) &&
                !Fallen(mech),
            "You can't punch vehicles unless you are prone!");

  // As per BMR, can only trip mechs.
  DOCHECKMA(AttackType == PA_TRIP && MechType(target) != CLASS_MECH,
            "You can only trip mechs!");

  // Can't trip mechs that are fallen or in the process of standing.
  DOCHECKMA(AttackType == PA_TRIP &&
                (Fallen(target) || mech_event_count(target, EVENT_STAND)),
            "Your target is already down!");

  // We're attacking a ground/naval unit.
  if (MechMove(target) != MOVE_VTOL && MechMove(target) != MOVE_FLY) {

    if ((AttackType != PA_KICK && AttackType != PA_TRIP) &&
        (MechZ(mech) >= MechZ(target))) {
      int isTooLow = 0; // Track whether we're too low or not.

      // If it's a fallen mech, too low.
      if (MechType(target) == CLASS_MECH && Fallen(target))
        isTooLow = 1;

      /* Target is to low to punch */
      if ((MechType(target) == CLASS_MECH) && (MechZ(mech) > MechZ(target)) &&
          (AttackType == PA_PUNCH)) {
        isTooLow = 1;
      }

      // If it's not a mech, bsuit, or DS, too low.
      if (MechType(target) != CLASS_MECH && MechType(target) != CLASS_BSUIT &&
          !IsDS(target))
        isTooLow = 1;

      // If it's a ground vehicle and we're fallen, then we can
      // punch as per BMR.
      if (AttackType == PA_PUNCH && MechType(target) == CLASS_VEH_GROUND &&
          Fallen(mech))
        isTooLow = 0;

      // If it's a suit that's not on us, we can't physical it.
      if (MechType(target) == CLASS_BSUIT && MechSwarmTarget(target) > 0) {
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

    DOCHECKMA((AttackType == PA_KICK || AttackType == PA_TRIP) &&
                  MechZ(mech) < MechZ(target),
              "The target is too high in elevation for you to kick at.");

    DOCHECKMA(MechZ(mech) - MechZ(target) > 1 ||
                  MechZ(target) - MechZ(mech) > 1,
              "You can't attack, the elevation difference is too large.");

    DOCHECKMA((AttackType == PA_KICK || AttackType == PA_TRIP) &&
                  (MechZ(target) < MechZ(mech) &&
                   (((MechType(target) == CLASS_MECH) && Fallen(target)) ||
                    (MechType(target) == CLASS_VEH_GROUND) ||
                    (MechType(target) == CLASS_BSUIT) ||
                    (MechType(target) == CLASS_MW))),
              "The target is too low in elevation for you to kick.")

  } else { // We're attacking a VTOL/Aero.

    if ((AttackType != PA_KICK) && MechZ(target) - MechZ(mech) > 3) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  phys_form(AttackType, 0));
    }

    if ((AttackType == PA_KICK || AttackType == PA_TRIP) &&
        MechZ(mech) != MechZ(target)) {
      mech_printf(mech, MECHALL, "The target is too far away for you to %s.",
                  phys_form(AttackType, 0));
      return;
    }

    DOCHECKMA(
        !(MechZ(target) - MechZ(mech) > -1 && MechZ(target) - MechZ(mech) < 4),
        "You can't attack, the elevation difference is too large.");
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

    ts = MechStatus(mech) & (TORSO_LEFT | TORSO_RIGHT);
    MechStatus(mech) &= ~ts;
    iwa = InWeaponArc(mech, MechFX(target), MechFY(target));
    MechStatus(mech) |= ts;

    DOCHECKMA(!(iwa & FORWARDARC), "Target is not in your 'real' forward arc!");

  } else { // We're punching, clubbing, or other sharp things.

    iwa = InWeaponArc(mech, MechFX(target), MechFY(target));

    if (AttackType == PA_CLUB) {
      // Clubs are a frontal attack. Go off of the forward arc, don't
      // take arms into account.
      DOCHECKMA(!(iwa & FORWARDARC) && swarmingUs != 1,
                "Target is not in your forward arc!");
    } else {
      // For other attacks, check on a per-arm basis.
      if (sect == RARM) {
        // We're attacking with right arm. Forward or right will do.
        DOCHECKMA(!((iwa & FORWARDARC) || (iwa & RSIDEARC) || swarmingUs),
                  "Target is not in your forward or right side arc!");
      } else {
        // We're attacking with left arm. Forward or left will do.
        DOCHECKMA(!((iwa & FORWARDARC) || (iwa & LSIDEARC)) || swarmingUs,
                  "Target is not in your forward or left side arc!");

      } // end

    } // end if/else() - club/punch arc check

  } // end if/else() - kick/punch arc check

  /**
   * Add in the movement modifiers
   */

  // If we have melee_specialist advantage, knock -1 off the BTH.
  baseToHit +=
      HasBoolAdvantage(mech->xcode.context, MechPilot(mech), "melee_specialist")
          ? MIN(0, AttackMovementMods(mech) - 1)
          : AttackMovementMods(mech);

  baseToHit += TargetMovementMods(mech, target, 0.0);

  // BSuits get +1 BTH
  baseToHit += MechType(target) == CLASS_BSUIT ? 1 : 0;

  // Kicking a BSuit is +3 BTH
  baseToHit +=
      ((MechType(target) == CLASS_BSUIT) && (AttackType == PA_KICK)) ? 3 : 0;

#ifdef BT_MOVEMENT_MODES
  // A dodging unit is +2, requires maneuvering_ace.
  if (Dodging(target))
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
  if (AttackType != PA_PUNCH && MechType(target) == CLASS_BSUIT &&
      MechSwarmTarget(target) > 0)
    baseToHit += (AttackType != PA_PUNCH) ? 3 : 5;

  // As per BMR, can only physical bsuits with punches, axes, or swords.
  // Added saw since it's the same idea.
  DOCHECKMA(AttackType == PA_KICK && MechType(target) == CLASS_BSUIT &&
                MechSwarmTarget(target) > 0,
            "You can't hit a swarmed suit with that, try a hand-held weapon!");

  // Terrain mods - Courtesy of RST
  // Heavy & Light are from Total Warfare and
  // Smoke from MaxTech old BMR
  // Check Smoke first since it can sit on top of other terrain
  // Might want to check for Fire also at some point?
  if (MechTerrain(target) == SMOKE) {
    baseToHit += 2;
  } else if (mech_real_terrain_get(target) == HEAVY_FOREST) {
    baseToHit += 2;
  } else if (mech_real_terrain_get(target) == LIGHT_FOREST) {
    baseToHit += 1;
  }

  roll = btech_random_roll(mech->xcode.context);

  // Carry out the attack.
  mech_printf(mech, MECHALL, "You try to %s %s.  BTH:  %d,\tRoll:  %d",
              phys_form(AttackType, 0),
              mech_to_mech_display_id(mech, target).text, baseToHit, roll);

  mech_printf(target, MECHSTARTED, "%s tries to %s you!",
              mech_to_mech_display_id(target, mech).text,
              phys_form(AttackType, 0));

  // We send to MechAttacks channel
  btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_ATTACKS, "%s",
                     tprintf("#%li attacks #%li (%s) (%i/%i)", mech->mynum,
                             target->mynum, phys_form(AttackType, 0), baseToHit,
                             roll));

  // Set the appropriate section(s) to recycle.
  mech_set_recycle_limb(mech, sect, PHYSICAL_RECYCLE_TIME);

  /*
   * Attack-specific recycles and flags.
   */
  if (AttackType == PA_AXE || AttackType == PA_SWORD || AttackType == PA_SAW ||
      AttackType == PA_MACE)
    MechSections(mech)[sect].config |= AXED;

  if (AttackType == PA_PUNCH)
    MechSections(mech)[sect].config &= ~AXED;

  // Clubbing recycles both arms.
  if (AttackType == PA_CLUB)
    mech_set_recycle_limb(mech, LARM, PHYSICAL_RECYCLE_TIME);

  RbaseToHit = baseToHit;
  if (mech->xcode.context->configuration->btech_glancing_blows == 2)
    RbaseToHit = baseToHit - 1;
  // We've successfully hit the target.
  if (roll >= RbaseToHit) {
    phys_succeed(mech, target, AttackType);
    if (mech->xcode.context->configuration->btech_glancing_blows &&
        (roll == RbaseToHit)) {
      mech_los_broadcast(target, "is nicked by a glancing blow!");
      mech_notify(target, MECHALL, "You are nicked by a glancing blow!");
      glance = 1;
    }
    if (AttackType == PA_CLUB) {
      int clubLoc = -1;

      if (MechSections(mech)[RARM].specials & CARRYING_CLUB)
        clubLoc = RARM;
      else if (MechSections(mech)[LARM].specials & CARRYING_CLUB)
        clubLoc = LARM;

      if (clubLoc > -1) {
        mech_notify(mech, MECHALL, "Your club shatters on contact.");
        mech_los_broadcast(mech, "'s club shatters with a loud *CRACK*!");

        MechSections(mech)[clubLoc].specials &= ~CARRYING_CLUB;
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

    if (MechType(target) == CLASS_BSUIT &&
        MechSwarmTarget(target) == mech->mynum) {

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
        MechFalls(mech, 1, 1);
      } // end if() - Miss/fall.
    } // end if() - Miss kick/club and risk falling.
  } // end if() - Physical failure handling.
} // end PhysicalAttack()

#define MyDamageMech(a, b, c, d, e, f, g, h, i)                                \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_PILOTING;                                                \
  DamageMech(a, b, c, d, e, f, g, h, i, -1, 0, -1, 0, 0);                      \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_GUNNERY
#define MyDamageMech2(a, b, c, d, e, f, g, h, i)                               \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_NONE;                                                    \
  DamageMech(a, b, c, d, e, f, g, h, i, -1, 0, -1, 0, 0);                      \
  (a)->xcode.context->combat_overrides.damage_experience =                     \
      BTECH_DAMAGE_XP_GUNNERY

/*
 * Try to trip the victim.
 */
