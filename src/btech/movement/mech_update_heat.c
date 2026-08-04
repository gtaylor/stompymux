/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include "mech_update_internal.h"

int OverheatMods(Mech *mech) {
  int returnValue;

  if (MechHeat(mech) >= 24.) {
    /* +4 to fire... */
    returnValue = 4;
  } else if (MechHeat(mech) >= 17.) {
    /* +3 to fire... */
    returnValue = 3;
  } else if (MechHeat(mech) >= 13.) {
    /* +2 to fire... */
    returnValue = 2;
  } else if (MechHeat(mech) >= 8.) {
    /* +1 to fire... */
    returnValue = 1;
  } else {
    returnValue = 0;
  }
  return (returnValue);
}

void ammo_explosion(Mech *attacker, Mech *mech, int ammoloc, int ammocritnum,
                    int damage) {
  if (MechType(mech) == CLASS_MW) {
    mech_notify(mech, MECHALL, "Your weapon's ammo explodes!");
    MechLOSBroadcast(mech, "'s weapon's ammo explodes!");
  } else {
    mech_notify(mech, MECHALL, "Ammunition explosion!");
    if (GetPartAmmoMode(mech, ammoloc, ammocritnum) & INFERNO_MODE)
      MechLOSBroadcast(mech, "is suddenly enveloped by a brilliant fireball!");
    else
      MechLOSBroadcast(mech, "has an internal ammo explosion!");
  }
  DestroyPart(mech, ammoloc, ammocritnum);
  if (!attacker)
    return;
  if (GetPartAmmoMode(mech, ammoloc, ammocritnum) & INFERNO_MODE) {
    Inferno_Hit(mech, mech, damage / 4, 0);
    if (mech->xcode.context->configuration->btech_inferno_penalty)
      MechWeapHeat(mech) += 30.0;
    damage = damage / 2;
  }
  if (MechType(mech) == CLASS_BSUIT)
    DamageMech(mech, attacker, 0, -1, ammoloc, 0, 0, damage, 0, -1, 0, -1, 0,
               0);
  else
    DamageMech(mech, attacker, 0, -1, ammoloc, 0, 0, -1, damage, -1, 0, -1, 0,
               0);

  /* Rule Reference: BMR Revised, Page 16-17 (Ammo Explosion=2 Bruise) */
  /* Rule Reference: Total Warfare, Page 41 (Ammo Explosion=2 Bruise) */

  if (MechType(mech) != CLASS_BSUIT) {
    mech_notify(mech, MECHPILOT,
                "You take personal injury from the ammunition explosion!");

    /* Rule Reference: MaxTech Revised, Page 46 (Reduce by 1 because of pain
     * resistance) */

    if (HasBoolAdvantage(mech->xcode.context, MechPilot(mech),
                         "pain_resistance"))
      headhitmwdamage(mech, mech, 1);
    else
      headhitmwdamage(mech, mech, 2);
  }
}

void HandleOverheat(Mech *mech) {
  int avoided = 0, hasinferno = 0;
  BattleMap *mech_map;
  int ammoloc, ammocritnum, damage = 0;

  if (MechHeat(mech) < 10.)
    return;
  /* Has it been a TURN already ? */
  if ((MechHeatLast(mech) + TURN) > mech->xcode.context->events->tick)
    return;
  MechHeatLast(mech) = mech->xcode.context->events->tick;

  /* Ammo - done first so infernobooms shut you down */
  if (MechHeat(mech) >= 10.) {
    if (mech->xcode.context->configuration->btech_inferno_penalty)
      hasinferno = FindInfernoAmmo(mech, &ammoloc, &ammocritnum);
    if (MechHeat(mech) >= 28.) {
      /* Ammo explosion (Avoid 8+, infernos 12+) */
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 12)
          avoided = 1;
      } else {
        if (btech_random_roll(mech->xcode.context) >= 8)
          avoided = 1;
      }
    } else if (MechHeat(mech) >= 23.) {
      /* Ammo explosion (Avoid 6+, infernos 10+) */
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 10)
          avoided = 1;
      } else {
        if (btech_random_roll(mech->xcode.context) >= 6)
          avoided = 1;
      }
    } else if (MechHeat(mech) >= 19.) {
      /* Ammo explosion (Avoid 4+, infernos 8+) */
      if (hasinferno) {
        if (btech_random_roll(mech->xcode.context) >= 8)
          avoided = 1;
      } else {
        if (btech_random_roll(mech->xcode.context) >= 4)
          avoided = 1;
      }

    } else if ((MechHeat(mech) >= 14.) && (hasinferno)) {
      if (btech_random_roll(mech->xcode.context) >= 6)
        avoided = 1;
    } else if ((MechHeat(mech) >= 10.) && (hasinferno)) {
      if (btech_random_roll(mech->xcode.context) >= 4)
        avoided = 1;
    } else if ((MechHeat(mech) < 19.) && (!hasinferno)) {
      avoided = 1;
    }

    if (!(avoided)) {
      if (!hasinferno)
        damage = FindDestructiveAmmo(mech, &ammoloc, &ammocritnum);
      else
        damage = hasinferno;
      if (damage) {
        /* BOOM! */
        /* That's going to hurt... */
        ammo_explosion(mech, mech, ammoloc, ammocritnum, damage);
      } else
        mech_notify(mech, MECHALL, "You have no ammunition, lucky you!");
    }
  }

  avoided = 0;
