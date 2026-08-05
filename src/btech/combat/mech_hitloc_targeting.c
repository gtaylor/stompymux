/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

/*
 * Total Warfare, p. 119:
 *
 * Mechs have the following attack direction diagram:
 *
 *    FS
 * FS    FS
 * LS    RS
 *    AS
 *
 * Vehicles have the following attack direction diagram (p. 192):
 *
 *    FS
 * LS    RS
 * LS    RS
 *    AS
 *
 * Aerospace units use a diagram similar to that of vehicles.  The full rules
 * start on p. 237, with additional rules for grounded units on p. 249.
 *
 * ProtoMechs, infantry, and buildings do not use attack direction.
 *
 * In a change from the previous implementation, we use the bearing from the
 * target to the attacker, since this uses angles which seem more intuitive.
 *
 * Note that these arcs haven't been traditional in BTMUX usage.  Instead, the
 * following rules had been adopted:
 *
 * FS: front 90 degrees
 * AS: back 90 degrees
 * LS: left 90 degrees
 * RS: right 90 degree
 *
 * They had also previously claimed some "official" BT arcs which don't match
 * the BMR.  We'll use these arcs for our "old" BT option instead:
 *
 *    FS
 * FS    FS
 * LS    RS
 *    AS
 *
 * (The main difference between BMR and TW is that vehicles use the same arcs
 * as mechs do in BMR.)
 *
 * The particular rules desired may be set using the "btech_hit_arcs"
 * configuration option.  0 = TW rules (default), 1 = "classic" BTMUX rules,
 * 2 = "official" BT rules.
 *
 * The main problem with the traditional BTMUX arcs is that they tend to
 * over-emphasize rear area shots, and under-emphasize frontal shots, relative
 * to what the original designs had been intended for.  This tended to
 * over-emphasize the weakness of the rear area, relative to the TBS rules.
 *
 * The new arcs reduce the rear area on both mechs and vehicles by 33%, and
 * reduce the front arc on vehicles also by 33%, while increasing the front arc
 * on mechs by a whopping 100%.  Side arcs on vehicles will increase by 33%,
 * while decreasing by 33% on mechs, and shifting towards the rear.
 *
 * I'm guessing the overall effect of these changes will be to lengthen
 * battles, since more shots will tend to spread themselves out over the front
 * side, at least on mechs.  This will also make it a bit harder for defenders
 * to sponge specific arcs, as well as for attackers to target specific arcs.
 *
 * For vehicles, the reduction in rear arc vulnerability will probably be a net
 * positive, but the front arc tends to be strongest, and that will also be
 * reduced slightly in size.
 *
 * Note that this function is for hit location arcs, not firing arcs.  The
 * current firing arc rules are correct, to the best of my knowledge.
 *
 * --Codicus Unitus (cu5)
 */
