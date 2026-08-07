/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2000 Thomas Wouters
 */

#include <stdio.h>
#include <string.h>

#include "btech/context.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "failures.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_damage_api.h"
#include "mech_enhanced_criticals_api.h"
#include "mech_equipment_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_notify_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static void mech_weapon_damage_info_show(DbRef player, Mech *mech, int section,
                                         int critical);

static void mech_weapon_critical_data(Mech *mech, int section, int critical,
                                      int *weapon_index, int *weapon_size,
                                      int *first_critical) {
  int wCritType = 0;

  /* Get the crit type */
  wCritType = mech_critical_part_type(mech, section, critical);

  /* Get the weapon index */
  *weapon_index = weapon_from_equipment_index(wCritType);

  /* Get the max number of crits for this weapon */
  *weapon_size = GetWeaponCrits(mech, *weapon_index);

  /* Find the first crit */
  *first_critical =
      FindFirstWeaponCrit(mech, section, critical, 0, wCritType, *weapon_size);
}

int mech_weapon_critical_to_hit_modifier(Mech *mech, int section, int critical,
                                         int rangeBracket) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int wRetMod = 0;
  int count = 0;
  int nloc, ncrit, stype;

  if (mech_class(mech) != CLASS_MECH)
    return 0;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_MODERATE)
      wRetMod++;

    if ((mech_critical_damage_flags(mech, section, i) &
         (WEAP_DAM_EN_FOCUS | WEAP_DAM_MSL_RANGING)) &&
        rangeBracket != RANGE_SHORT)
      wRetMod++;
    count++;
  }

  if (count < wWeapSize) { // got split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_MODERATE)
          wRetMod++;
        if ((mech_critical_damage_flags(mech, nloc, i) &
             (WEAP_DAM_EN_FOCUS | WEAP_DAM_MSL_RANGING)) &&
            rangeBracket != RANGE_SHORT)
          wRetMod++;
      }
    }
  }

  return wRetMod;
}

int mech_weapon_critical_heat_modifier(Mech *mech, int section, int critical) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int wRetMod = 0;
  int count = 0, nloc, ncrit, stype;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  if (!weapon_catalogue_is_energy(wWeapIndex))
    return 0;

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_EN_CRYSTAL)
      wRetMod++;
    count++;
  }

  if (count < wWeapSize && mech_class(mech) == CLASS_MECH) { // split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_EN_CRYSTAL)
          wRetMod++;
      }
    }
  }

  return wRetMod;
}

int mech_weapon_critical_damage_penalty(Mech *mech, int section, int critical) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int wRetMod = 0;
  int count = 0, nloc, ncrit, stype;

  if (mech_class(mech) != CLASS_MECH)
    return 0;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  if (!weapon_catalogue_is_energy(wWeapIndex))
    return 0;

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_EN_FOCUS)
      wRetMod++;
    count++;
  }

  if (count < wWeapSize) { // got split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_EN_FOCUS)
          wRetMod++;
      }
    }
  }

  return wRetMod;
}

bool mech_weapon_critical_can_explode(Mech *mech, int section, int critical,
                                      int roll) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int wExplosionCheck = 0;
  int count = 0, nloc, ncrit, stype;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) &
        (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
      wExplosionCheck++;
    count++;
  }

  if (count < wWeapSize) { // got split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) &
            (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
          wExplosionCheck++;
      }
    }
  }

  if (wExplosionCheck > 0)
    wExplosionCheck += 1;

  return wExplosionCheck >= roll;
}

bool mech_weapon_critical_can_jam(Mech *mech, int section, int critical,
                                  int roll) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int wJamCheck = 0;
  int count = 0, nloc, ncrit, stype;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) & WEAP_DAM_BALL_BARREL)
      wJamCheck++;
    count++;
  }

  if (count < wWeapSize) { // got split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) & WEAP_DAM_BALL_BARREL)
          wJamCheck++;
      }
    }
  }

  if (wJamCheck > 0)
    wJamCheck += 1;

  return wJamCheck >= roll;
}

