#include "mech_physical_internal.h"
void ChargeMech(Mech *mech, Mech *target) {
  int baseToHit = 5;
  int roll;
  int hitGroup;
  int hitloc;
  int isrear = 0;
  int iscritical = 0;
  int target_damage;
  int mech_damage;
  int received_damage;
  int inflicted_damage;
  int spread;
  int i;
  int mech_charge;
  int target_charge;
  int mech_baseToHit;
  int targ_baseToHit;
  int mech_roll;
  int targ_roll;
  int done = 0;
  char location[50];
  int ts, iwa;
  char emit_buff[LBUF_SIZE];

  /* Are they both charging ? */
  if (MechChargeTarget(target) == mech->mynum) {
    /* They are both charging each other */
    mech_charge = 1;
    target_charge = 1;

    /* Check the sections of the first unit for weapons that are cycling */
    done = 0;
    for (i = 0; i < CHARGE_SECTIONS && !done; i++) {
      if (SectHasBusyWeap(mech, resect[i])) {
        ArmorStringFromIndex(resect[i], location, MechType(mech),
                             MechMove(mech));
        mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                    location);
        mech_charge = 0;
        done = 1;
      }
    }

    /* Check the sections of the second unit for weapons that are cycling */
    done = 0;
    for (i = 0; i < CHARGE_SECTIONS && !done; i++) {
      if (SectHasBusyWeap(target, resect[i])) {
        ArmorStringFromIndex(resect[i], location, MechType(target),
                             MechMove(target));
        mech_printf(target, MECHALL, "You have weapons recycling on your %s.",
                    location);
        target_charge = 0;
        done = 1;
      }
    }

    /* Is the second unit capable of charging */
    if (!Started(target) || Uncon(target) || Blinded(target))
      target_charge = 0;
    /* Is the first unit capable of charging */
    if (!Started(mech) || Uncon(mech) || Blinded(mech))
      mech_charge = 0;

    /* Is the first unit moving fast enough to charge */
    if (MechSpeed(mech) < MP1) {
      mech_notify(mech, MECHALL, "You aren't moving fast enough to charge.");
      mech_charge = 0;
    }

    /* Is the second unit moving fast enough to charge */
    if (MechSpeed(target) < MP1) {
      mech_notify(target, MECHALL, "You aren't moving fast enough to charge.");
      target_charge = 0;
    }

    /* Check to see if any sections cycling from a previous attack */
    if (MechType(mech) == CLASS_MECH) {
      /* Is the first unit's legs cycling */
      if (MechSections(mech)[LLEG].recycle ||
          MechSections(mech)[RLEG].recycle) {
        mech_notify(mech, MECHALL,
                    "Your legs are still recovering from your last attack.");
        mech_charge = 0;
      }
      /* Is the first unit's arms cycling */
      if (MechSections(mech)[RARM].recycle ||
          MechSections(mech)[LARM].recycle) {
        mech_notify(mech, MECHALL,
                    "Your arms are still recovering from your last attack.");
        mech_charge = 0;
      }
    } else {
      /* Is the first unit's front side cycling */
      if (MechSections(mech)[FSIDE].recycle) {
        mech_notify(mech, MECHALL,
                    "You are still recovering from your last attack!");
        mech_charge = 0;
      }
    }

    /* Check to see if any sections cycling from a previous attack */
    if (MechType(target) == CLASS_MECH) {
      /* Is the second unit's legs cycling */
      if (MechSections(target)[LLEG].recycle ||
          MechSections(target)[RLEG].recycle) {
        mech_notify(target, MECHALL,
                    "Your legs are still recovering from your last attack.");
        target_charge = 0;
      }
      /* Is the second unit's arms cycling */
      if (MechSections(target)[RARM].recycle ||
          MechSections(target)[LARM].recycle) {
        mech_notify(target, MECHALL,
                    "Your arms are still recovering from your last attack.");
        target_charge = 0;
      }
    } else {
      /* Is the second unit's front side cycling */
      if (MechSections(target)[FSIDE].recycle) {
        mech_notify(target, MECHALL,
                    "You are still recovering from your last attack!");
        target_charge = 0;
      }
    }

    /* Is the second unit jumping */
    if (Jumping(target)) {
      mech_notify(mech, MECHALL,
                  "Your target is jumping, you charge underneath it.");
      mech_notify(target, MECHALL,
                  "You can't charge while jumping, try death from above.");
      mech_charge = 0;
      target_charge = 0;
    }

    /* Is the first unit jumping */
    if (Jumping(mech)) {
      mech_notify(target, MECHALL,
                  "Your target is jumping, you charge underneath it.");
      mech_notify(mech, MECHALL,
                  "You can't charge while jumping, try death from above.");
      mech_charge = 0;
      target_charge = 0;
    }

    /* Is the second unit fallen and the first unit not a tank */
    if (Fallen(target) && (MechType(mech) != CLASS_VEH_GROUND)) {
      mech_notify(mech, MECHALL, "Your target's too low for you to charge it!");
      mech_charge = 0;
    }

    /* Not sure at the moment if I need this here, but I figured
     * couldn't hurt for now */
    /* Is the first unit fallen and the second unit not a tank */
    if (Fallen(mech) && (MechType(target) != CLASS_VEH_GROUND)) {
      mech_notify(target, MECHALL,
                  "Your target's too low for you to charge it!");
      target_charge = 0;
    }

    /* If the second unit is a mech it can only charge mechs */
    if ((MechType(target) == CLASS_MECH) && (MechType(mech) != CLASS_MECH)) {
      mech_notify(target, MECHALL, "You can only charge mechs!");
      target_charge = 0;
    }

    /* If the first unit is a mech it can only charge mechs */
    if ((MechType(mech) == CLASS_MECH) && (MechType(target) != CLASS_MECH)) {
      mech_notify(mech, MECHALL, "You can only charge mechs!");
      mech_charge = 0;
    }

    /* If the second unit is a tank, it can only charge tanks and mechs */
    if ((MechType(target) == CLASS_VEH_GROUND) &&
        ((MechType(mech) != CLASS_MECH) &&
         (MechType(mech) != CLASS_VEH_GROUND))) {
      mech_notify(target, MECHALL, "You can only charge mechs and tanks!");
      target_charge = 0;
    }

    /* If the first unit is a tank, it can only charge tanks and mechs */
    if ((MechType(mech) == CLASS_VEH_GROUND) &&
        ((MechType(target) != CLASS_MECH) &&
         (MechType(target) != CLASS_VEH_GROUND))) {
      mech_notify(mech, MECHALL, "You can only charge mechs and tanks!");
      mech_charge = 0;
    }

    /* Are they stunned ? */
    if (mech_event_count(mech, EVENT_UNSTUN_CREW)) {
      mech_notify(mech, MECHALL, "You are too stunned to ram!");
      mech_charge = 0;
    }

    if (mech_event_count(target, EVENT_UNSTUN_CREW)) {
      mech_notify(target, MECHALL, "You are too stunned to ram!");
      target_charge = 0;
    }

    /* Are they trying to unjam their turrets ? */
    if (mech_event_count(mech, EVENT_UNJAM_TURRET)) {
      mech_notify(mech, MECHALL, "You are too busy unjamming your turret!");
      mech_charge = 0;
    }

    if (mech_event_count(target, EVENT_UNJAM_TURRET)) {
      mech_notify(mech, MECHALL, "You are too busy unjamming your turret!");
      target_charge = 0;
    }

    /* Check the arcs to make sure the target is in the front arc */
    ts = MechStatus(mech) & (TORSO_LEFT | TORSO_RIGHT);
    MechStatus(mech) &= ~ts;
    if (!(InWeaponArc(mech, MechFX(target), MechFY(target)) & FORWARDARC)) {
      mech_notify(mech, MECHALL,
                  "Your charge target is not in your forward arc and you are "
                  "unable to charge it.");
      mech_charge = 0;
    }
    MechStatus(mech) |= ts;

    ts = MechStatus(target) & (TORSO_LEFT | TORSO_RIGHT);
    MechStatus(mech) &= ~ts;
    if (!(InWeaponArc(target, MechFX(mech), MechFY(mech)) & FORWARDARC)) {
      mech_notify(target, MECHALL,
                  "Your charge target is not in your forward arc and you are "
                  "unable to charge it.");
      target_charge = 0;
    }
    MechStatus(mech) |= ts;

    /* Now to calculate how much damage the first unit will do */
    if (mech->xcode.context->configuration->btech_newcharge)
      target_damage =
          (((((float)MechChargeDistance(mech)) * MP1) -
            MechSpeed(target) *
                cos((MechFacing(mech) - MechFacing(target)) * (M_PI / 180.))) *
           MP_PER_KPH) *
          (MechRealTons(mech) + 5) / 10;
    else
      target_damage =
          ((MechSpeed(mech) -
            MechSpeed(target) *
                cos((MechFacing(mech) - MechFacing(target)) * (M_PI / 180.))) *
           MP_PER_KPH) *
          (MechRealTons(mech) + 5) / 10;

    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                         "melee_specialist"))
      target_damage++;

    /* Not able to do any damage */
    if (target_damage <= 0) {
      mech_notify(
          mech, MECHPILOT,
          "Your target unit will not sustain any damage. Charge aborted!");
      mech_charge = 0;
    }

    /* Now see how much damage the second unit will do */
    mech_damage = (MechRealTons(target) + 5) / 10;

    if (HasBoolAdvantage(mech->xcode.context, MechPilot(target),
                         "melee_specialist"))
      mech_damage++;

    /* Not able to do any damage */
    if (mech_damage <= 0) {
      mech_notify(target, MECHPILOT,
                  "Your unit won't sustain any dmage. Charge aborted!");
      target_charge = 0;
    }

    /* BTH for first unit */
    mech_baseToHit = 5;
    mech_baseToHit += FindPilotPiloting(mech) - FindPilotPiloting(target);

    mech_baseToHit += (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                                        "melee_specialist")
                           ? MIN(0, AttackMovementMods(mech) - 1)
                           : AttackMovementMods(mech));

    mech_baseToHit += TargetMovementMods(mech, target, 0.0);