#ifdef BT_EXILE_MW3STATS
  if (!is_player(mech->xcode.context->database, MechPilot(mech))) {
#endif
    if (MechHeat(mech) >= 30.) {
      /* Shutdown */
    } else if (MechHeat(mech) >= 26.) {
      /* Shutdown avoid on 10+ */
      if (btech_random_roll(mech->xcode.context) >= 10)
        avoided = 1;
    } else if (MechHeat(mech) >= 22.) {
      /* Shutdown avoid on 8+ */
      if (btech_random_roll(mech->xcode.context) >= 8)
        avoided = 1;
    } else if (MechHeat(mech) >= 18.) {
      /* Shutdown avoid on 6+ */
      if (btech_random_roll(mech->xcode.context) >= 6)
        avoided = 1;
    } else if (MechHeat(mech) >= 14.) {
      /* Shutdown avoid on 4+ */
      if (btech_random_roll(mech->xcode.context) >= 4)
        avoided = 1;
    }
#ifdef BT_EXILE_MW3STATS
  } else {
    avoided = 1;
    if (MechHeat(mech) >= 14.) {
      mech_notify(mech, MECHALL,
                  "You frantically attempt to override the shutdown process!");
      avoided =
          char_getskillsuccess(mech->xcode.context, MechPilot(mech), "computer",
                               (MechHeat(mech) >= 30.   ? 8
                                : MechHeat(mech) >= 26. ? 6
                                : MechHeat(mech) >= 22. ? 4
                                : MechHeat(mech) >= 18. ? 2
                                                        : 0),
                               1);
      if (avoided)
        AccumulateComputerXP(MechPilot(mech), mech, 1);
    }
  }
#endif
  if (!(avoided) && Started(mech)) {
    if (MechStatus(mech) & STARTED)
      mech_notify(mech, MECHALL,
                  "[fg=red inverse]Reactor shutting down...[reset]");
    if (MechStatus2(mech) & SLITE_ON) {
      mech_notify(mech, MECHALL, "Your searchlight shuts off.");
      MechStatus2(mech) &= ~SLITE_ON;
      MechCritStatus(mech) &= ~SLITE_LIT;
    }
    if (Jumping(mech) || OODing(mech) || (is_aero(mech) && !Landed(mech))) {
      mech_notify(mech, MECHALL, "[bold]You fall from the sky![reset]");
      MechLOSBroadcast(mech, "falls from the sky!");
      mech_map = btech_context_get_map(mech->xcode.context, mech->mapindex);
      MechFalls(mech, JumpSpeedMP(mech, mech_map), 0);
      domino_space(mech, 2);
    } else {
      MechLOSBroadcast(mech, "stops in mid-motion!");
      if ((fabs(MechSpeed(mech)) > MP1) && !Fallen(mech) &&
          (!MadePilotSkillRoll(mech, 3)))
        MechFalls(mech, 0, 1);
    }
    mech_power_down(mech);
    mech_event_cancel(mech, EVENT_MOVE);
    mech_event_cancel(mech, EVENT_STAND);
  }
}

