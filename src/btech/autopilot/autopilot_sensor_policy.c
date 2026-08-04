/*
 * $Id: autogun.c,v 1.5 2005/08/03 21:40:54 av1-op Exp $
 *
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 *
 * Created: Sun Nov 17 13:23:20 1996 fingon
 * Last modified: Sun Jun 14 16:29:44 1998 fingon
 *
 */

#include "autopilot_autogun_internal.h"

/* Function to determine if there are any slites affecting the AI */
int SearchLightInRange(Mech *mech, BattleMap *map) {

  Mech *target;
  int i;

  /* Make sure theres a valid mech or map */
  if (!mech || !map)
    return 0;

  /* Loop through all the units on the map */
  for (i = 0; i < map->first_free; i++) {

    /* No units on the map */
    if (!(target = btech_context_find_object(mech->xcode.context,
                                             map->mechsOnMap[i])))
      continue;

    /* The unit doesn't have slite on */
    if (!(MechSpecials(target) & SLITE_TECH) ||
        MechCritStatus(mech) & SLITE_DEST)
      continue;

    /* Is the mech close enough to be affected by the slite */
    if (FaMechRange(target, mech) < LITE_RANGE) {

      /* Returning true, but let's differentiate also between being in-arc. */
      if ((MechStatus(target) & SLITE_ON) &&
          InWeaponArc(target, MechFX(mech), MechFY(mech)) & FORWARDARC) {

        /* Make sure its in los */
        if (!(map->LOSinfo[target->mapnumber][mech->mapnumber] &
              MECHLOSFLAG_BLOCK))

          /* Slite on and, arced, and LoS to you */
          return 3;
        else
          /* Slite on, arced, but LoS blocked */
          return 4;

      } else if (!(MechStatus(target) & SLITE_ON) &&
                 InWeaponArc(target, MechFX(mech), MechFY(mech)) & FORWARDARC) {

        if (!(map->LOSinfo[target->mapnumber][mech->mapnumber] &
              MECHLOSFLAG_BLOCK))

          /* Slite off, arced, and LoS to you */
          return 5;

        else
          /* Slite off, arced, and LoS blocked */
          return 6;
      }

      /* Slite is in range of you, but apparently not arced on you.
       * Return tells wether on or off */
      return (MechStatus(target) & SLITE_ON ? 1 : 2);
    }
  }
  return 0;
}

/* Function to determine if the AI should use V or L sensor */
int PrefVisSens(Mech *mech, BattleMap *map, int slite, Mech *target) {

  /* No map or mech so use default till we get put somewhere */
  if (!mech || !map)
    return SENSOR_VIS;

  /* Ok the AI is lit or using slite so use V */
  if (MechStatus(mech) & SLITE_ON || MechCritStatus(mech) & SLITE_LIT)
    return SENSOR_VIS;

  /* The target is lit so use V */
  if (target && MechCritStatus(target) & SLITE_LIT)
    return SENSOR_VIS;

  /* Ok if its night/dawn/dusk and theres no slite use L */
  if (map->maplight <= 1 && slite != 3 && slite != 5)
    return SENSOR_LA;

  /* Default sensor */
  return SENSOR_VIS;
}

/*
 * AI event to let the AI decide what sensors to use for a given
 * target and situation
 */