#ifdef BT_MOVEMENT_MODES
    if (Dodging(target))
      mech_baseToHit += 2;
#endif

    /* BTH for second unit */
    targ_baseToHit = 5;
    targ_baseToHit += FindPilotPiloting(target) - FindPilotPiloting(mech);

    targ_baseToHit += (HasBoolAdvantage(mech->xcode.context, MechPilot(target),
                                        "melee_specialist")
                           ? MIN(0, AttackMovementMods(target) - 1)
                           : AttackMovementMods(target));

    targ_baseToHit += TargetMovementMods(target, mech, 0.0);

#ifdef BT_MOVEMENT_MODES
    if (Dodging(mech))
      targ_baseToHit += 2;
#endif

    /* Now check to see if its possible for them to even charge */
    if (mech_charge)
      if (mech_baseToHit > 12) {
        mech_printf(mech, MECHALL, "Charge: BTH %d\tYou choose not to charge.",
                    mech_baseToHit);
        mech_charge = 0;
      }

    if (target_charge)
      if (targ_baseToHit > 12) {
        mech_printf(target, MECHALL,
                    "Charge: BTH %d\tYou choose not to charge.",
                    targ_baseToHit);
        target_charge = 0;
      }

    /* Since neither can charge lets exit */
    if (!mech_charge && !target_charge) {
      /* MechChargeTarget(mech) and the others are set
         after the return */
      MechChargeTarget(target) = -1;
      MechChargeTimer(target) = 0;
      MechChargeDistance(target) = 0;
      return;
    }

    /* Roll */
    mech_roll = btech_random_roll(mech->xcode.context);
    targ_roll = btech_random_roll(mech->xcode.context);

    if (mech_charge)
      mech_printf(mech, MECHALL, "Charge: BTH %d\tRoll: %d", mech_baseToHit,
                  mech_roll);

    if (target_charge)
      mech_printf(target, MECHALL, "Charge: BTH %d\tRoll: %d", targ_baseToHit,
                  targ_roll);

    /* Ok the first unit made its roll */
    if (mech_charge && mech_roll >= mech_baseToHit) {
      /* OUCH */
      mech_printf(target, MECHALL, "CRASH!!!\n%s charges into you!",
                  mech_to_mech_display_id(target, mech).text);
      mech_notify(mech, MECHALL, "SMASH!!! You crash into your target!");
      hitGroup = mech_hit_group(mech, target);
      isrear = (hitGroup == BACK);

      /* Record the damage for debugging then dish it out */
      inflicted_damage = target_damage;
      spread = target_damage / 5;

      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
        MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear,
                     iscritical, 5, 0);
      }

      if (target_damage % 5) {
        hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
        MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear,
                     iscritical, (target_damage % 5), 0);
      }

      hitGroup = mech_hit_group(target, mech);
      isrear = (hitGroup == BACK);

      /* Ok now how much damage will the first unit take from
       * charging */
      if (mech->xcode.context->configuration->btech_newcharge &&
          mech->xcode.context->configuration->btech_tl3_charge)
        target_damage =
            (((((float)MechChargeDistance(mech)) * MP1) -
              MechSpeed(target) * cos((MechFacing(mech) - MechFacing(target)) *
                                      (M_PI / 180.))) *
             MP_PER_KPH) *
            (MechRealTons(mech) + 5) / 20;
      else
        target_damage = (MechRealTons(target) + 5) / 10; /* REUSED! */

      /* Record the damage for debugging then dish it out */
      received_damage = target_damage;
      spread = target_damage / 5;

      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
        MyDamageMech2(mech, mech, 0, -1, hitloc, isrear, iscritical, 5, 0);
      }

      if (target_damage % 5) {
        hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
        MyDamageMech2(mech, mech, 0, -1, hitloc, isrear, iscritical,
                      (target_damage % 5), 0);
      }

      /* Stop him */
      MechSpeed(mech) = 0;
      MechDesiredSpeed(mech) = 0;

      /* Emit the damage for debugging purposes */
      snprintf(emit_buff, LBUF_SIZE,
               "#%li charges #%li (%i/%i) Distance:"
               " %.2f DI: %i DR: %i",
               mech->mynum, target->mynum, mech_baseToHit, mech_roll,
               MechChargeDistance(mech), inflicted_damage, received_damage);
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         emit_buff);

      /* Make the first unit roll for doing the charge if it is a mech */
      if (MechType(mech) == CLASS_MECH && !MadePilotSkillRoll(mech, 2)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(mech, 1, 1);
      }
      /* Make the second unit roll for receiving the charge if it is a mech */
      if (MechType(mech) == CLASS_MECH && !MadePilotSkillRoll(target, 2)) {
        mech_notify(target, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(target, 1, 1);
      }
    }

    /* Ok the second unit made its roll */
    if (target_charge && targ_roll >= targ_baseToHit) {
      /* OUCH */
      mech_printf(mech, MECHALL, "CRASH!!!\n%s charges into you!",
                  mech_to_mech_display_id(mech, target).text);
      mech_notify(target, MECHALL, "SMASH!!! You crash into your target!");
      hitGroup = mech_hit_group(target, mech);
      isrear = (hitGroup == BACK);

      /* Record the damage for debugging then dish it out */
      inflicted_damage = mech_damage;
      spread = mech_damage / 5;

      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
        MyDamageMech(mech, target, 1, MechPilot(target), hitloc, isrear,
                     iscritical, 5, 0);
      }

      if (mech_damage % 5) {
        hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
        MyDamageMech(mech, target, 1, MechPilot(target), hitloc, isrear,
                     iscritical, (mech_damage % 5), 0);
      }

      hitGroup = mech_hit_group(mech, target);
      isrear = (hitGroup == BACK);

      /* Ok now how much damage will the second unit take from
       * charging */
      if (mech->xcode.context->configuration->btech_newcharge &&
          mech->xcode.context->configuration->btech_tl3_charge)
        target_damage =
            (((((float)MechChargeDistance(target)) * MP1) -
              MechSpeed(mech) * cos((MechFacing(target) - MechFacing(mech)) *
                                    (M_PI / 180.))) *
             MP_PER_KPH) *
            (MechRealTons(mech) + 5) / 20;
      else
        target_damage = (MechRealTons(mech) + 5) / 10; /* REUSED! */

      /* Record the damage for debugging then dish it out */
      received_damage = target_damage;
      spread = target_damage / 5;

      for (i = 0; i < spread; i++) {
        hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
        MyDamageMech2(target, target, 0, -1, hitloc, isrear, iscritical, 5, 0);
      }

      if (mech_damage % 5) {
        hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
        MyDamageMech2(target, target, 0, -1, hitloc, isrear, iscritical,
                      (mech_damage % 5), 0);
      }

      /* Stop him */
      MechSpeed(target) = 0;
      MechDesiredSpeed(target) = 0;

      /* Emit the damage for debugging purposes */
      snprintf(emit_buff, LBUF_SIZE,
               "#%li charges #%li (%i/%i) Distance:"
               " %.2f DI: %i DR: %i",
               target->mynum, mech->mynum, targ_baseToHit, targ_roll,
               MechChargeDistance(target), inflicted_damage, received_damage);
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                         emit_buff);

      if (MechType(mech) == CLASS_MECH && !MadePilotSkillRoll(mech, 2)) {
        mech_notify(mech, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(mech, 1, 1);
      }
      if (MechType(target) == CLASS_MECH && !MadePilotSkillRoll(target, 2)) {
        mech_notify(target, MECHALL,
                    "Your piloting skill fails and you fall over!!");
        mech_fall(target, 1, 1);
      }
    }

    /* Cycle the sections so they can't make another attack for a while */
    if (MechType(mech) == CLASS_MECH) {
      for (i = 0; i < CHARGE_SECTIONS; i++)
        mech_set_recycle_limb(mech, resect[i], PHYSICAL_RECYCLE_TIME);
    } else {
      mech_set_recycle_limb(mech, FSIDE, PHYSICAL_RECYCLE_TIME);
      mech_set_recycle_limb(mech, TURRET, PHYSICAL_RECYCLE_TIME);
    }

    if (MechType(target) == CLASS_MECH) {
      for (i = 0; i < CHARGE_SECTIONS; i++)
        mech_set_recycle_limb(target, resect[i], PHYSICAL_RECYCLE_TIME);
    } else {
      mech_set_recycle_limb(target, FSIDE, PHYSICAL_RECYCLE_TIME);
      mech_set_recycle_limb(target, TURRET, PHYSICAL_RECYCLE_TIME);
    }

    /* MechChargeTarget(mech) and the others are set
       after the return */
    MechChargeTarget(target) = -1;
    MechChargeTimer(target) = 0;
    MechChargeDistance(target) = 0;
    return;
  }

  /* Check to see if any weapons cycling in any of the sections */
  for (i = 0; i < CHARGE_SECTIONS; i++) {
    if (SectHasBusyWeap(mech, i)) {
      ArmorStringFromIndex(i, location, MechType(mech), MechMove(mech));
      mech_printf(mech, MECHALL, "You have weapons recycling on your %s.",
                  location);
      return;
    }
  }

  /* Check if they going fast enough to charge */
  DOCHECKMA(MechSpeed(mech) < MP1, "You aren't moving fast enough to charge.");

  /* Check to see if their sections cycling */
  if (MechType(mech) == CLASS_MECH) {
    DOCHECKMA(MechSections(mech)[LLEG].recycle ||
                  MechSections(mech)[RLEG].recycle,
              "Your legs are still recovering from your last attack.");
    DOCHECKMA(MechSections(mech)[RARM].recycle ||
                  MechSections(mech)[LARM].recycle,
              "Your arms are still recovering from your last attack.");
  } else {
    DOCHECKMA(MechSections(mech)[FSIDE].recycle,
              "You are still recovering from your last attack!");
  }

  /* See if either the target or the attacker are jumping */
  DOCHECKMA(Jumping(target),
            "Your target is jumping, you charge underneath it.");
  DOCHECKMA(Jumping(mech),
            "You can't charge while jumping, try death from above.");

  /* If target is fallen make sure you in a tank */
  DOCHECKMA(Fallen(target) && (MechType(mech) != CLASS_VEH_GROUND),
            "Your target's too low for you to charge it!");

  /* Only mechs can charge mechs */
  DOCHECKMA((MechType(mech) == CLASS_MECH) && (MechType(target) != CLASS_MECH),
            "You can only charge mechs!");

  /* Only tanks can charge tanks and mechs */
  DOCHECKMA((MechType(mech) == CLASS_VEH_GROUND) &&
                ((MechType(target) != CLASS_MECH) &&
                 (MechType(target) != CLASS_VEH_GROUND)),
            "You can only charge mechs and tanks!");

  /* Check the arc make sure target is in front arc */
  ts = MechStatus(mech) & (TORSO_LEFT | TORSO_RIGHT);
  MechStatus(mech) &= ~ts;
  iwa = InWeaponArc(mech, MechFX(target), MechFY(target));
  MechStatus(mech) |= ts;
  DOCHECKMA(!(iwa & FORWARDARC), "Your charge target is not in your forward "
                                 "arc and you are unable to charge it.");

  /* Damage inflicted by the charge */
  if (mech->xcode.context->configuration->btech_newcharge)
    target_damage =
        (((((float)MechChargeDistance(mech)) * MP1) -
          MechSpeed(target) *
              cos((MechFacing(mech) - MechFacing(target)) * (M_PI / 180.))) *
         MP_PER_KPH) *
            (MechRealTons(mech) + 5) / 10 +
        1;
  else
    target_damage =
        ((MechSpeed(mech) -
          MechSpeed(target) *
              cos((MechFacing(mech) - MechFacing(target)) * (M_PI / 180.))) *
         MP_PER_KPH) *
            (MechRealTons(mech) + 5) / 10 +
        1;

  if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                       "melee_specialist"))
    target_damage++;

  /* Not enough damage done so no charge */
  DOCHECKMP(target_damage <= 0,
            "Your target pulls away from you and you are unable to charge it.");

  /* BTH */
  baseToHit += FindPilotPiloting(mech) - FindSPilotPiloting(target);

  baseToHit += (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                                 "melee_specialist")
                    ? MIN(0, AttackMovementMods(mech) - 1)
                    : AttackMovementMods(mech));

  baseToHit += TargetMovementMods(mech, target, 0.0);

