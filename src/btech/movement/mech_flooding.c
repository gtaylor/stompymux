/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *  Copyright (c) 1998-2002 Thomas Wouters
 *  Copyright (c) 2000-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 */

#include <ctype.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "environment_damage_api.h"
#include "legacy_macros.h"
#include "map.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_stagger.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
/* Flooding code. Once we're in water, this is checked
   now and then (basically when DamageMech'ed and/or
   depth changes and/or we fall) */

void MechFloodsLoc(Mech *mech, int loc, int lev) {
  char locbuff[32];
  ;

  if (MechStatus(mech) & COMBAT_SAFE)
    return;

  if ((GetSectArmor(mech, loc) &&
       (GetSectRArmor(mech, loc) || !GetSectORArmor(mech, loc))) ||
      !GetSectInt(mech, loc))
    return;
  if (!InWater(mech))
    return;
  if (lev >= 0)
    return;
  /* No armor, and in water. */
  if (lev == -1 && (!Fallen(mech) && loc != LLEG && loc != RLEG &&
                    (!MechIsQuad(mech) || (loc != LARM && loc != RARM))))
    return;
  if (MechType(mech) != CLASS_MECH)
    return;

  if (SectIsFlooded(mech, loc))
    return;

  /* Woo, valid target. */
  ArmorStringFromIndex(loc, locbuff, MechType(mech), MechMove(mech));
  mech_printf(
      mech, MECHALL,
      "[fg=red bold]Water floods into your %s disabling everything that was "
      "there![reset]",
      locbuff);
  mech_los_broadcast(
      mech, tprintf("has a gaping hole in %s, and water pours in!", locbuff));

  SetSectFlooded(mech, loc);
  mech_parts_destroy(mech, mech, loc, 1, 1);
}

void MechFloods(Mech *mech) {
  int i;
  int elev = MechElevation(mech);

  if (!InWater(mech))
    return;

  /* Waterproof Tech - no flooding if we have this */
  if (MechSpecials2(mech) & WATERPROOF_TECH)
    return;

  if (MechType(mech) == CLASS_BSUIT) {

    if (MechSwarmTarget(mech) > 0)
      return;

    mech_notify(mech, MECHALL,
                "You somehow find yourself in water and realize this may "
                "really really suck...");
    mech_notify(mech, MECHALL,
                "Everything gets very dark as water starts to fill your suit "
                "and you sink towards the bottom!");

    mech_los_broadcast(
        mech, "shudders, splashes in the water for a second, then goes limp "
              "and sinks to the bottom.");

    KillMechContentsIfIC(mech);
    DestroyMech(mech, mech, 0, KILL_TYPE_FLOOD);
    return;
  }

  if (MechType(mech) != CLASS_MECH)
    return;

  if (MechZ(mech) >= 0)
    return;

  for (i = 0; i < NUM_SECTIONS; i++)
    MechFloodsLoc(mech, i, elev);
}