static int EnableSomeHS(Mech *mech, int numsinks) {

  numsinks = MIN(numsinks,
                 (MechSpecials(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)) ? 4 : 2);
  numsinks = MIN(numsinks, MechDisabledHS(mech));

  if (!numsinks)
    return 0;

  MechDisabledHS(mech) -= numsinks;
  MechMinusHeat(mech) += numsinks; /* We dont check for water and
                                                                      such after
                                      enabling them, only the next tic. */
#ifdef HEATCUTOFF_DEBUG
  mech_printf(mech, MECHALL,
              "[fg=green]%d heatsink%s kick%s into action.[reset]", numsinks,
              numsinks == 1 ? "" : "s", numsinks == 1 ? "s" : "");
#endif

  return numsinks;
}

static int DisableSomeHS(Mech *mech, int numsinks) {

  numsinks = MIN(numsinks,
                 (MechSpecials(mech) & (DOUBLE_HEAT_TECH | CLAN_TECH)) ? 4 : 2);
  numsinks = MIN(numsinks, MechActiveNumsinks(mech));

  if (!numsinks)
    return 0;

  MechDisabledHS(mech) += numsinks;
  MechMinusHeat(mech) -=
      numsinks; /* Submerged heatsinks silently
                                                   still dissipate some heat */
#ifdef HEATCUTOFF_DEBUG
  mech_printf(mech, MECHALL,
              "[fg=yellow]%d heatsink%s hum%s into silence.[reset]", numsinks,
              numsinks == 1 ? "" : "s", numsinks == 1 ? "s" : "");
#endif

  return numsinks;
}

/* Update the Unit's current heat values as well as
 * send messages to the pilot based on heat level */