bool mech_weapon_ammo_feed_is_locked(Mech *mech, int section, int critical) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int i;
  int count = 0, nloc, ncrit, stype;

  if (mech_class(mech) != CLASS_MECH)
    return false;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  /* Iterate over the crits and see if we have any enhanced damage */
  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_damage_flags(mech, section, i) &
        (WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
      return true;
    count++;
  }

  if (count < wWeapSize) { // got split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_damage_flags(mech, nloc, i) &
            (WEAP_DAM_BALL_AMMO | WEAP_DAM_MSL_AMMO))
          return true;
      }
    }
  }

  return false;
}

int mech_weapon_damaged_slot_count_at(Mech *mech, int section, int critical) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  return mech_weapon_damaged_slot_count(mech, section, wFirstCrit, wWeapSize);
}

int mech_weapon_damaged_slot_count(Mech *mech, int section, int wFirstCrit,
                                   int wWeapSize) {
  int wCritsDamaged = 0;
  int i;
  int count = 0, nloc, ncrit, stype;

  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_is_damaged(mech, section, i))
      wCritsDamaged++;
    count++;
  }

  if (count < wWeapSize && mech_class(mech) == CLASS_MECH) { // split crits
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_is_damaged(mech, nloc, i))
          wCritsDamaged++;
      }
    }
  }

  return wCritsDamaged;
}

bool mech_weapon_critical_should_destroy(Mech *mech, int section, int critical,
                                         bool incrementCount) {
  int wCritsDamaged = 0;
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;

  if (mech_class(mech) != CLASS_MECH)
    return true;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  if (incrementCount)
    wCritsDamaged++;

  wCritsDamaged +=
      mech_weapon_damaged_slot_count(mech, section, wFirstCrit, wWeapSize);

  if ((wCritsDamaged * 2) > wWeapSize)
    return true;

  return false;
}

void mech_weapon_critical_apply(Mech *mech, Mech *attacker, int LOS,
                                int section, int critical) {
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int wCritRoll = btech_random_roll(mech_context(mech));
  int tDestroyWeapon = 0;
  int tNoCrit = 0;
  int tModerateCrit = 0;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  /* See if we should just destroy the sucker outright */
  if (mech_weapon_critical_should_destroy(mech, section, critical, true))
    tDestroyWeapon = 1;
  else {
    /* Add the total number of damaged slots */
    wCritRoll +=
        mech_weapon_damaged_slot_count(mech, section, wFirstCrit, wWeapSize);
    wCritRoll++;
  }

  if (!tDestroyWeapon) {
    /* See what damage we do */
    if (weapon_catalogue_is_energy(wWeapIndex)) {
      if (wCritRoll <= 3) {
        tNoCrit = 1;
      } else if (wCritRoll <= 5) {
        tModerateCrit = 1;
      } else if (wCritRoll <= 7) {
        mech_printf(
            mech, MECHALL,
            "Your %s's focusing mechanism gets knocked out of alignment!!",
            &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_EN_FOCUS);
      } else if (wCritRoll <= 9) {
        mech_printf(mech, MECHALL,
                    "Your %s's charging crystal takes a direct hit!!",
                    &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_EN_CRYSTAL);
      } else {
        tDestroyWeapon = 1;
      }
    } else if (weapon_catalogue_is_missile(wWeapIndex)) {
      if (wCritRoll <= 3) {
        tNoCrit = 1;
      } else if (wCritRoll <= 5) {
        tModerateCrit = 1;
      } else if (wCritRoll <= 7) {
        mech_printf(mech, MECHALL, "Your %s's ranging system takes a hit!!",
                    &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_MSL_RANGING);
      } else if (wCritRoll <= 9) {
        mech_printf(mech, MECHALL, "Your %s's ammo feed is damaged!!",
                    &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_MSL_AMMO);
      } else {
        tDestroyWeapon = 1;
      }
    } else if (weapon_catalogue_is_ballistic(wWeapIndex) ||
               weapon_catalogue_is_artillery(wWeapIndex)) {
      if (wCritRoll <= 3) {
        tNoCrit = 1;
      } else if (wCritRoll <= 5) {
        tModerateCrit = 1;
      } else if (wCritRoll <= 7) {
        mech_printf(mech, MECHALL, "Your %s's barrel warps from the damage!!",
                    &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_BALL_BARREL);
      } else if (wCritRoll <= 9) {
        mech_printf(mech, MECHALL, "Your %s's ammo feed is damaged!!",
                    &MechWeapons[wWeapIndex].name[3]);
        mech_critical_damage_flags_add(mech, section, critical,
                                       WEAP_DAM_BALL_AMMO);
      } else {
        tDestroyWeapon = 1;
      }
    } else {
      tDestroyWeapon = 1;
    }
  }

  if (tDestroyWeapon) {
    mech_printf(mech, MECHALL, "Your %s has been destroyed!!",
                &MechWeapons[wWeapIndex].name[3]);
    mech_weapon_destroy(mech, section,
                        mech_critical_part_type(mech, section, critical),
                        wFirstCrit, 1, wWeapSize);
  } else {
    mech_critical_fire_mode_add(mech, section, critical, DAMAGED_MODE);

    if (tNoCrit)
      mech_printf(mech, MECHALL,
                  "Your %s takes a hit but suffers no noticeable damage!!",
                  &MechWeapons[wWeapIndex].name[3]);
    else if (tModerateCrit) {
      mech_printf(mech, MECHALL, "Your %s takes a hit but continues working!!",
                  &MechWeapons[wWeapIndex].name[3]);
      mech_critical_damage_flags_add(mech, section, critical,
                                     WEAP_DAM_MODERATE);
    }
  }
}

