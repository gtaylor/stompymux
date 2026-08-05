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
#include "mech_ammodump_api.h"
#include "mech_c3_api.h"
#include "mech_c3i_api.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_pickup_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
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
#include "section_types.h"
#include "weapon_catalogue_api.h"

int mech_weapon_critical_handle(Mech *attacker, Mech *wounded, int hitloc,
                                int critHit, int critType, int LOS) {
  int wMaxCrits, wFirstCrit, wWeapDestroyed = 0;
  int wAmmoSection, wAmmoCritSlot;
  int damage;
  char locname[30];
  char msgbuf[MBUF_SIZE] = {0};
  BtechContext *context = mech_context(wounded);

  ArmorStringFromIndex(hitloc, locname, mech_class(wounded),
                       mech_movement_type(wounded));

  /* Get the max number of crits for this weapon */
  wMaxCrits = GetWeaponCrits(wounded, Weapon2I(critType));

  /* Find the first crit */
  wFirstCrit =
      FindFirstWeaponCrit(wounded, hitloc, critHit, 0, critType, wMaxCrits);

  /* See if the weapon is already destroyed */
  if (wFirstCrit != -1) {
    wWeapDestroyed =
        (mech_critical_is_nonfunctional(wounded, hitloc, wFirstCrit) ||
         (mech_critical_temporary_failure(wounded, hitloc, wFirstCrit) ==
          FAIL_DESTROYED));
  }

  /* Gauss rifle-ish weapons explode when critted */
  if ((MechWeapons[Weapon2I(critType)].special & GAUSS) && !wWeapDestroyed) {
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                &MechWeapons[Weapon2I(critType)].name[3]);

    mech_printf(wounded, MECHALL, "It explodes for %d points damage.",
                MechWeapons[Weapon2I(critType)].explosiondamage);

    if (!mech_is_destroyed(wounded)) {
      snprintf(msgbuf, MBUF_SIZE,
               "'s %s is covered in a large electrical discharge!", locname);
      mech_los_broadcast(wounded, msgbuf);
    }

    mech_weapon_destroy(wounded, hitloc, critType, wFirstCrit, wMaxCrits,
                        wMaxCrits);

    if (attacker) {
      DamageMech(wounded, attacker, 0, -1, hitloc, 0, 0, 0,
                 MechWeapons[Weapon2I(critType)].explosiondamage, -1, 7, -1, 0,
                 1);
    }
    /* Rule Reference: BMR Revised, Page 16-17 (Ammo Explosion=2 Bruise) */
    /* Rule Reference: Total Warfare, Page 41 (Ammo Explosion=2 Bruise) */

    if (mech_class(wounded) != CLASS_BSUIT) {
      mech_notify(wounded, MECHPILOT,
                  "You take personal injury from the weapon's explosion!");

      /* Rule Reference: MaxTech Revised, Page 46 (Reduce by 1 because of pain
       * resistance) */

      if (HasBoolAdvantage(context, mech_pilot_dbref(wounded),
                           "pain_resistance"))
        headhitmwdamage(wounded, wounded, 1);
      else
        headhitmwdamage(wounded, wounded, 2);
    }
    return 1;
  } else if (weapon_catalogue_is_anti_missile(
                 Weapon2I(critType))) { /* Have to shut down AMS when its
                                           critted */
    mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                &MechWeapons[Weapon2I(critType)].name[3]);

    mech_ams_enabled_set(wounded, false);
    mech_technology_flags_remove(wounded,
                                 IS_ANTI_MISSILE_TECH | CL_ANTI_MISSILE_TECH);
  } else if (weapon_catalogue_is_hot_loaded(
                 Weapon2I(critType),
                 mech_critical_fire_mode(wounded, hitloc, wFirstCrit)) &&
             !wWeapDestroyed) { /* And crit hotloaded LRMs */
    if (FindAmmoForWeapon(wounded, Weapon2I(critType), 0, &wAmmoSection,
                          &wAmmoCritSlot) > 0) {
      damage = MechWeapons[Weapon2I(critType)].damage;

      if (IsMissile(Weapon2I(critType)) || IsArtillery(Weapon2I(critType))) {
        int missile_count =
            btech_context_missile_hit_count(context, Weapon2I(critType), 10);
        if (missile_count > 0) {
          damage *= missile_count;
        }
      }

      mech_printf(wounded, MECHALL, "Your %s has been destroyed!",
                  &MechWeapons[Weapon2I(critType)].name[3]);

      mech_printf(
          wounded, MECHALL,
          "[fg=red bold]Your hotloaded launcher explodes for %d points of "
          "damage![reset]",
          damage);

      if (!mech_is_destroyed(wounded)) {
        snprintf(msgbuf, MBUF_SIZE,
                 " loses a launcher in a brilliant explosion!");
        mech_los_broadcast(wounded, msgbuf);
      }

      mech_weapon_destroy(wounded, hitloc, critType, wFirstCrit, wMaxCrits,
                          wMaxCrits);

      if (attacker) {
        DamageMech(wounded, attacker, 0, -1, hitloc, 0, 0, 0, damage, -1, 7, -1,
                   0, 1);
      }

      return 1;
    }
  } else if ((mech_critical_ammo_mode(wounded, hitloc, wFirstCrit) &
              AC_INCENDIARY_MODE) &&
             !wWeapDestroyed &&
             mech_weapon_is_recycling_at(
                 wounded, hitloc,
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

      if (!mech_is_destroyed(wounded)) {
        snprintf(msgbuf, MBUF_SIZE,
                 "'s %s is engulfed in a brilliant blue flame!", locname);
        mech_los_broadcast(wounded, msgbuf);
      }

      mech_weapon_destroy(wounded, hitloc, critType, wFirstCrit, wMaxCrits,
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

void mech_main_weapon_jam(Mech *mech) {
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
  BtechContext *context = mech_context(mech);

  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    if (mech_section_is_destroyed(mech, loop))
      continue;
    count = FindWeapons_Advanced(mech, loop, weaparray, weapdata, critical, 1);
    if (count > 0) {
      for (ii = 0; ii < count; ii++) {
        if (!mech_critical_is_broken(mech, loop, critical[ii])) {
          /* tempcrit = GetWeaponCrits(mech, weaparray[ii]); */
          tempcrit = (int)btech_context_random_i31(context);
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
    mech_critical_temporary_failure_set(mech, maxloc, critnum, FAIL_DESTROYED);
    mech_printf(mech, MECHALL, "[fg=red bold]Your %s is jammed![reset]",
                &MechWeapons[maxtype].name[3]);
  }
}

void mech_random_weapon_select(Mech *objMech, int wLoc, int *critNum,
                               int wIgnoreJams) {
  int awCrits[MAX_WEAPS_SECTION];
  int wcWeaps = 0;
  int wIter;

  /*
   * Find our weapons
   */

  for (wIter = 0; wIter < MAX_WEAPS_SECTION; wIter++) {
    if (IsWeapon(mech_critical_part_type(objMech, wLoc, wIter))) {
      if (!mech_critical_is_broken(objMech, wLoc, wIter)) {
        if (!wIgnoreJams || (wIgnoreJams && !mech_critical_temporary_failure(
                                                objMech, wLoc, wIter))) {
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

  *critNum = awCrits[btech_random_range(mech_context(objMech), 0, wcWeaps - 1)];
}

/*
 * Make sure we're not set to go over our walking/cruise speed
 */
void mech_speed_limit_to_cruise(Mech *objMech) {
  int wMaxSpeed = 0;

  wMaxSpeed = mech_cargo_maximum_speed(objMech, mech_maximum_speed(objMech));

  if (mech_movement_type(objMech) == MOVE_VTOL)
    wMaxSpeed =
        sqrt((float)wMaxSpeed * wMaxSpeed -
             mech_vertical_speed(objMech) * mech_vertical_speed(objMech));

  float walking_speed = 2.0F * wMaxSpeed / 3.0F;
  if (walking_speed < mech_desired_speed(objMech))
    mech_desired_speed_set(objMech, walking_speed - 0.1F);
}

void mech_vehicle_stabilizer_critical_apply(Mech *objMech, int wLoc) {
  /*
   * Double attacker movement for all weapons fired from
   * this location. If no weapons in this location, crit has no
   * effect. Only first stablizer hit matters, subsequent ones
   * should be ignored.
   */

  char strLocName[30];

  ArmorStringFromIndex(wLoc, strLocName, mech_class(objMech),
                       mech_movement_type(objMech));

  if (mech_section_configuration_has(objMech, wLoc, STABILIZERS_DESTROYED))
    mech_printf(objMech, MECHALL,
                "The destroyed weapon stabilizers in your %s take another hit!",
                strLocName);
  else {
    mech_printf(objMech, MECHALL,
                "The weapon stabilizers in your %s have been destroyed!",
                strLocName);
    mech_section_configuration_add(objMech, wLoc, STABILIZERS_DESTROYED);
  }
}

void mech_turret_lock_critical_apply(Mech *objMech) {
  /*
   * Turret locks in the current direction.
   */

  MechConditionSummary condition = mech_condition_summary(objMech);
  if (condition.turret_locked) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed)
    mech_turret_jammed_set(objMech, false);

  mech_turret_locked_set(objMech, true);
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot destroys your turret rotation mechanism![reset]");
}

void mech_turret_jam_critical_apply(Mech *objMech) {
  /*
   * Turret rotation temporarily jams. Vehicle crew must spend
   * attack phase unjamming (read for mux: no weapons fire/ramming/etc...
   * while unjamming turret. Second jam crit == turret locked.
   */

  MechConditionSummary condition = mech_condition_summary(objMech);
  if (condition.turret_locked) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  if (condition.turret_jammed) {
    mech_turret_lock_critical_apply(objMech);
    return;
  }

  mech_turret_jammed_set(objMech, true);
  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]Your turret gets jammed on its current facing![reset]");
}

void mech_weapon_jam_critical_apply(Mech *objMech, int wLoc) {
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

  if (mech_section_is_destroyed(objMech, wLoc))
    return;

  mech_random_weapon_select(objMech, wLoc, &wCritNum, 1);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = mech_critical_part_type(objMech, wLoc, wCritNum);
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

    mech_critical_temporary_failure_set(objMech, wLoc, wCritNum, wCritType);
    mech_set_recycle_part(objMech, wLoc, wCritNum,
                          btech_random_range(mech_context(objMech), 60, 120));
  }
}

void mech_weapon_destroyed_critical_apply(Mech *objAttacker, Mech *objMech,
                                          int wLoc, int LOS) {
  /*
   * A weapon in this location is destroyed.
   */
  int wWeapIdx = 0;
  int wCritNum = 0;
  int wCritType = 0;
  int firstCrit = 0;

  if (mech_section_is_destroyed(objMech, wLoc))
    return;

  mech_random_weapon_select(objMech, wLoc, &wCritNum, 0);

  if (wCritNum < 0) {
    mech_notify(
        objMech, MECHALL,
        "The shot pierces your armor yet fails to hit a critical system!");
    return;
  }

  wCritType = mech_critical_part_type(objMech, wLoc, wCritNum);
  wWeapIdx = Weapon2I(wCritType);

  if (mech_weapon_critical_handle(objAttacker, objMech, wLoc, wCritNum,
                                  wCritType, LOS)) {
    return;
  }

  if (wWeapIdx >= 0) {
    firstCrit = FindFirstWeaponCrit(objMech, wLoc, -1, 0, wCritType, 1);

    mech_weapon_destroy(objMech, wLoc, wCritType, firstCrit, 1, 1);
    mech_printf(objMech, MECHALL, "[fg=red bold]Your %s is destroyed![reset]",
                &MechWeapons[wWeapIdx].name[3]);
  }
}

void mech_turret_blown_off_critical_apply(Mech *objMech, Mech *objAttacker,
                                          int LOS) {
  /*
   * The turret is blown off, destroying everything in there
   */

  if (mech_section_is_destroyed(objMech, TURRET))
    return;

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]The shot pops your turret clear off its housing![reset]");
  mech_los_broadcast(objMech, "'s turret flies off!");
  mech_section_destroy(objMech, objAttacker, LOS, TURRET);
}

void mech_ammunition_critical_apply(Mech *objMech, Mech *objAttacker, int wLoc,
                                    int LOS) {
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
  BtechContext *context = mech_context(objMech);

  for (wSecIter = 0; wSecIter <= 7; wSecIter++) {
    if (mech_section_is_destroyed(objMech, wSecIter))
      continue;

    for (wSlotIter = mech_section_critical_count(objMech, wSecIter) - 1;
         wSlotIter >= 0; wSlotIter--) {
      wPartType = mech_critical_part_type(objMech, wSecIter, wSlotIter);
      wWeapIdx = Ammo2WeaponI(wPartType);

      if (IsAmmo(wPartType) &&
          mech_critical_data(objMech, wSecIter, wSlotIter) &&
          (!(MechWeapons[wWeapIdx].special & GAUSS))) {
        wTempDamage =
            weapon_maximum_ammunition_damage(context, Ammo2WeaponI(wPartType)) *
            mech_critical_data(objMech, wSecIter, wSlotIter);
        wTotalAmmoDamage += wTempDamage;

        mech_critical_data_set(objMech, wSecIter, wSlotIter, 0);
      }
    }
  }

  if (wTotalAmmoDamage == 0) {
    mech_weapon_destroyed_critical_apply(objAttacker, objMech, wLoc, LOS);
    return;
  }

  mech_notify(
      objMech, MECHALL,
      "[fg=red bold]One of your ammo bins is struck causing a cascading "
      "explosion![reset]");
  mech_los_broadcast(objMech, "has an internal ammo explosion!");

  DamageMech(objMech, objAttacker, 0, -1, wLoc, 0, 0, 0, wTotalAmmoDamage, 0, 0,
             -1, 0, 1);
}
