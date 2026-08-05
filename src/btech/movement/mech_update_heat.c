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
void mech_heat_update(Mech *mech) {

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
  mech_overheat_handle(mech);
}