void mech_weapon_status(DbRef player, Mech *mech, char *buffer) {
  int secIter = 0;
  int weapIter = 0;
  int wWeapsInSec = 0;
  int wcWeaps = 0;
  int wDamagedSlots = 0;
  unsigned char weaparray[MAX_WEAPS_SECTION] = {0};
  unsigned char weapdata[MAX_WEAPS_SECTION] = {0};
  int critical[MAX_WEAPS_SECTION] = {0};
  char tempbuff[LBUF_SIZE] = {0};
  char strLocation[80] = {0};
  char weapbuff[LBUF_SIZE] = {0};

  if (!common_checks(player, mech, MECH_USUALSP))
    return;

  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "=========================WEAPON SYSTEMS "
               "STATUS=========================");
  mecha_notify(btech_context_evaluation(mech_context(mech)), player,
               "[##] -------- Weapon Name -------- || Location -------- || "
               "Status -----");

  for (secIter = 0; secIter < NUM_SECTIONS; secIter++) {
    wWeapsInSec =
        FindWeapons_Advanced(mech, secIter, weaparray, weapdata, critical, 1);

    if (wWeapsInSec <= 0)
      continue;

    ArmorStringFromIndex(secIter, tempbuff, mech_class(mech),
                         mech_movement_type(mech));
    snprintf(strLocation, sizeof(strLocation), "%-18.18s", tempbuff);

    for (weapIter = 0; weapIter < wWeapsInSec; weapIter++) {
      snprintf(weapbuff, sizeof(weapbuff), "[%2d] %-29.29s || ", wcWeaps++,
               &MechWeapons[weaparray[weapIter]].name[3]);

      strcat(weapbuff, strLocation);
      wDamagedSlots = 0;

      if (mech_critical_is_broken(mech, secIter, critical[weapIter]) ||
          mech_critical_temporary_failure(mech, secIter, critical[weapIter]) ==
              FAIL_DESTROYED)
        strcat(weapbuff, "|| [fg=red bold]DESTROYED[reset]");
      else {

        if (mech_class(mech) == CLASS_MECH)
          wDamagedSlots = mech_weapon_damaged_slot_count_at(mech, secIter,
                                                            critical[weapIter]);

        if (mech_critical_is_disabled(mech, secIter, critical[weapIter]))
          strcat(weapbuff, "|| [fg=red bold]DISABLED[reset]");
        else if (mech_critical_temporary_failure(mech, secIter,
                                                 critical[weapIter])) {
          switch (mech_critical_temporary_failure(mech, secIter,
                                                  critical[weapIter])) {
          case FAIL_JAMMED:
            strcat(weapbuff, "|| [fg=yellow]JAMMED[reset]");
            break;
          case FAIL_SHORTED:
            strcat(weapbuff, "|| [fg=blue]SHORTED[reset]");
            break;
          case FAIL_EMPTY:
            strcat(weapbuff, "|| [fg=cyan]EMPTY[reset]");
            break;
          case FAIL_DUD:
            strcat(weapbuff, "|| [fg=yellow]DUD[reset]");
            break;
          case FAIL_AMMOJAMMED:
            strcat(weapbuff, "|| [fg=yellow]AMMOJAM[reset]");
            break;
          }
        } else if (wDamagedSlots > 0)
          strcat(weapbuff, "|| [fg=yellow bold]DAMAGED[reset]");
        else
          strcat(weapbuff, "|| [fg=green bold]OPERATIONAL[reset]");
      }

      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   weapbuff);

      mech_weapon_damage_info_show(player, mech, secIter, critical[weapIter]);
    }
  }
}

