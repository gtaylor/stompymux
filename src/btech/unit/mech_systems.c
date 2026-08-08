#include "checked_conversion.h"
#include "mech_classification_api.h"
#include "mech_crew_api.h"
#include "mech_equipment_api.h"
#include "mech_position_api.h"
#include "mech_utils_internal.h"
#include "registry_api.h"
#include "weapon_catalogue_api.h"

static float integer_as_float(int value) { return (float)value; }

int btech_random_roll(BtechContext *context) {
  long first_roll = btech_random_range(context, 1, 6);
  long second_roll = btech_random_range(context, 1, 6);
  int i = clamp_intptr_to_int(first_roll + second_roll);

  context->random.statistics.rolls[i - 2]++;
  context->random.statistics.total_rolls++;
  return i;
}

int MyHexDist(int x1, int y1, int x2, int y2, int tc) {
  int xd = abs(x2 - x1);
  int yd = abs(y2 - y1);
  int hm;

  /* _the_ base case */
  if (x1 == x2)
    return yd;
  /*
     +
     +
     +
     +
   */
  if ((hm = (xd / 2)) <= yd)
    return (yd - hm) + tc + xd;

  /*
     +     +
     +   +
     + +
     +
   */
  if (!yd)
    return (xd + tc);
  /*
     +
     +
     +   +
     + +
     +
   */
  /* For now, same as above */
  return (xd + tc);
}

int CountDestroyedLegs(Mech *objMech) {
  int wcDeadLegs = 0;

  if (((objMech)->ud.type) != CLASS_MECH)
    return 0;

  if (mech_is_quad(objMech)) {
    if (IsLegDestroyed(objMech, LARM))
      wcDeadLegs++;

    if (IsLegDestroyed(objMech, RARM))
      wcDeadLegs++;
  }

  if (IsLegDestroyed(objMech, LLEG))
    wcDeadLegs++;

  if (IsLegDestroyed(objMech, RLEG))
    wcDeadLegs++;

  return wcDeadLegs;
}

int IsLegDestroyed(Mech *objMech, int wLoc) {
  return (mech_section_is_destroyed(objMech, wLoc) ||
          mech_section_is_breached(objMech, wLoc) ||
          mech_section_is_flooded(objMech, wLoc));
}

int IsMechLegLess(Mech *objMech) {
  int wcMaxLegs = 0;

  if (((objMech)->ud.type) != CLASS_MECH)
    return 0;

  if (mech_is_quad(objMech))
    wcMaxLegs = 4;
  else
    wcMaxLegs = 2;

  if (CountDestroyedLegs(objMech) >= wcMaxLegs)
    return 1;

  return 0;
}

int FindFirstWeaponCrit(Mech *objMech, int wLoc, int wSlot, int wStartSlot,
                        int wCritType, int wMaxCrits) {
  int wCritsInLoc = 0;
  int wCritIter, wFirstCrit;

  /*
   * First let's count the number of crits in this loc, incase
   * we have two of the same weapon
   */

  wFirstCrit = -1;

  for (wCritIter = wStartSlot; wCritIter < NUM_CRITICALS; wCritIter++) {
    if (mech_critical_part_type(objMech, wLoc, wCritIter) == wCritType) {
      wCritsInLoc++;

      if (wFirstCrit == -1)
        wFirstCrit = wCritIter;
    }
  }

  if ((wFirstCrit > -1) && (wSlot == -1))
    return wFirstCrit;

  /*
   * Now, if there are more crits than our max crit, then we have
   * two of the same weapon in this location. We need to figure
   * out which weapon this crit actually belongs to.
   */
  if (wCritsInLoc > wMaxCrits) {
    /*
     * Well, we have thje first crit of the first instance, so
     * let's see if our crit falls out of that range.. if so, then
     * we need to figure out what range it actually falls into.
     */
    if ((wFirstCrit + wMaxCrits) <= wSlot)
      wFirstCrit = FindFirstWeaponCrit(
          objMech, wLoc, wSlot, (wFirstCrit + wMaxCrits), wCritType, wMaxCrits);
  }

  return wFirstCrit;
}

