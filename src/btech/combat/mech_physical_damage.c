#include "mech_physical_internal.h"
void PhysicalTrip(Mech *mech, Mech *target) {
  // If we trip our target (who is a mech), make a roll to see if he falls.
  if (!MadePilotSkillRoll(target, 0) && !Fallen(target)) {

    // Emit to Attacker
    mech_printf(mech, MECHALL, "You trip %s!",
                mech_to_mech_display_id(mech, target).text);

    // Emit to victim and LOS.
    mech_notify(target, MECHSTARTED, "You are tripped and fall to the ground!");
    mech_los_broadcast(target, "trips up and falls down!");

    MechFalls(target, 1, 0);
  } else {
    mech_los_broadcast(target, "manages to stay upright!");
  }
} // end PhysicalTrip()

/*
 * Damage the victim.
 */
void PhysicalDamage(Mech *mech, Mech *target, int weightdmg, int AttackType,
                    int sect, int glance) {

  int hitloc = 0, damage, hitgroup = 0, isrear, iscritical;

  isrear = 0;
  iscritical = 0;

  /* Two types of physical attack weapons - Those affected by TSM
   * and those not - Right now just saw but can add more to the list via
   * || (AttackType == PA_BLAH) */
  if (AttackType == PA_SAW) {

    /* Saws do a constant 7 damage due to their mechanical nature. */
    damage = 7;

  } else {

    /* Sword attack uses an odd weapon damage amount */
    if (AttackType == PA_SWORD) {
      damage = (MechTons(mech) + 5) / weightdmg + 1;
    } else {
      /* Round Down to nearest ton -- TW Page 145 */
      damage = (int)floor((((float)MechTons(mech)) / weightdmg));
    }

    /* Calc in affect by TSM */
    if ((MechHeat(mech) >= 9.) && (MechSpecials(mech) & TRIPLE_MYOMER_TECH)) {
      damage = damage * 2;
    }
  }

  /* If we have melee_specialist, add a point of damage. */
  if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                       "melee_specialist")) {
    damage++;
  }

  switch (AttackType) {
  case PA_PUNCH:

    if (!OkayCritSectS(sect, 2, LOWER_ACTUATOR)) {
      damage = damage / 2;
    }

    if (!OkayCritSectS(sect, 1, UPPER_ACTUATOR)) {
      damage = damage / 2;
    }

    hitgroup = FindAreaHitGroup(mech, target);
    if (hitgroup == BACK) {
      isrear = 1;
    }

    if (MechType(mech) == CLASS_MECH) {

      if (Fallen(mech)) {

        /* Total Warfare page 151 - Prone mechs can only make
         * two types of physical attacks - Punching (with one arm)
         * vehicles in same hex and thrashing - But for now including
         * this. - Dany 01/2007 */
        if ((MechType(target) != CLASS_MECH) ||
            (Fallen(target) &&
             (MechElevation(mech) == MechElevation(target)))) {
          hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
        } else if (!Fallen(target) &&
                   (MechElevation(mech) > MechElevation(target))) {
          hitloc = FindPunchLocation(target, hitgroup);
        } else if (MechElevation(mech) == MechElevation(target)) {
          hitloc = FindKickLocation(target, hitgroup);
        }

      } else if (MechElevation(mech) < MechElevation(target)) {

        if (Fallen(target) || MechType(target) != CLASS_MECH) {
          hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
        } else {
          hitloc = FindKickLocation(target, hitgroup);
        }

      } else {
        hitloc = FindPunchLocation(target, hitgroup);
      }

    } else {
      hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
    }

    break;

  case PA_SWORD:
  case PA_AXE:
  case PA_MACE:
  case PA_CLUB:

    hitgroup = FindAreaHitGroup(mech, target);
    if (hitgroup == BACK) {
      isrear = 1;
    }

    if (MechType(mech) == CLASS_MECH) {

      if (MechElevation(mech) < MechElevation(target)) {

        if (Fallen(target) || MechType(target) != CLASS_MECH) {
          hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
        } else {
          hitloc = FindKickLocation(target, hitgroup);
        }

      } else if (MechElevation(mech) > MechElevation(target)) {
        hitloc = FindPunchLocation(target, hitgroup);
      } else {
        hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
      }

    } else {
      hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
    }
    break;

  case PA_KICK:

    if (!OkayCritSectS(sect, 2, LOWER_ACTUATOR))
      damage = damage / 2;

    if (!OkayCritSectS(sect, 1, UPPER_ACTUATOR))
      damage = damage / 2;

    if (Fallen(target) || MechType(target) != CLASS_MECH) {
      hitloc = FindTargetHitLoc(mech, target, &isrear, &iscritical);
    } else {

      hitgroup = FindAreaHitGroup(mech, target);
      if (hitgroup == BACK) {
        isrear = 1;
      }

      if (MechElevation(mech) > MechElevation(target)) {
        hitloc = FindPunchLocation(target, hitgroup);
      } else {
        hitloc = FindKickLocation(target, hitgroup);
      }
    }
    break;
  }

  if (glance) {
    damage = (damage + 1) / 2;
  }

  // Damage the target.
  MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear, iscritical,
               damage, 0);

  // If we've successfully hit a suit, knock him off.
  if (MechType(target) == CLASS_BSUIT && MechSwarmTarget(target) > 0 &&
      AttackType != PA_KICK) {
    StopSwarming(target, 0);
  }

  // If we kick our target (who is a mech), make a roll to see if he falls.
  if (MechType(target) == CLASS_MECH && AttackType == PA_KICK) {
    if (!MadePilotSkillRoll(target, 0) && !Fallen(target)) {
      mech_notify(target, MECHSTARTED, "The kick knocks you to the ground!");
      mech_los_broadcast(target, "stumbles and falls down!");
      MechFalls(target, 1, 0);
    }
  }

} // end PhysicalDamage()