static void mech_weapon_damage_info_show(DbRef player, Mech *mech, int section,
                                         int critical) {
  EvaluationContext *evaluation = btech_context_evaluation(mech_context(mech));
  int wWeapSize = 0;
  int wFirstCrit = 0;
  int wWeapIndex = 0;
  int awDamage[3];
  int i;
  int tHasDamagedPart = 0;
  int tPrintSpace = 0;
  int wAmmoLoc = mech_critical_desired_ammo_section(mech, section, critical);
  char strLocation[80];
  int awNonOpCrits[3];
  int damflag;
  int count = 0, nloc, ncrit, stype;

  mech_weapon_critical_data(mech, section, critical, &wWeapIndex, &wWeapSize,
                            &wFirstCrit);

  for (i = 0; i < 3; i++) {
    awDamage[i] = 0;
    awNonOpCrits[i] = 0;
  }

  for (i = wFirstCrit; i < MIN(NUM_CRITICALS, wFirstCrit + wWeapSize); i++) {
    if (mech_critical_is_damaged(mech, section, i)) {
      tHasDamagedPart = 1;
      damflag = mech_critical_damage_flags(mech, section, i);
      awDamage[0] += damflag & WEAP_DAM_MODERATE;
      awDamage[1] += (damflag & (WEAP_DAM_EN_FOCUS | WEAP_DAM_BALL_BARREL |
                                 WEAP_DAM_MSL_RANGING));
      awDamage[2] += (damflag & (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO |
                                 WEAP_DAM_MSL_AMMO));
      awNonOpCrits[0]++;
    } else if (mech_critical_is_destroyed(mech, section, i)) {
      awNonOpCrits[1]++;
    } else if (mech_critical_is_disabled(mech, section, i)) {
      awNonOpCrits[2]++;
    }
  }

  if (count < wWeapSize && mech_class(mech) == CLASS_MECH) {
    if (GetSplitData(mech, section, wFirstCrit, &nloc, &ncrit, &stype)) {
      for (i = ncrit; i < (wWeapSize - count); i++) {
        if (mech_critical_is_damaged(mech, nloc, i)) {
          tHasDamagedPart = 1;
          damflag = mech_critical_damage_flags(mech, nloc, i);
          awDamage[0] += damflag & WEAP_DAM_MODERATE;
          awDamage[1] += (damflag & (WEAP_DAM_EN_FOCUS | WEAP_DAM_BALL_BARREL |
                                     WEAP_DAM_MSL_RANGING));
          awDamage[2] += (damflag & (WEAP_DAM_EN_CRYSTAL | WEAP_DAM_BALL_AMMO |
                                     WEAP_DAM_MSL_AMMO));
          awNonOpCrits[0]++;
        } else if (mech_critical_is_destroyed(mech, nloc, i)) {
          awNonOpCrits[1]++;
        } else if (mech_critical_is_disabled(mech, nloc, i)) {
          awNonOpCrits[2]++;
        }
      }
    }
  }

  if (tHasDamagedPart) {
    tPrintSpace = 1;

    if (awDamage[0] > 0) {
      notify_printf(evaluation, player,
                    "      General damage (%d hit%s): +%d to hit.", awDamage[0],
                    awDamage[0] > 1 ? "s" : "", awDamage[0]);
    }

    if (weapon_catalogue_is_energy(wWeapIndex)) {
      if (awDamage[1] > 0) {
        notify_printf(evaluation, player,
                      "      Focus misalignment (%d hit%s): -%d damage. +%d to "
                      "hit at >%d hexes.",
                      awDamage[1], awDamage[1] > 1 ? "s" : "", awDamage[1],
                      awDamage[1], MechWeapons[wWeapIndex].shortrange);
      }

      if (awDamage[2] > 0) {
        notify_printf(evaluation, player,
                      "      Charging crystal damage (%d hit%s): +%d heat. "
                      "Explodes on %d or less.[reset]",
                      awDamage[2], awDamage[2] > 1 ? "s" : "", awDamage[2],
                      awDamage[2] + 1);
      }
    } else if (weapon_catalogue_is_missile(wWeapIndex)) {
      if (awDamage[1] > 0) {
        notify_printf(
            evaluation, player,
            "      Ranging system damage (%d hit%s): +%d to hit at >%d hexes.",
            awDamage[1], awDamage[1] > 1 ? "s" : "", awDamage[1],
            MechWeapons[wWeapIndex].shortrange);
      }

      if (awDamage[2] > 0) {
        notify_printf(evaluation, player,
                      "      Ammo feed damage (%d hit%s): Can't switch ammo. "
                      "Explodes on %d or less.",
                      awDamage[2], awDamage[2] > 1 ? "s" : "", awDamage[2] + 1);
      }
    } else if (weapon_catalogue_is_ballistic(wWeapIndex) ||
               weapon_catalogue_is_artillery(wWeapIndex)) {
      if (awDamage[1] > 0) {
        notify_printf(evaluation, player,
                      "      [fg=red bold]Barrel damage (%d hit%s): Jams on a "
                      "%d or less.[reset]",
                      awDamage[1], awDamage[1] > 1 ? "s" : "", awDamage[1] + 1);
      }

      if (awDamage[2] > 0) {
        notify_printf(evaluation, player,
                      "      Ammo feed damage (%d hit%s): Can't switch ammo. "
                      "Explodes on %d or less.",
                      awDamage[2], awDamage[2] > 1 ? "s" : "", awDamage[2] + 1);
      }
    }

    if ((awDamage[0] == 0) && (awDamage[1] == 0) && (awDamage[2] == 0))
      notify_printf(evaluation, player,
                    "      Damaged, but fully operational.");
    tPrintSpace = 1;
  }

  if (wAmmoLoc >= 0) {
    ArmorStringFromIndex(wAmmoLoc, strLocation, mech_class(mech),
                         mech_movement_type(mech));
    notify_printf(evaluation, player, "      Prefered ammo source: %s",
                  strLocation);
    tPrintSpace = 1;
  }

  if ((awNonOpCrits[0] > 0) || (awNonOpCrits[1] > 0) || (awNonOpCrits[2] > 0)) {
    notify_printf(evaluation, player,
                  "      Slot status: Damaged: %d. Destroyed: %d. Disabled: %d",
                  awNonOpCrits[0], awNonOpCrits[1], awNonOpCrits[2]);
    tPrintSpace = 1;
  }

  if (tPrintSpace)
    mecha_notify(evaluation, player, " ");
}
