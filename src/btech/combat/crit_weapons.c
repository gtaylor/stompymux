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

int handleWeaponCrit(Mech *attacker, Mech *wounded, int hitloc, int critHit,
                     int critType, int LOS) {
  int wMaxCrits, wFirstCrit, wWeapDestroyed = 0;
  int wAmmoSection, wAmmoCritSlot;
  int damage;
  char locname[30];
  char msgbuf[MBUF_SIZE] = {0};

  ArmorStringFromIndex(hitloc, locname, MechType(wounded), MechMove(wounded));

  /* Get the max number of crits for this weapon */
  wMaxCrits = GetWeaponCrits(wounded, Weapon2I(critType));

  /* Find the first crit */
  wFirstCrit =
      FindFirstWeaponCrit(wounded, hitloc, critHit, 0, critType, wMaxCrits);

  /* See if the weapon is already destroyed */
  if (wFirstCrit != -1) {
    wWeapDestroyed =
        (PartIsNonfunctional(wounded, hitloc, wFirstCrit) ||
         (PartTempNuke(wounded, hitloc, wFirstCrit) == FAIL_DESTROYED));
  }

  /* Gauss rifle-ish weapons explode when critted */
  if ((MechWeapons[Weapon2I(critType)].special & GAUSS) && !wWeapDestroyed) {
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                &MechWeapons[Weapon2I(critType)].name[3]);

    mech_printf(wounded, MECHALL, "It explodes for %d points damage.",
                MechWeapons[Weapon2I(critType)].explosiondamage);

    if (!Destroyed(wounded)) {
      snprintf(msgbuf, MBUF_SIZE,
               "'s %s is covered in a large electrical discharge!", locname);
      MechLOSBroadcast(wounded, msgbuf);
    }

    DestroyWeapon(wounded, hitloc, critType, wFirstCrit, wMaxCrits, wMaxCrits);

    if (attacker) {
      DamageMech(wounded, attacker, 0, -1, hitloc, 0, 0, 0,
                 MechWeapons[Weapon2I(critType)].explosiondamage, -1, 7, -1, 0,
                 1);
    }
    /* Rule Reference: BMR Revised, Page 16-17 (Ammo Explosion=2 Bruise) */
    /* Rule Reference: Total Warfare, Page 41 (Ammo Explosion=2 Bruise) */

    if (MechType(wounded) != CLASS_BSUIT) {
      mech_notify(wounded, MECHPILOT,
                  "You take personal injury from the weapon's explosion!");

      /* Rule Reference: MaxTech Revised, Page 46 (Reduce by 1 because of pain
       * resistance) */

      if (HasBoolAdvantage(wounded->xcode.context, MechPilot(wounded),
                           "pain_resistance"))
        headhitmwdamage(wounded, wounded, 1);
      else
        headhitmwdamage(wounded, wounded, 2);
    }
    return 1;
  } else if (IsAMS(Weapon2I(
                 critType))) { /* Have to shut down AMS when its critted */
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                &MechWeapons[Weapon2I(critType)].name[3]);

    MechStatus(wounded) &= ~AMS_ENABLED;
    MechSpecials(wounded) &= ~(IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH);
  } else if (HotLoading(Weapon2I(critType),
                        GetPartFireMode(wounded, hitloc, wFirstCrit)) &&
             !wWeapDestroyed) { /* And crit hotloaded LRMs */
    if (FindAmmoForWeapon(wounded, Weapon2I(critType), 0, &wAmmoSection,
                          &wAmmoCritSlot) > 0) {
      damage = MechWeapons[Weapon2I(critType)].damage;

      if (IsMissile(Weapon2I(critType)) || IsArtillery(Weapon2I(critType))) {
        const MissileHitEntry *entry = missile_hit_registry_find_weapon(
            &wounded->xcode.context->missile_hits, Weapon2I(critType));
        if (entry != nullptr)
          damage *= entry->num_missiles[10];
      }

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  &MechWeapons[Weapon2I(critType)].name[3]);

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]Your hotloaded launcher explodes for %d points of "
          "damage![reset]",
          damage);

      if (!Destroyed(wounded)) {
        snprintf(msgbuf, MBUF_SIZE,
                 " loses a launcher in a brilliant explosion!");
        MechLOSBroadcast(wounded, msgbuf);
      }

      DestroyWeapon(wounded, hitloc, critType, wFirstCrit, wMaxCrits,
                    wMaxCrits);

      if (attacker) {
        DamageMech(wounded, attacker, 0, -1, hitloc, 0, 0, 0, damage, -1, 7, -1,
                   0, 1);
      }

      return 1;
    }
  } else if ((GetPartAmmoMode(wounded, hitloc, wFirstCrit) &
              AC_INCENDIARY_MODE) &&
             !wWeapDestroyed &&
             WpnIsRecycling(wounded, hitloc,
                            wFirstCrit)) { /* Incendiary ACs blow up too */

    if (FindAmmoForWeapon_sub(wounded, -1, -1, Weapon2I(critType), 0,
                              &wAmmoSection, &wAmmoCritSlot, 0,
                              AC_INCENDIARY_MODE) > 0) {

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  &MechWeapons[Weapon2I(critType)].name[3]);

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]The incendiary ammunition in your launcher ignites "
          "for %d points of damage![reset]",
          MechWeapons[Weapon2I(critType)].damage);

      if (!Destroyed(wounded)) {
        snprintf(msgbuf, MBUF_SIZE,
                 "'s %s is engulfed in a brilliant blue flame!", locname);
        MechLOSBroadcast(wounded, msgbuf);
      }

      DestroyWeapon(wounded, hitloc, critType, wFirstCrit, wMaxCrits,
                    wMaxCrits);

      if (attacker) {
        DamageMech(wounded, attacker, 0, -1, hitloc, 0, 0, 0,
                   MechWeapons[Weapon2I(critType)].damage, -1, 7, -1, 0, 1);

        return 1;
      }
    }
  }

  return 0;
}