#define DFA_SECTIONS 6
/* Rules make no distinction about Torso not needing recycled  We'll let Head
 * slide for now */

const int resect[CHARGE_SECTIONS] = {LARM, RARM, LLEG, RLEG, LTORSO, RTORSO};

/*
 * Executed at the end of a DFA
 */
int DeathFromAbove(Mech *mech, Mech *target) {
  int baseToHit = 5;
  int roll;
  int hitGroup;
  int hitloc;
  int isrear = 0;
  int iscritical = 0;
  int target_damage;
  int mech_damage;
  int spread;
  int i, tmpi;
  char location[50];
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);

  /* Weapons recycling check on each major section */
  for (i = 0; i < DFA_SECTIONS; i++)
    if (SectHasBusyWeap(mech, resect[i])) {
      ArmorStringFromIndex(resect[i], location, MechType(mech), MechMove(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return 0;
    }
  // Our target is no longer on the map.
  DOCHECKMA0((mech->mapindex != target->mapindex),
             "Your target is no longer valid.");

#ifdef BT_MOVEMENT_MODES
  DOCHECKMA0(Dodging(mech) || mech_move_mode_locked(mech),
             "You cannot use physicals while using a special movement mode.");
#endif

  DOCHECKMA0(MechSections(mech)[LLEG].recycle ||
                 MechSections(mech)[RLEG].recycle,
             "Your legs are still recovering from your last attack.");
  DOCHECKMA0(MechSections(mech)[RARM].recycle ||
                 MechSections(mech)[LARM].recycle,
             "Your arms are still recovering from your last attack.");

  DOCHECKMA0(Jumping(target),
             "Your target is airborne, you cannot land on it.");

  if ((MechType(target) == CLASS_VTOL) || (MechType(target) == CLASS_AERO) ||
      (MechType(target) == CLASS_DS))
    DOCHECKMA0(!Landed(target),
               "Your target is airborne, you cannot land on it.");

  DOCHECKMA0((MechTeam(mech) == MechTeam(target)) && MapNoFriendlyFire(map),
             "Friendly DFA? I don't think so....");
  if (mech->xcode.context->configuration->btech_phys_use_pskill)
    baseToHit = FindPilotPiloting(mech);

  baseToHit += (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                                 "melee_specialist")
                    ? MIN(0, AttackMovementMods(mech)) - 1
                    : AttackMovementMods(mech));
  baseToHit += TargetMovementMods(mech, target, 0.0);
  baseToHit += MechType(target) == CLASS_BSUIT ? 1 : 0;

#ifdef BT_MOVEMENT_MODES
  if (Dodging(target))
    baseToHit += 2;
#endif

  DOCHECKMA0(
      baseToHit > 12,
      tprintf("DFA: BTH %d\tYou choose not to attack and land from your jump.",
              baseToHit));

  roll = btech_random_roll(mech->xcode.context);
  mech_printf(mech, MECHALL, "DFA: BTH %d\tRoll: %d", baseToHit, roll);

  MechStatus(mech) &= ~JUMPING;
  MechStatus(mech) &= ~DFA_ATTACK;

  if (roll >= baseToHit) {
    /* OUCH */
    mech_printf(target, MECHSTARTED,
                "DEATH FROM ABOVE!!!\n%s lands on you from above!",
                mech_to_mech_display_id(target, mech).text);
    mech_notify(mech, MECHALL, "You land on your target legs first!");
    mech_los_broadcast_unit(mech, target, "lands on %s!");

    hitGroup = FindAreaHitGroup(mech, target);
    if (hitGroup == BACK)
      isrear = 1;

    target_damage = (3 * MechRealTons(mech)) / 10;

    if (MechTons(mech) % 10)
      target_damage++;

    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                         "melee_specialist"))
      target_damage++;

    spread = target_damage / 5;

    for (i = 0; i < spread; i++) {
      if (Fallen(target) || MechType(target) != CLASS_MECH)
        hitloc = FindHitLocation(target, hitGroup, &iscritical, &isrear);
      else
        hitloc = FindPunchLocation(target, hitGroup);

      MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear, iscritical,
                   5, 0);
    }

    if (target_damage % 5) {
      if (Fallen(target) || (MechType(target) != CLASS_MECH))
        hitloc = FindHitLocation(target, hitGroup, &iscritical, &isrear);
      else
        hitloc = FindPunchLocation(target, hitGroup);

      MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear, iscritical,
                   (target_damage % 5), 0);
    }

    mech_damage = MechTons(mech) / 5;

    spread = mech_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = FindKickLocation(mech, FRONT);
      MyDamageMech2(mech, mech, 0, -1, hitloc, 0, 0, 5, 0);
    }

    if (mech_damage % 5) {
      hitloc = FindKickLocation(mech, FRONT);
      MyDamageMech2(mech, mech, 0, -1, hitloc, 0, 0, (mech_damage % 5), 0);
    }

    if (!Fallen(mech)) {
      if (!MadePilotSkillRoll(mech, 4)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_los_broadcast(mech, "stumbles and falls down!");
        MechFalls(mech, 1, 0);
      }
      if (MechType(target) == CLASS_MECH && !MadePilotSkillRoll(target, 2)) {
        mech_notify(target, MECHSTARTED,
                    "Your piloting skill fails and you fall over!!");
        mech_los_broadcast(target, "stumbles and falls down!");
        MechFalls(target, 1, 0);
      }
    }

  } else {
    /* Missed DFA attack */
    if (!Fallen(mech)) {
      mech_notify(mech, MECHALL,
                  "You miss your DFA attack and fall on your back!!");
      mech_los_broadcast(mech, "misses DFA and falls down!");
    }

    mech_damage = MechTons(mech) / 5;
    spread = mech_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = FindHitLocation(mech, BACK, &iscritical, &tmpi);
      MyDamageMech2(mech, mech, 0, -1, hitloc, 1, iscritical, 5, 0);
    }

    if (mech_damage % 5) {
      hitloc = FindHitLocation(mech, BACK, &iscritical, &tmpi);
      MyDamageMech2(mech, mech, 0, -1, hitloc, 1, iscritical, (mech_damage % 5),
                    0);
    }

    /* now damage pilot */
    if (!MadePilotSkillRoll(mech, 2)) {
      mech_notify(mech, MECHALL, "You take personal injury from the fall!");
      headhitmwdamage(mech, mech, 1);
    }

    MechSpeed(mech) = 0.0;
    MechDesiredSpeed(mech) = 0.0;

    mech_make_fall(mech);
    MechZ(mech) = MechElevation(mech);
    MechFZ(mech) = MechZ(mech) * ZSCALE;

    if (MechZ(mech) < 0)
      MechFloods(mech);
  }

  for (i = 0; i < DFA_SECTIONS; i++)
    mech_set_recycle_limb(mech, resect[i], PHYSICAL_RECYCLE_TIME);

  return 1;
} // end DeathFromAbove()

/*
 * Executed when we're ready to finish the charge.
 */