int FindAreaHitGroup(Mech *mech, Mech *target) {
  int m_fs_hw, fs_hw;
  int m_as_hw, as_hw;

  int ad;

  /*
   * Select rule set.  We compute this every time, to allow for dynamic
   * configuration changes (although we could hook the configuration
   * option instead and just do it once per change).
   *
   * The front side center is always 0 degrees, and the aft side center
   * is always 180 degrees.  We merely configure how wide those arcs are,
   * by setting the size of the half arc to each side of the center (the
   * "half-width").
   *
   * The left side and right side are simply the leftovers, and are
   * determined by whether an arc is less than or greater than 180.
   */
  switch (mech->xcode.context->configuration->btech_hit_arcs) {
  case 0: /* TW rules */
  default:
    m_fs_hw = 90;
    m_as_hw = 30;

    fs_hw = 30;
    as_hw = 30;
    break;

  case 1: /* classic BTMUX rules */
    m_fs_hw = 45;
    m_as_hw = 45;

    fs_hw = m_fs_hw;
    as_hw = m_as_hw;
    break;

  case 2: /* BMR rules */
    m_fs_hw = 90;
    m_as_hw = 30;

    fs_hw = m_fs_hw;
    as_hw = m_as_hw;
    break;
  }

  /* Compute attack direction.  */
  ad = AcceptableDegree(
      FindBearing(MechFX(target), MechFY(target), MechFX(mech), MechFY(mech)) -
      MechFacing(target));

  /* Determine hit group.  */
  switch (MechType(target)) {
  case CLASS_MECH:
    /* Mech rules.  */
    if (ad >= (360 - m_fs_hw) || ad <= (0 + m_fs_hw)) {
      return FRONT;
    } else if (ad >= (180 - m_as_hw) && ad <= (180 + m_as_hw)) {
      return BACK;
    } else if (ad >= 180) {
      return LEFTSIDE;
    } else {
      return RIGHTSIDE;
    }
    break;

  default:
    /*
     * TODO: This function shouldn't really do anything at all for
     * most other types.  As a sensible default, use the vehicle
     * rules, which is no more broken than the old code.
     */
    /* FALLTHROUGH */

  case CLASS_VEH_GROUND:
  case CLASS_VEH_NAVAL:
  case CLASS_VTOL:
    /* Vehicle rules.  */
    if (ad >= (360 - fs_hw) || ad <= (0 + fs_hw)) {
      return FRONT;
    } else if (ad >= (180 - as_hw) && ad <= (180 + as_hw)) {
      return BACK;
    } else if (ad >= 180) {
      return LEFTSIDE;
    } else {
      return RIGHTSIDE;
    }
    break;
  }
}

int FindTargetHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical) {

  int hitGroup;

  *iscritical = 0;
  hitGroup = FindAreaHitGroup(mech, target);

  if (hitGroup == BACK) {
    *isrear = 1;
  }

  if (MechType(target) == CLASS_MECH && (MechStatus(target) & PARTIAL_COVER)) {
    return FindPunchLocation(target, hitGroup);
  }

  if (MechType(mech) == CLASS_MW && MechType(target) == CLASS_MECH &&
      MechZ(mech) <= MechZ(target)) {
    return FindKickLocation(target, hitGroup);
  }

  if (MechType(target) == CLASS_MECH &&
      ((MechType(mech) == CLASS_BSUIT &&
        MechSwarmTarget(mech) == target->mynum))) {
    return find_swarm_hit_location(mech->xcode.context, iscritical, isrear);
  }

  return mech_hit_location(target, hitGroup, iscritical, isrear);
}

int findNARCHitLoc(Mech *mech, Mech *hitMech, int *tIsRearHit) {
  int tIsRear = 0;
  int tIsCritical = 0;
  int wHitLoc = FindTargetHitLoc(mech, hitMech, &tIsRear, &tIsCritical);

  while (GetSectInt(hitMech, wHitLoc) <= 0) {
    wHitLoc = TransferTarget(hitMech, wHitLoc);
    if (wHitLoc < 0)
      return -1;
  }

  *tIsRearHit = 0;
  if (tIsRear) {
    if (MechType(hitMech) == CLASS_MECH)
      *tIsRearHit = 1;
    else if (wHitLoc == FSIDE)
      wHitLoc = BSIDE;
  }

  return wHitLoc;
}

int FindTCHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical) {
  int hitGroup;

  *isrear = 0;
  *iscritical = 0;
  hitGroup = FindAreaHitGroup(mech, target);
  if (hitGroup == BACK)
    *isrear = 1;
  if (MechAimType(mech) == MechType(target) &&
      btech_random_range(mech->xcode.context, 1, 6) >= 3)
    switch (MechType(target)) {
    case CLASS_MECH:
    case CLASS_MW:
      switch (MechAim(mech)) {
      case RARM:
        if (hitGroup != LEFTSIDE)
          return RARM;
        break;
      case LARM:
        if (hitGroup != RIGHTSIDE)
          return LARM;
        break;
      case RLEG:
        if (hitGroup != LEFTSIDE && !(MechStatus(target) & PARTIAL_COVER))
          return RLEG;
        break;
      case LLEG:
        if (hitGroup != RIGHTSIDE && !(MechStatus(target) & PARTIAL_COVER))
          return LLEG;
        break;
      case RTORSO:
        if (hitGroup != LEFTSIDE)
          return RTORSO;
        break;
      case LTORSO:
        if (hitGroup != RIGHTSIDE)
          return LTORSO;
        break;
      case CTORSO:

        /*        if (hitGroup != LEFTSIDE && hitGroup != RIGHTSIDE) */
        return CTORSO;
      case HEAD:
        if (Immobile(target))
          return HEAD;
      }
      break;
    case CLASS_AERO:
    case CLASS_DS:
      switch (MechAim(mech)) {
      case AERO_NOSE:
        if (hitGroup != BACK)
          return AERO_NOSE;
        break;
      case AERO_LWING:
        if (hitGroup != RIGHTSIDE)
          return AERO_LWING;
        break;
      case AERO_RWING:
        if (hitGroup != LEFTSIDE)
          return AERO_RWING;
        break;
      case AERO_AFT:
        if (hitGroup != FRONT)
          return AERO_AFT;
        break;
      }
      [[fallthrough]];
    case CLASS_VEH_GROUND:
    case CLASS_VEH_NAVAL:
    case CLASS_VTOL:
      switch (MechAim(mech)) {
      case RSIDE:
        if (hitGroup != LEFTSIDE)
          return (RSIDE);
        break;
      case LSIDE:
        if (hitGroup != RIGHTSIDE)
          return (LSIDE);
        break;
      case FSIDE:
        if (hitGroup != BACK)
          return (FSIDE);
        break;
      case BSIDE:
        if (hitGroup != FRONT)
          return (BSIDE);
        break;
      case TURRET:
        return (TURRET);
        break;
      }
      break;
    }
  if (MechType(target) == CLASS_MECH && (MechStatus(target) & PARTIAL_COVER))
    return FindPunchLocation(target, hitGroup);
  return mech_hit_location(target, hitGroup, iscritical, isrear);
}

int FindAimHitLoc(Mech *mech, Mech *target, int *isrear, int *iscritical) {
  int hitGroup;

  *isrear = 0;
  *iscritical = 0;
  hitGroup = FindAreaHitGroup(mech, target);
  if (hitGroup == BACK)
    *isrear = 1;
  if (MechType(target) == CLASS_MECH || MechType(target) == CLASS_MW)
    switch (MechAim(mech)) {
    case RARM:
      if (hitGroup != LEFTSIDE)
        return (RARM);
      break;
    case LARM:
      if (hitGroup != RIGHTSIDE)
        return (LARM);
      break;
    case RLEG:
      if (hitGroup != LEFTSIDE && !(MechStatus(target) & PARTIAL_COVER))
        return (RLEG);
      break;
    case LLEG:
      if (hitGroup != RIGHTSIDE && !(MechStatus(target) & PARTIAL_COVER))
        return (LLEG);
      break;
    case RTORSO:
      if (hitGroup != LEFTSIDE)
        return (RTORSO);
      break;
    case LTORSO:
      if (hitGroup != RIGHTSIDE)
        return (LTORSO);
      break;
    case CTORSO:
      return (CTORSO);
    case HEAD:
      return (HEAD);
    }
  else if (is_aero(target))
    return MechAim(mech);
  else
    switch (MechAim(mech)) {
    case RSIDE:
      if (hitGroup != LEFTSIDE)
        return (RSIDE);
      break;
    case LSIDE:
      if (hitGroup != RIGHTSIDE)
        return (LSIDE);
      break;
    case FSIDE:
      if (hitGroup != BACK)
        return (FSIDE);
      break;
    case BSIDE:
      if (hitGroup != FRONT)
        return (BSIDE);
      break;
    case TURRET:
      return (TURRET);
      break;
    }

  if (MechType(target) == CLASS_MECH && (MechStatus(target) & PARTIAL_COVER))
    return FindPunchLocation(target, hitGroup);
  return mech_hit_location(target, hitGroup, iscritical, isrear);
}