int checkAllSections(Mech *mech, int specialToFind) {
  int i;

  for (i = 0; i < NUM_SECTIONS; i++) {
    if (checkSectionForSpecial(mech, specialToFind, i))
      return 1;
  }

  return 0;
}

int checkSectionForSpecial(Mech *mech, int specialToFind, int wSec) {
  if (mech_section_is_destroyed(mech, wSec))
    return 0;

  if (((mech)->ud.sections)[wSec].specials & specialToFind)
    return 1;

  return 0;
}

int getRemainingInternalPercent(Mech *mech) {
  int i;
  float wMax = 0.0F;
  float wRemaining = 0.0F;

  for (i = 0; i < NUM_SECTIONS; i++) {
    wMax += integer_as_float(mech_section_original_internal(mech, i));

    wRemaining += integer_as_float(mech_section_internal(mech, i));
  }

  if (wMax <= 0.0F)
    return 0;

  return clamp_float_to_int((wRemaining / wMax) * 100.0F);
}

int getRemainingArmorPercent(Mech *mech) {
  int i;
  float wMax = 0.0F;
  float wRemaining = 0.0F;

  for (i = 0; i < NUM_SECTIONS; i++) {
    wMax += integer_as_float(mech_section_original_armor(mech, i));
    wMax += integer_as_float(mech_section_original_rear_armor(mech, i));

    wRemaining += integer_as_float(mech_section_armor(mech, i));
    wRemaining += integer_as_float(mech_section_rear_armor(mech, i));
  }

  if (wMax <= 0.0F)
    return 0;

  return clamp_float_to_int((wRemaining / wMax) * 100.0F);
}

int FindObj(Mech *mech, int loc, int type) {
  int count = 0, i;

  for (i = 0; i < NUM_CRITICALS; i++)
    if (mech_critical_part_type(mech, loc, i) == type)
      if (!mech_critical_is_nonfunctional(mech, loc, i))
        count++;
  return count;
}

int FindObjWithDest(Mech *mech, int loc, int type) {
  int count = 0, i;

  for (i = 0; i < NUM_CRITICALS; i++)
    if (mech_critical_part_type(mech, loc, i) == type)
      count++;
  return count;
}

/* Usage:
   mech      = Mech who's looking for people
   mech_map  = Map mech's on
   x,y       = Target hex
   needlos   = Bitvector
   1 = Require LOS
   2 = We actually want a mech that is friendly and has LOS to hex
 */
Mech *find_mech_in_hex(Mech *mech, BattleMap *mech_map, int x, int y,
                       int needlos) {
  int loop;
  Mech *target;

  for (loop = 0; loop < mech_map->first_free; loop++)
    if (mech_map->mechsOnMap[loop] != mech->mynum &&
        mech_map->mechsOnMap[loop] != -1) {
      target = (Mech *)btech_context_find_object(mech->xcode.context,
                                                 mech_map->mechsOnMap[loop]);
      if (!target)
        continue;
      if (!(((target)->pd.x) == x && ((target)->pd.y) == y) && !(needlos & 2))
        continue;
      if (needlos) {
        if (needlos & 1)
          if (!mech_los_check(mech, target, x, y, mech_range_to(mech, target)))
            continue;
        if (needlos & 2) {
          if (((mech)->pd.team) != ((target)->pd.team))
            continue;
          if (!(MechSeesHex(target, mech_map, x, y)))
            continue;
          if (mech == target)
            continue;
        }
      }
      return target;
    }
  return NULL;
}

