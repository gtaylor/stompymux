/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *       All rights reserved
 */

#include "mech_hitloc_internal.h"

/* Do L3 FASA motive system crits */
void DoMotiveSystemHit(Mech *mech, int wRollMod) {
  int wRoll;
  const char MAX_LEN = 64;
  char strVhlTypeName[64];

  wRoll = btech_random_roll(mech->xcode.context) + wRollMod;

  switch (MechMove(mech)) {
  case MOVE_TRACK:
    strcpy(strVhlTypeName, "tank");
    break;
  case MOVE_WHEEL:
    strcpy(strVhlTypeName, "vehicle");
    wRoll += 2;
    break;
  case MOVE_HOVER:
    strcpy(strVhlTypeName, "hovercraft");
    wRoll += 4;
    break;
  case MOVE_HULL:
    strcpy(strVhlTypeName, "ship");
    break;
  case MOVE_FOIL:
    strcpy(strVhlTypeName, "hydrofoil");
    wRoll += 4;
    break;
  case MOVE_SUB:
    strncpy(strVhlTypeName, "submarine", MAX_LEN);
    break;
  default:
    strncpy(strVhlTypeName, "weird unidentifiable toy (warn a wizard!)",
            MAX_LEN);
    break;
  }

  if (wRoll < 8) /* no effect */
    return;

  mech_notify(mech, MECHALL, "[fg=yellow bold]CRITICAL HIT![reset]");

  if (wRoll < 10) { /* minor effect */
    MechPilotSkillBase(mech) += 1;

    if (Fallen(mech))
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    else
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system takes a minor hit, making it "
          "harder to control your %s![reset]",
          strVhlTypeName);

    if (MechSpeed(mech) != 0.0)
      mech_los_broadcast(mech, "wobbles slightly.");
  } else if (wRoll < 12) { /* moderate effect */
    MechPilotSkillBase(mech) += 2;

    if (Fallen(mech))
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    else
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system takes a moderate hit, slowing "
          "you down and making it harder to control your %s![reset]",
          strVhlTypeName);

    if (MechSpeed(mech) != 0.0)
      mech_los_broadcast(mech, "wobbles violently.");

    mech_max_speed_lower(mech, MP1);
    correct_speed(mech);
  } else {
    if (Fallen(mech))
      mech_notify(mech, MECHALL,
                  "[fg=red bold]Your destroyed motive system takes another "
                  "hit![reset]");
    else
      mech_printf(
          mech, MECHALL,
          "[fg=red bold]Your motive system is destroyed! Your %s can no "
          "longer move![reset]",
          strVhlTypeName);

    if (MechSpeed(mech) > 0)
      mech_los_broadcast(mech, "shakes violently then begins to slow down.");

    mech_max_speed_set(mech, 0.0);
    mech_make_fall(mech);
    correct_speed(mech);
  }
}