/*! \todo {Improve this so it knows more about the terrain} */
void auto_sensor_event(Autopilot *autopilot) {
  Mech *target = NULL;
  BattleMap *map;
  int wanted_s[2];
  int rvis;
  int slite, prefvis;
  float trng;
  int set = 0;

  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mymechnum)) {
    dprintk("mymechnum is bad!");
    return;
  }
  if (!is_good_obj(autopilot->xcode.context->database, autopilot->mynum)) {
    dprintk("mynum is bad!");
    return;
  }

  Mech *mech = (Mech *)autopilot->mymech;

  /* Make sure its a MECH Xcode Object and the AI is
   * an AUTOPILOT Xcode Object */
  /* Basic checks */
  if (!mech) {
    dprintk("mech is bad!");
    return;
  }
  if (!autopilot) {
    dprintk("ai is bad!");
    return;
  }

  if (!btech_context_is_mech(mech->xcode.context, mech->mynum) ||
      !btech_context_is_auto(autopilot->xcode.context, autopilot->mynum))
    return;

  /* Mech is dead so stop trying to shoot things */
  if (Destroyed(mech)) {
    DoStopGun(autopilot);
    return;
  }

  /* Mech isn't started */
  if (!Started(mech)) {
    Zombify(autopilot);
    return;
  }

  /* The mech is using user defined sensors so don't try
   * and change them */
  if (autopilot->flags & AUTOPILOT_LSENS)
    return;

  /* Get the map */
  if (!(map = btech_context_get_map(mech->xcode.context, mech->mapindex))) {

    /* Bad Map */
    Zombify(autopilot);
    return;
  }

  /* Get the target if there is one */
  if (MechTarget(mech) > 0)
    target = btech_context_get_mech(mech->xcode.context, MechTarget(mech));

  /* Checks to see if there is slite, and what types of vis
   * and which visual sensor (V or L) to use */
  slite = (map->mapvis != 2 ? SearchLightInRange(mech, map) : 0);
  rvis = (map->maplight ? (map->mapvis) : (map->mapvis * (slite ? 1 : 3)));
  prefvis = PrefVisSens(mech, map, slite, target);

  /* Is there a target */
  if (target) {

    /* Range to target */
    trng = FaMechRange(mech, target);

    /* Actually not gonna bother with this */
    /* If the target is running hot and is close switch to IR */
    if (!set && HeatFactor(target) > 35 && (int)trng < 15) {
      // wanted_s[0] = SENSOR_IR;
      // wanted_s[1] = ((MechTons(target) >= 60) ? SENSOR_EM : prefvis);
      // set++;
    }

    /* If the target is BIG and close enough, use EM */
    if (!set && MechTons(target) >= 60 && (int)trng <= 20) {
      wanted_s[0] = SENSOR_EM;
      wanted_s[1] = SENSOR_IR;
      set++;
    }

    /* If the target is flying switch to Radar */
    if (!set && !Landed(target) && FlyingT(target)) {
      wanted_s[0] = SENSOR_RA;
      wanted_s[1] = prefvis;
      set++;
    }

    /* If the target is really close and the unit has BAP, use it */
    if (!set && (int)trng <= 4 && MechSpecials(mech) & BEAGLE_PROBE_TECH &&
        !(MechCritStatus(mech) & BEAGLE_DESTROYED)) {
      wanted_s[0] = SENSOR_BAP;
      wanted_s[1] = SENSOR_BAP;
      set++;
    }

    /* If the target is really close and the unit has Bloodhound, use it */
    if (!set && (int)trng <= 8 && MechSpecials2(mech) & BLOODHOUND_PROBE_TECH &&
        !(MechCritStatus(mech) & BLOODHOUND_DESTROYED)) {
      wanted_s[0] = SENSOR_BHAP;
      wanted_s[1] = SENSOR_BHAP;
      set++;
    }

    /* Didn't stop at any of the others so use selected visual sensors */
    if (!set) {
      wanted_s[0] = prefvis;
      wanted_s[1] = (rvis <= 15 ? SENSOR_EM : prefvis);
      set++;
    }
  }

  /* Ok no target and no sensors set yet so lets go for defaults */
  if (!set) {
    if (rvis <= 15) {
      /* Vis is less then or equal to 15 so go to E I for longer range */
      wanted_s[0] = SENSOR_EM;
      wanted_s[1] = SENSOR_IR;
    } else {
      /* Ok lets go with default visual sensors */
      wanted_s[0] = prefvis;
      wanted_s[1] = prefvis;
    }
  }

  /* Check to make sure valid sensors are selected and then set them */
  if (wanted_s[0] >= SENSOR_VIS && wanted_s[0] <= SENSOR_BHAP &&
      wanted_s[1] >= SENSOR_VIS && wanted_s[1] <= SENSOR_BHAP &&
      (MechSensor(mech)[0] != wanted_s[0] ||
       MechSensor(mech)[1] != wanted_s[1])) {

    wanted_s[0] = BOUNDED(SENSOR_VIS, wanted_s[0], SENSOR_BHAP);
    wanted_s[1] = BOUNDED(SENSOR_VIS, wanted_s[1], SENSOR_BHAP);

    MechSensor(mech)[0] = wanted_s[0];
    MechSensor(mech)[1] = wanted_s[1];
    mech_notify(mech, MECHALL, "As your sensors change, your lock clears.");
    MechTarget(mech) = -1;
    MarkForLOSUpdate(mech);
  }
}

/*
 * Create a weapon_list node
 */