int FindAndCheckAmmo(Mech *mech, int weapindx, int section, int critical,
                     int *ammoLoc, int *ammoCrit, int *ammoLoc1, int *ammoCrit1,
                     int *wGattlingShots) {
  int mod, nmod = 0;
  int wMaxShots = 0;
  int wRoundsToCheck = 1;
  int wWeapMode = mech_critical_fire_mode(mech, section, critical);
  int tResetMode = 0;
  DbRef player = mech_gunner_dbref(mech);

  /* Return if it's an energy or PC weapon */
  if (MechWeapons[weapindx].type == TBEAM ||
      MechWeapons[weapindx].type == THAND)
    return 1;

  /* Check for rocket launchers */
  if (MechWeapons[weapindx].special == ROCKET) {
    if (wWeapMode & ROCKET_FIRED) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "That weapon has already been used!");
      return 0;
    }
    return 1;
  }

  /* Check for One-Shots */
  if (wWeapMode & OS_MODE) {
    if (mech_critical_fire_mode(mech, section, critical) & OS_USED) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "That weapon has already been used!");
      return 0;
    }
    return 1;
  }
  /* Check RACs - No special ammo type possible */
  if (MechWeapons[weapindx].special & RAC) {
    wMaxShots = CountAmmoForWeapon(mech, weapindx);

    if ((wWeapMode & RAC_TWOSHOT_MODE) && (wMaxShots < 2)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_TWOSHOT_MODE);

      return 1;
    }

    if ((wWeapMode & RAC_FOURSHOT_MODE) && (wMaxShots < 4)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_FOURSHOT_MODE);

      return 1;
    }

    if ((wWeapMode & RAC_SIXSHOT_MODE) && (wMaxShots < 6)) {
      mech_critical_fire_mode_clear(mech, section, critical, RAC_SIXSHOT_MODE);

      return 1;
    }
  }
  /* Check GMGs */
  if (wWeapMode & GATTLING_MODE) {
    wMaxShots = CountAmmoForWeapon(mech, weapindx);

    /*
     * Gattling MGs suck up damage * 3 in ammo
     */

    if ((wMaxShots / 3) < *wGattlingShots)
      *wGattlingShots = MAX((wMaxShots / 3), 1);
  }
  /* If we're an ULTRA or RFAC, we need to check for multiple rounds */
  if ((wWeapMode & ULTRA_MODE) || (wWeapMode & RFAC_MODE))
    wRoundsToCheck = 2;

  mod = mech_critical_ammo_mode(mech, section, critical) & AMMO_MODES;

  if (!mod) {
    if (!FindAmmoForWeapon_sub(mech, section, critical, weapindx, section,
                               ammoLoc, ammoCrit, AMMO_MODES, 0)) {
      mecha_notify(
          btech_context_evaluation(mech->xcode.context), player,
          "You don't have any ammo for that weapon stored on this mech!");
      return 0;
    }

    if (!mech_critical_data(mech, *ammoLoc, *ammoCrit)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "You are out of ammo for that weapon!");
      return 0;
    }

    if (wRoundsToCheck > 1) {
      mech_critical_data_set(mech, *ammoLoc, *ammoCrit,
                             mech_critical_data(mech, *ammoLoc, *ammoCrit) - 1);

      if (FindAmmoForWeapon_sub(mech, section, critical, weapindx, section,
                                ammoLoc1, ammoCrit1, AMMO_MODES, 0)) {
        if (!mech_critical_data(mech, *ammoLoc1, *ammoCrit1))
          tResetMode = 1;
      } else
        tResetMode = 1;

      if (tResetMode)
        mech_critical_fire_mode_clear(mech, section, critical, wWeapMode);

      mech_critical_data_set(mech, *ammoLoc, *ammoCrit,
                             mech_critical_data(mech, *ammoLoc, *ammoCrit) + 1);
    }
  } else {
    if (weapon_catalogue_is_artillery(weapindx))
      nmod = (~mod) & ARTILLERY_MODES;
    else
      nmod = (~mod) & AMMO_MODES;
    mod = (mod & AMMO_MODES);

    if (!FindAmmoForWeapon_sub(mech, section, critical, weapindx, section,
                               ammoLoc, ammoCrit, nmod, mod)) {
      mecha_notify(
          btech_context_evaluation(mech->xcode.context), player,
          "You don't have any ammo for that weapon stored on this mech!");
      return 0;
    }

    if (!mech_critical_data(mech, *ammoLoc, *ammoCrit)) {
      mecha_notify(btech_context_evaluation(mech->xcode.context), player,
                   "You are out of the special ammo type for that weapon!");
      return 0;
    }

    if (wRoundsToCheck > 1) {
      mech_critical_data_set(mech, *ammoLoc, *ammoCrit,
                             mech_critical_data(mech, *ammoLoc, *ammoCrit) - 1);

      if (FindAmmoForWeapon_sub(mech, section, critical, weapindx, section,
                                ammoLoc1, ammoCrit1, nmod, mod)) {
        if (!mech_critical_data(mech, *ammoLoc1, *ammoCrit1))
          tResetMode = 1;
      } else
        tResetMode = 1;

      if (tResetMode)
        mech_critical_fire_mode_clear(mech, section, critical, wWeapMode);

      mech_critical_data_set(mech, *ammoLoc, *ammoCrit,
                             mech_critical_data(mech, *ammoLoc, *ammoCrit) + 1);
    }
  }

  return 1;
}