#ifdef BT_MOVEMENT_MODES
  if (Dodging(target))
    baseToHit += 2;
#endif

  DOCHECKMA(baseToHit > 12,
            tprintf("Charge: BTH %d\tYou choose not to charge.", baseToHit));

  /* Roll */
  roll = btech_random_roll(mech->xcode.context);
  mech_printf(mech, MECHALL, "Charge: BTH %d\tRoll: %d", baseToHit, roll);

  /* Did the charge work ? */
  if (roll >= baseToHit) {
    /* OUCH */
    mech_los_broadcast_unit(
        mech, target,
        tprintf("%ss %%s!", MechType(mech) == CLASS_MECH ? "charge" : "ram"));
    mech_printf(target, MECHSTARTED, "CRASH!!!\n%s %ss into you!",
                mech_to_mech_display_id(target, mech).text,
                MechType(mech) == CLASS_MECH ? "charge" : "ram");
    mech_notify(mech, MECHALL, "SMASH!!! You crash into your target!");
    hitGroup = mech_hit_group(mech, target);

    if (hitGroup == BACK)
      isrear = 1;
    else
      isrear = 0;

    /* Record the damage then dish it out */
    inflicted_damage = target_damage;
    spread = target_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
      MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear, iscritical,
                   5, 0);
    }

    if (target_damage % 5) {
      hitloc = mech_hit_location(target, hitGroup, &iscritical, &isrear);
      MyDamageMech(target, mech, 1, MechPilot(mech), hitloc, isrear, iscritical,
                   (target_damage % 5), 0);
    }

    hitGroup = mech_hit_group(target, mech);
    isrear = (hitGroup == BACK);

    /* Damage done to the attacker for the charge */
    if (mech->xcode.context->configuration->btech_newcharge &&
        mech->xcode.context->configuration->btech_tl3_charge)
      mech_damage =
          (((((float)MechChargeDistance(mech)) * MP1) -
            MechSpeed(target) *
                cos((MechFacing(mech) - MechFacing(target)) * (M_PI / 180.))) *
           MP_PER_KPH) *
          (MechRealTons(target) + 5) / 20;
    else
      mech_damage = (MechRealTons(target) + 5) / 10;

    /* Record the damage then dish it out */
    received_damage = mech_damage;
    spread = mech_damage / 5;

    for (i = 0; i < spread; i++) {
      hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
      MyDamageMech2(mech, mech, 0, -1, hitloc, isrear, iscritical, 5, 0);
    }

    if (mech_damage % 5) {
      hitloc = mech_hit_location(mech, hitGroup, &iscritical, &isrear);
      MyDamageMech2(mech, mech, 0, -1, hitloc, isrear, iscritical,
                    (mech_damage % 5), 0);
    }

    /* Force piloting roll for attacker if they are in a mech */
    if (MechType(mech) == CLASS_MECH && !MadePilotSkillRoll(mech, 2)) {
      mech_notify(mech, MECHALL,
                  "Your piloting skill fails and you fall over!!");
      mech_fall(mech, 1, 1);
    }

    /* Force piloting roll for target if they are in a mech */
    if (MechType(target) == CLASS_MECH && !MadePilotSkillRoll(target, 2)) {
      mech_notify(target, MECHSTARTED,
                  "Your piloting skill fails and you fall over!!");
      mech_fall(target, 1, 1);
    }

    /* Stop him */
    MechSpeed(mech) = 0;
    MechDesiredSpeed(mech) = 0;

    /* Emit the damage for debugging purposes */
    snprintf(emit_buff, LBUF_SIZE,
             "#%li charges #%li (%i/%i) Distance:"
             " %.2f DI: %i DR: %i",
             mech->mynum, target->mynum, baseToHit, roll,
             MechChargeDistance(mech), inflicted_damage, received_damage);
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       emit_buff);
  }

  /* Cycle the sections so they can't make another attack for a while */
  if (MechType(mech) == CLASS_MECH) {
    for (i = 0; i < CHARGE_SECTIONS; i++)
      mech_set_recycle_limb(mech, resect[i], PHYSICAL_RECYCLE_TIME);
  } else {
    mech_set_recycle_limb(mech, FSIDE, PHYSICAL_RECYCLE_TIME);
    mech_set_recycle_limb(mech, TURRET, PHYSICAL_RECYCLE_TIME);
  }
  return;
} // end ChargeMech()

/*
 * Checks to see if we can grab a club with our arms.
 */