void JamMainWeapon(Mech *mech) {
  unsigned char weaparray[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  int critical[MAX_WEAPS_SECTION];
  int count;
  int loop;
  int ii;
  int tempcrit;
  int maxcrit = 0;
  int maxloc = 0;
  int critfound = 0;
  int critnum = 0;
  unsigned char maxtype = 0;

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (SectIsDestroyed(mech, loop))
      continue;
    count = FindWeapons(mech, loop, weaparray, weapdata, critical);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        if (!PartIsBroken(mech, loop, critical[ii])) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_random_i31(&mech->xcode.context->random);
          if (tempcrit > maxcrit) {
            critfound = 1;
            maxcrit = tempcrit;
            maxloc = loop;
            maxtype = weaparray[ii];
            critnum = critical[ii];
          }
        }
      }
    }
  }

  if (critfound) {
    SetPartTempNuke(mech, maxloc, critnum, FAIL_DESTROYED);
    mech_printf(mech, MECHALL, "[fg=red bold]Your %s is jammed![reset]",
                &MechWeapons[maxtype].name[3]);
  }
}

void pickRandomWeapon(Mech *objMech, int wLoc, int *critNum, int wIgnoreJams) {
  int awCrits[MAX_WEAPS_SECTION];
  int wcWeaps = 0;
  int wIter;

  /*
   * Find our weapons
   */

  for (wIter = 0; wIter < MAX_WEAPS_SECTION; wIter++) {
    if (IsWeapon(GetPartType(objMech, wLoc, wIter))) {
      if (!PartIsBroken(objMech, wLoc, wIter)) {
        if (!wIgnoreJams ||
            (wIgnoreJams && !PartTempNuke(objMech, wLoc, wIter))) {
          awCrits[wcWeaps] = wIter;

          wcWeaps++;
        }
      }
    }
  }

  if (wcWeaps <= 0) {
    *critNum = -1;
    return;
  }

  /*
   * Now randomly pick one
   */

  *critNum =
      awCrits[btech_random_range(objMech->xcode.context, 0, wcWeaps - 1)];
}