void ChannelEmitKill(Mech *mech, Mech *attacker, const char *reason) {
  if (!attacker)
    attacker = mech;

  /* Very Rare Occassion where using btsetxcodevalue(mech,mechdamage,) triggers
   * this, we'll just ignore */
  if ((mech->mynum == attacker->mynum) &&
      !is_good_obj(mech->xcode.context->database, mech->mynum))
    return;

  if (mech != attacker)
    ((attacker)->rd.units_killed) = ((attacker)->rd.units_killed) + 1;

  if (reason) {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("#%ld [%s] has been killed by #%ld [%s] (%s)",
                               mech->mynum, ((mech)->ud.mech_type),
                               attacker->mynum, ((attacker)->ud.mech_type),
                               reason));
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEATHS, "%s",
                       tprintf("#%ld [%s] has been killed by #%ld [%s] (%s)",
                               mech->mynum, ((mech)->ud.mech_type),
                               attacker->mynum, ((attacker)->ud.mech_type),
                               reason));
  } else {
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEBUG, "%s",
                       tprintf("#%ld [%s] has been killed by #%ld [%s]",
                               mech->mynum, ((mech)->ud.mech_type),
                               attacker->mynum, ((attacker)->ud.mech_type)));
    btech_channel_send(mech->xcode.context, BTECH_CHANNEL_MECH_DEATHS, "%s",
                       tprintf("#%ld [%s] has been killed by #%ld [%s]",
                               mech->mynum, ((mech)->ud.mech_type),
                               attacker->mynum, ((attacker)->ud.mech_type)));
  }

  if (mech_is_dropship(mech)) {
    if (reason) {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_DS_INFO, "%s",
                         tprintf("#%ld has been killed by #%ld (%s)",
                                 mech->mynum, attacker->mynum, reason));
    } else {
      btech_channel_send(mech->xcode.context, BTECH_CHANNEL_DS_INFO, "%s",
                         tprintf("#%ld has been killed by #%ld", mech->mynum,
                                 attacker->mynum));
    }
  }

  /* Trigger AMECHDEST.  */
  if (is_good_obj(mech->xcode.context->database, mech->mynum) &&
      is_good_obj(mech->xcode.context->database, attacker->mynum)) {
    char *reason_copy = NULL;

    char *args[1] = {NULL};
    int nargs = 0;

    if (reason) {
      reason_copy = alloc_lbuf("bt.reason");

      if (reason_copy) {
        /* Safe because reason is a KILL_TYPE_*. */
        strcpy(reason_copy, reason);

        args[0] = reason_copy;
        nargs = 1;
      }
    }

    notify_event(btech_context_evaluation(attacker->xcode.context), NULL,
                 attacker->mynum, attacker->mynum, mech->mynum,
                 LUA_EVENT_MECH_DESTROYED, args, nargs);

    if (reason_copy) {
      free_lbuf(reason_copy);
    }
  }
}

#define NUM_NEIGHBORS 6
const int dirs[6][2] = {{0, -1}, {1, 0}, {1, 1}, {0, 1}, {-1, 1}, {-1, 0}};

void visit_neighbor_hexes(BattleMap *map, int tx, int ty,
                          NeighborHexCallback callback, void *context) {
  for (int i = 0; i < NUM_NEIGHBORS; i++) {
    int x1 = tx + dirs[i][0];
    int y1 = ty + dirs[i][1];
    if (tx % 2 && !(x1 % 2))
      y1--;
    if (x1 < 0 || x1 >= map->map_width || y1 < 0 || y1 >= map->map_height)
      continue;
    callback(map, x1, y1, context);
  }
}
