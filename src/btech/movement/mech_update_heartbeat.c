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

void mech_update(DbRef key, void *data) {
  Mech *mech = (Mech *)data;

  if (!mech)
    return;
  MechStatus(mech) &= ~FIRED;
  if (is_aero(mech)) {
    aero_update(mech);
    return;
  }
  if (Started(mech) || Uncon(mech))
    UpdatePilotSkillRolls(mech);
  if (Started(mech) || MechPlusHeat(mech) > 0.1)
    UpdateHeat(mech);
  if (Started(mech))
    MechVisMod(mech) = BOUNDED(
        0, MechVisMod(mech) + btech_random_range(mech->xcode.context, -40, 40),
        100);
  mech_ecm_check(mech);
  mech_tag_check(mech);
  end_lite_check(mech);

  if (MechStatus2(mech) & AUTOTURN_TURRET)
    updateAutoturnTurret(mech);
}
