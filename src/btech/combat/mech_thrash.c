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
#include "legacy_macros.h"
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech.h"
#include "mech_combat_misc_api.h"
#include "mech_damage_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_macros.h"
#include "mech_move_api.h"
#include "mech_notify.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
/*
 * - Only when fallen
 * - Tonnage / 3 (rounded up for .5)
 * - 5 Point groups to PA
 * - Clear or paved terrain only
 * - Automatically works
 * - Doesn't hit suits that are swarmed or jumping
 * - No weapons recycling in arms and legs
 * - Arms and legs recycle after attack
 * - Make pskill roll or take damage as if 1 level fall
 */

void mech_thrash(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *target;
  BattleMap *map = btech_context_get_map(mech->xcode.context, mech->mapindex);
  int terrain;
  int limbs = 4;
  int aLimbs[] = {RARM, LARM, LLEG, RLEG};
  int i;
  int tempLoc;
  char locName[50];
  int damage, tempDamage;

  cch(MECH_USUALO);
  DOCHECK_CONTEXT(mech->xcode.context, !Fallen(mech),
                  "You need to be prone to thrash!");
  DOCHECK_CONTEXT(mech->xcode.context, !map, "Invalid map! Contact a wizard!");

  terrain = map_real_terrain_get(map, MechX(mech), MechY(mech));

  DOCHECK_CONTEXT(
      mech->xcode.context,
      !((terrain == GRASSLAND) || (terrain == ROAD) || (terrain == BRIDGE)),
      "Thrashing only works in clear terrain or on roads or bridges.");

  /* Check locations */
  for (i = 0; i < 4; i++) {
    tempLoc = aLimbs[i];

    if (SectIsDestroyed(mech, tempLoc)) {
      limbs--;
      continue;
    }

    ArmorStringFromIndex(tempLoc, locName, MechType(mech), MechMove(mech));

    DOCHECK_CONTEXT(mech->xcode.context, SectHasBusyWeap(mech, tempLoc),
                    tprintf("You have weapons recycling on your %s.", locName));
    DOCHECK_CONTEXT(
        mech->xcode.context, MechSections(mech)[tempLoc].recycle,
        tprintf("Your %s is still recovering from your last attack.", locName));
  }

  /* Can't thrash if we have no limbs */
  if (!limbs) {
    mech_notify(mech, MECHALL, "You can't thrash if you have no limbs!");
    return;
  }
#ifndef REALWEIGHT_DAMAGE
  damage = MechTons(mech) / 3;
#else
  damage = MechRealTons(mech) / 3;
#endif /* REALWEIGHT_DAMAGE */

  /* Rules say tonnage/3, not tonnage/3 * limbs  Page 151, Total Warfare*/

  mech_notify(mech, MECHALL,
              "You start to flail your arms and legs like a wild man!");
  mech_los_broadcast(mech,
                     "starts to flail its arms and legs like a wild beast!");

  /* Let's see who we can smack around */
  for (i = 0; i < map->first_free; i++) {
    if (map->mechsOnMap[i] >= 0) {
      target = (Mech *)btech_context_find_object(mech->xcode.context,
                                                 map->mechsOnMap[i]);

      if (!target)
        continue;

      if (MechType(target) != CLASS_BSUIT)
        continue;

      if (MechTeam(target) == MechTeam(mech))
        continue;

      if (Jumping(target) || OODing(target))
        continue;

      if (FaMechRange(mech, target) > 1.0)
        continue;

      mech_printf(mech, MECHALL, "You manage to hit %s!",
                  mech_to_mech_display_id(mech, target).text);
      mech_printf(target, MECHALL, "You get hit by %s's thrashing limbs!",
                  mech_to_mech_display_id(target, mech).text);

      tempDamage = damage;

      while (tempDamage > 0) {
        if (tempDamage > 5) {
          DamageMech(
              target, mech, 1, MechPilot(mech),
              btech_random_range(mech->xcode.context, 0, NUM_BSUIT_MEMBERS - 1),
              0, 0, 5, 0, -1, 0, -1, 0, 1);
          tempDamage -= 5;
        } else {
          DamageMech(
              target, mech, 1, MechPilot(mech),
              btech_random_range(mech->xcode.context, 0, NUM_BSUIT_MEMBERS - 1),
              0, 0, tempDamage, 0, -1, 0, -1, 0, 1);
          tempDamage = 0;
        }
      }
    }
  }

  /* Make our roll and recycle our limbs -- Removed. You gotta be prone anyways!
   */
  /* Dunno who commented this out. This is what it should be. You make a pilot
   * roll. if you miss, you take 1 level falling damage to emulate hitting
   * yourself */

  if (!MadePilotSkillRoll_Advanced(mech, 0, 0)) {
    mech_fall(mech, 1, 1);
  }

  for (i = 0; i < 4; i++) {
    tempLoc = aLimbs[i];

    if (SectIsDestroyed(mech, tempLoc))
      continue;

    mech_set_recycle_limb(mech, tempLoc, PHYSICAL_RECYCLE_TIME);
  }
}