void UpdateHeat(Mech *mech) {

  int legsinks;
  float maxspeed;
  float intheat;
  float inheat;
  BattleMap *map;

  // These guys don't get heat updates.
  if (!MechHasHeat(mech))
    return;

  inheat = MechHeat(mech);
  maxspeed = MMaxSpeed(mech);
  MechPlusHeat(mech) = 0.;

  if (MechTerrain(mech) == FIRE && MechType(mech) == CLASS_MECH)
    MechPlusHeat(mech) += 5.;

  /* We do a trick here.  We look at the previous heat level to determine
   * if TSM is/was on.  If it is/was, we recalc what running and walk speeds are
   * to better set how much heat the unit is putting out */
  if (MechSpecials(mech) & TRIPLE_MYOMER_TECH) {
    if (inheat >= 9.)
      maxspeed = ceil((rint((MMaxSpeed(mech) / 1.5) / MP1) + 1) * 1.5) * MP1;
  }

  if (fabs(MechSpeed(mech)) > 0.0) {
#ifndef BT_MOVEMENT_MODES
    if (IsRunning(MechDesiredSpeed(mech), maxspeed))
      MechPlusHeat(mech) += 2.;
#else
    if (Sprinting(mech) || Evading(mech))
      MechPlusHeat(mech) += 3.;
    else if (IsRunning(MechDesiredSpeed(mech), maxspeed))
      MechPlusHeat(mech) += 2.;
#endif
    else
      MechPlusHeat(mech) += 1.;
  }

  if (Jumping(mech))
    MechPlusHeat(mech) += (MechJumpSpeed(mech) * MP_PER_KPH > 3.)
                              ? MechJumpSpeed(mech) * MP_PER_KPH
                              : 3.;

  if (Started(mech))
    MechPlusHeat(mech) += (float)MechEngineHeat(mech);

  if (StealthArmorActive(mech))
    MechPlusHeat(mech) += 10;

  if (NullSigSysActive(mech))
    MechPlusHeat(mech) += 10;

  intheat = MechPlusHeat(mech);

  MechPlusHeat(mech) += MechWeapHeat(mech);

  /* ADD Water effects here */
  if (InWater(mech) && MechZ(mech) <= -1) {
    legsinks = FindLegHeatSinks(mech);
    legsinks = (legsinks > 4) ? 4 : legsinks;
    if (MechZ(mech) == -1 && !Fallen(mech)) {
      MechMinusHeat(mech) = MIN(2 * MechActiveNumsinks(mech),
                                legsinks + MechActiveNumsinks(mech));
    } else {
      MechMinusHeat(mech) =
          MIN(2 * MechActiveNumsinks(mech), 6 + MechActiveNumsinks(mech));
    }
  } else {
    MechMinusHeat(mech) = (float)(MechActiveNumsinks(mech));
  }

  /* Infernoed */
  if (Jellied(mech)) {
    MechMinusHeat(mech) = MechMinusHeat(mech) - 6;
    if (MechMinusHeat(mech) < 0)
      MechMinusHeat(mech) = 0;
  }

  if (InSpecial(mech))
    if ((map = btech_context_find_object(mech->xcode.context, mech->mapindex)))
      if (MapUnderSpecialRules(map))
        if (MapTemperature(map) < -30 || MapTemperature(map) > 50) {
          if (MapTemperature(map) < -30)
            MechMinusHeat(mech) += (-30 - MapTemperature(map) + 9) / 10;
          else
            MechMinusHeat(mech) -= (MapTemperature(map) - 50 + 9) / 10;
        }

  /* Handle heat cutoff now */
  /* En/DisableSomeHS() take care of MechMinusHeat also. */
  /* Re-Written to use Exile's code - Dany 12/05 */
  if (Heatcutoff(mech)) {
    float overheat = MechPlusHeat(mech) - MechMinusHeat(mech);

    if (overheat >= 10.)
      EnableSomeHS(mech, floor(overheat - 10.) + 1);
    else if (overheat < 9.)
      DisableSomeHS(mech, floor(9. - overheat) + 1);

  } else if (MechDisabledHS(mech)) {
    EnableSomeHS(mech, 100);
  }

  MechHeat(mech) = MechPlusHeat(mech) - MechMinusHeat(mech);

  /* No lowering of heat if heat is under 9 */
  MechWeapHeat(mech) -= (MechMinusHeat(mech) - intheat) / WEAPON_RECYCLE_TIME;

  if (MechWeapHeat(mech) < 0.0)
    MechWeapHeat(mech) = 0.0;

  if (MechHeat(mech) < 0.0)
    MechHeat(mech) = 0.0;

  /* Rule Reference: BMR Revised, Page 17 (Heat=>26 +2 Bruise, Heat=>15 +1
   * Bruise, w/o Lifesupport) */
  /* Rule Reference: Total Warfare, Page 42 (Heat=>26 +2 Bruise, Heat=>15 +1
   * Bruise, w/o Lifesupport) */
  /* Custom Rule: Give bruise if heat > 30 and Random 0 or 1 */

  if ((mech->xcode.context->events->tick % TURN) == 0)
    if (MechCritStatus(mech) & LIFE_SUPPORT_DESTROYED ||
        (MechHeat(mech) > 30. &&
         btech_random_range(mech->xcode.context, 0, 1) == 0)) {
      if (MechHeat(mech) > 25.) {
        mech_notify(mech, MECHPILOT, "You take personal injury from heat!");
        headhitmwdamage(mech, mech,
                        MechCritStatus(mech) & LIFE_SUPPORT_DESTROYED ? 2 : 1);
      } else if (MechHeat(mech) >= 15.) {
        mech_notify(mech, MECHPILOT, "You take personal injury from heat!");
        headhitmwdamage(mech, mech, 1);
      }
    }

  if (MechHeat(mech) >= 19.) {
    if (inheat < 19.) {
      mech_notify(mech, MECHALL,
                  "[fg=red bold]=====================================\n"
                  "Your Excess Heat indicator turns RED!\n"
                  "=====================================[reset]");
    }
  } else if (MechHeat(mech) >= 14.) {
    if (inheat >= 19. || inheat < 14.) {
      mech_notify(mech, MECHALL,
                  "[fg=yellow bold]=======================================\n"
                  "Your Excess Heat indicator turns YELLOW\n"
                  "=======================================[reset]");
    }
  } else {
    if (inheat >= 14.) {
      mech_notify(mech, MECHALL,
                  "[fg=green]======================================\n"
                  "Your Excess Heat indicator turns GREEN\n"
                  "======================================[reset]");
    }
  }
  HandleOverheat(mech);
}