/*
 * Make sure we're not set to go over our walking/cruise speed
 */
void limitSpeedToCruise(Mech *objMech) {
  int wMaxSpeed = 0;

  wMaxSpeed = MMaxSpeed(objMech);

  if (MechMove(objMech) == MOVE_VTOL)
    wMaxSpeed = sqrt((float)wMaxSpeed * wMaxSpeed -
                     MechVerticalSpeed(objMech) * MechVerticalSpeed(objMech));

  if (WalkingSpeed(wMaxSpeed) < MechDesiredSpeed(objMech))
    MechDesiredSpeed(objMech) = WalkingSpeed(wMaxSpeed) - 0.1;
}

void DoVehicleStablizerCrit(Mech *objMech, int wLoc) {
  /*
   * Double attacker movement for all weapons fired from
   * this location. If no weapons in this location, crit has no
   * effect. Only first stablizer hit matters, subsequent ones
   * should be ignored.
   */

  char strLocName[30];

  ArmorStringFromIndex(wLoc, strLocName, MechType(objMech), MechMove(objMech));

  if (MechSections(objMech)[wLoc].config & STABILIZERS_DESTROYED)
    mech_printf(objMech, MECHALL,
                "The destroyed weapon stabilizers in your %s take another hit!",
                strLocName);
  else {
    mech_printf(objMech, MECHALL,
                "The weapon stabilizers in your %s have been destroyed!",
                strLocName);
    MechSections(objMech)[wLoc].config |= STABILIZERS_DESTROYED;
  }
}

void DoTurretLockCrit(Mech *objMech) {
  /*
   * Turret locks in the current direction.
   */

  if (MechTankCritStatus(objMech) & TURRET_LOCKED) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (MechTankCritStatus(objMech) & TURRET_JAMMED)
    MechTankCritStatus(objMech) &= ~TURRET_JAMMED;

  MechTankCritStatus(objMech) |= TURRET_LOCKED;
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot destroys your turret rotation mechanism![reset]");
}

void DoTurretJamCrit(Mech *objMech) {
  /*
   * Turret rotation temporarily jams. Vehicle crew must spend
   * attack phase unjamming (read for mux: no weapons fire/ramming/etc...
   * while unjamming turret. Second jam crit == turret locked.
   */

  if (MechTankCritStatus(objMech) & TURRET_LOCKED) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (MechTankCritStatus(objMech) & TURRET_JAMMED) {
    DoTurretLockCrit(objMech);
    return;
  }

  MechTankCritStatus(objMech) |= TURRET_JAMMED;
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your turret gets jammed on its current facing![reset]");
}

void DoWeaponJamCrit(Mech *objMech, int wLoc) {
  /*
   * A weapon in this location is stuck. The vehicle crew must spend
   * the attack phase unjamming this weapon.
   *
   * Can this really apply to a non-ammo weapon? Maybe we should just do a
   * 'shorted/jammed' failure on the weapon?
   *
   * ALTERATION: Currently it's coded to 'auto-unjam' after 60 to 120 seconds.
   */

  int wWeapIdx = 0;
  int wCritType = 0;
  int wCritNum = 0;

  if (SectIsDestroyed(objMech, wLoc))
    return;

  pickRandomWeapon(objMech, wLoc, &wCritNum, 1);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = GetPartType(objMech, wLoc, wCritNum);
  wWeapIdx = Weapon2I(wCritType);

  if (wWeapIdx >= 0) {
    switch (MechWeapons[wWeapIdx].type) {
    case TBEAM:
    case TMISSILE:
    case TARTILLERY:
      wCritType = FAIL_SHORTED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  &MechWeapons[wWeapIdx].name[3]);
      break;
    case TAMMO:
      wCritType = FAIL_JAMMED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot temporarily jams your %s![reset]",
                  &MechWeapons[wWeapIdx].name[3]);
      break;
    default:
      wCritType = FAIL_SHORTED;
      mech_printf(objMech, MECHALL,
                  "[fg=red bold]The shot causes your %s to temporarily short "
                  "out![reset]",
                  &MechWeapons[wWeapIdx].name[3]);
      break;
    }

    SetPartTempNuke(objMech, wLoc, wCritNum, wCritType);
    mech_set_recycle_part(objMech, wLoc, wCritNum,
                          btech_random_range(objMech->xcode.context, 60, 120));
  }
}

