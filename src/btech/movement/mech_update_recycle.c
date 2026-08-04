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

int recycle_weaponry(Mech *mech) {

  int loop;
  int count, i;
  int crit[MAX_WEAPS_SECTION];
  unsigned char weaptype[MAX_WEAPS_SECTION];
  unsigned char weapdata[MAX_WEAPS_SECTION];
  char location[20];

  int diff = (mech->xcode.context->events->tick - MechLWRT(mech));
  int lowest = 0;

  if (diff < 1) {
    if (diff < 0)
      MechLWRT(mech) = mech->xcode.context->events->tick;
    return 1;
  }
  MechLWRT(mech) = mech->xcode.context->events->tick;

  if (!Started(mech) || Destroyed(mech))
    return 0;

  mech->xcode.context->combat_overrides.arcs = 1;
  for (loop = 0; loop < NUM_SECTIONS; loop++) {
    count = FindWeapons(mech, loop, weaptype, weapdata, crit);
    for (i = 0; i < count; i++) {
      if (WpnIsRecycling(mech, loop, crit[i])) {
        /* Immediate recycle if its destroyed */
        if (PartTempNuke(mech, loop, crit[i]) == FAIL_DESTROYED ||
            SectIsDestroyed(mech, loop))
          GetPartData(mech, loop, crit[i]) = 0;
        if (diff >= GetPartData(mech, loop, crit[i])) {
          GetPartData(mech, loop, crit[i]) = 0;
          /*
           * The ROCKET_FIRED branch intentionally selects an empty format
           * to suppress any recycle notification for that case.
           */
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wformat-zero-length"
#endif
          mech_printf(
              mech, MECHSTARTED,
              MechType(mech) == CLASS_MW
                  ? "[fg=green]You are ready to attack again with %s.[reset]"
              : PartTempNuke(mech, loop, crit[i]) != 0
                  ? "[fg=green]%s is operational again.[reset]"
              : (GetPartFireMode(mech, loop, crit[i]) & ROCKET_FIRED)
                  ? ""
                  : "[fg=green]%s finished recycling.[reset]",
              &MechWeapons[weaptype[i]].name[3]);
#ifdef __clang__
#pragma clang diagnostic pop
#endif
          SetPartTempNuke(mech, loop, crit[i], 0);
        } else {
          if (PartTempNuke(mech, loop, crit[i]) != FAIL_DESTROYED) {
            GetPartData(mech, loop, crit[i]) -= diff;
            if (GetPartData(mech, loop, crit[i]) < lowest || !lowest)
              lowest = GetPartData(mech, loop, crit[i]);
          }
        }
      }
    }

    /* Cycle a section */
    if (MechSections(mech)[loop].recycle &&
        ((MechType(mech) == CLASS_MECH) || (MechType(mech) == CLASS_BSUIT) ||
         (MechType(mech) == CLASS_VEH_GROUND) ||
         (MechType(mech) == CLASS_VTOL))) {

      /* Is the section finished cycling or do we deincrement it */
      if (diff >= MechSections(mech)[loop].recycle &&
          !SectIsDestroyed(mech, loop)) {

        MechSections(mech)[loop].recycle = 0;
        ArmorStringFromIndex(loop, location, MechType(mech), MechMove(mech));

        mech_printf(mech, MECHSTARTED,
                    "[fg=green]%s%s has finished its previous action.[reset]",
                    MechType(mech) == CLASS_BSUIT ? "" : "Your ", location);

      } else {

        MechSections(mech)[loop].recycle -= diff;
        if (MechSections(mech)[loop].recycle < lowest || !lowest)
          lowest = MechSections(mech)[loop].recycle;
      }
    }
  }
  mech->xcode.context->combat_overrides.arcs = 0;
  return lowest;
}

int SkidMod(float Speed) {
  if (Speed < 2.1)
    return -1;
  if (Speed < 4.1)
    return 0;
  if (Speed < 7.1)
    return 1;
  if (Speed < 10.1)
    return 2;
  return 4;
}

/*
 * Move the unit back to its previous location because of cliff or something
 */
void move_unit_back(Mech *mech, float deltax, float deltay, int lastelevation,
                    int ot, int le) {

  MechFX(mech) -= deltax;
  MechFY(mech) -= deltay;
  MechX(mech) = MechLastX(mech);
  MechY(mech) = MechLastY(mech);
  MechZ(mech) = lastelevation;
  MechFZ(mech) = MechZ(mech) * ZSCALE;
  MechTerrain(mech) = ot;
  MechElev(mech) = le;
}