void DoWeaponDestroyedCrit(Mech *objAttacker, Mech *objMech, int wLoc,
                           int LOS) {
  /*
   * A weapon in this location is destroyed.
   */
  int wWeapIdx = 0;
  int wCritNum = 0;
  int wCritType = 0;
  int firstCrit = 0;

  if (SectIsDestroyed(objMech, wLoc))
    return;

  pickRandomWeapon(objMech, wLoc, &wCritNum, 0);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = GetPartType(objMech, wLoc, wCritNum);
  wWeapIdx = Weapon2I(wCritType);

  if (handleWeaponCrit(objAttacker, objMech, wLoc, wCritNum, wCritType, LOS)) {
    return;
  }

  if (wWeapIdx >= 0) {
    firstCrit = FindFirstWeaponCrit(objMech, wLoc, -1, 0, wCritType, 1);

    DestroyWeapon(objMech, wLoc, wCritType, firstCrit, 1, 1);
    mech_printf(objMech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
                &MechWeapons[wWeapIdx].name[3]);
  }
}

void DoTurretBlownOffCrit(Mech *objMech, Mech *objAttacker, int LOS) {
  /*
   * The turret is blown off, destroying everything in there
   */

  if (SectIsDestroyed(objMech, TURRET))
    return;

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot pops your turret clear off its housing![reset]");
  MechLOSBroadcast(objMech, "'s turret flies off!");
  DestroySection(objMech, objAttacker, LOS, TURRET);
}

void DoAmmunitionCrit(Mech *objMech, Mech *objAttacker, int wLoc, int LOS) {
  /*
   * Count total ammo carried on the tank. Apply damage directly to
   * the internal structure of the vehicle.
   *
   * If the vehicle has CASE, apply the damage to the rear ARMOR and also
   * cause a Driver Hit, Commander Hit and Crew Stunned crit.
   *
   * if the vehicle has no ammunition, treat this as a weapon destroyed crit.
   */

  int wTotalAmmoDamage = 0;
  int wTempDamage = 0;
  int wSecIter, wSlotIter;
  int wPartType = 0;
  int wWeapIdx;

  for (wSecIter = 0; wSecIter <= 7; wSecIter++) {
    if (SectIsDestroyed(objMech, wSecIter))
      continue;

    for (wSlotIter = CritsInLoc(objMech, wSecIter) - 1; wSlotIter >= 0;
         wSlotIter--) {
      wPartType = GetPartType(objMech, wSecIter, wSlotIter);
      wWeapIdx = Ammo2WeaponI(wPartType);

      if (IsAmmo(wPartType) && GetPartData(objMech, wSecIter, wSlotIter) &&
          (!(MechWeapons[wWeapIdx].special & GAUSS))) {
        wTempDamage = (FindMaxAmmoDamage(objMech->xcode.context,
                                         Ammo2WeaponI(wPartType)) *
                       GetPartData(objMech, wSecIter, wSlotIter));
        wTotalAmmoDamage += wTempDamage;

        SetPartData(objMech, wSecIter, wSlotIter, 0);
      }
    }
  }

  if (wTotalAmmoDamage == 0) {
    DoWeaponDestroyedCrit(objAttacker, objMech, wLoc, LOS);
    return;
  }

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]One of your ammo bins is struck causing a cascading "
      "explosion![reset]");
  MechLOSBroadcast(objMech, "has an internal ammo explosion!");

  DamageMech(objMech, objAttacker, 0, -1, wLoc, 0, 0, 0, wTotalAmmoDamage, 0, 0,
             -1, 0, 1);
}
