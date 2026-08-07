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
#include "map.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
#include "mech_equipment_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_hitloc_api.h"
#include "mech_ice_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_physical_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "section_types.h"
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
  BtechContext *context = mech_context(mech);
  BattleMap *map = btech_context_get_map(context, mech_map_dbref(mech));
  int terrain;
  int limbs = 4;
  int aLimbs[] = {RARM, LARM, LLEG, RLEG};
  int i;
  int tempLoc;
  char locName[50];
  int damage, tempDamage;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  if (!mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You need to be prone to thrash!");
    return;
  }
  if (!map) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid map! Contact a wizard!");
    return;
  }

  terrain =
      map_real_terrain_get(map, mech_position_x(mech), mech_position_y(mech));

  if (!((terrain == GRASSLAND) || (terrain == ROAD) || (terrain == BRIDGE))) {
    mecha_notify(
        btech_context_evaluation(context), player,
        "Thrashing only works in clear terrain or on roads or bridges.");
    return;
  }

  /* Check locations */
  for (i = 0; i < 4; i++) {
    tempLoc = aLimbs[i];

    if (mech_section_is_destroyed(mech, tempLoc)) {
      limbs--;
      continue;
    }

    ArmorStringFromIndex(tempLoc, locName, mech_class(mech),
                         mech_movement_type(mech));

    if (mech_section_has_recycling_weapon(mech, tempLoc)) {
      mecha_notify(btech_context_evaluation(context), player,
                   tprintf("You have weapons recycling on your %s.", locName));
      return;
    }
    if (mech_section_recycle_ticks(mech, tempLoc)) {
      mecha_notify(btech_context_evaluation(context), player,
                   tprintf("Your %s is still recovering from your last attack.",
                           locName));
      return;
    }
  }

  /* Can't thrash if we have no limbs */
  if (!limbs) {
    mech_notify(mech, MECHALL, "You can't thrash if you have no limbs!");
    return;
  }
#ifndef REALWEIGHT_DAMAGE
  damage = mech_tonnage(mech) / 3;
#else
  damage = mech_real_tonnage(mech) / 3;
#endif /* REALWEIGHT_DAMAGE */

  /* Rules say tonnage/3, not tonnage/3 * limbs  Page 151, Total Warfare*/

  mech_notify(mech, MECHALL,
              "You start to flail your arms and legs like a wild man!");
  mech_los_broadcast(mech,
                     "starts to flail its arms and legs like a wild beast!");

  /* Let's see who we can smack around */
  for (i = 0; i < map->first_free; i++) {
    if (map->mechsOnMap[i] >= 0) {
      target = (Mech *)btech_context_find_object(context, map->mechsOnMap[i]);

      if (!target)
        continue;

      if (mech_class(target) != CLASS_BSUIT)
        continue;

      if (mech_team(target) == mech_team(mech))
        continue;

      if (mech_is_jumping(target) || mech_is_out_of_control(target))
        continue;

      if (mech_range_to(mech, target) > 1.0)
        continue;

      mech_printf(mech, MECHALL, "You manage to hit %s!",
                  mech_to_mech_display_id(mech, target).text);
      mech_printf(target, MECHALL, "You get hit by %s's thrashing limbs!",
                  mech_to_mech_display_id(target, mech).text);

      tempDamage = damage;

      while (tempDamage > 0) {
        if (tempDamage > 5) {
          DamageMech(target, mech, 1, mech_pilot_dbref(mech),
                     btech_random_range(context, 0, NUM_BSUIT_MEMBERS - 1), 0,
                     0, 5, 0, -1, 0, -1, 0, 1);
          tempDamage -= 5;
        } else {
          DamageMech(target, mech, 1, mech_pilot_dbref(mech),
                     btech_random_range(context, 0, NUM_BSUIT_MEMBERS - 1), 0,
                     0, tempDamage, 0, -1, 0, -1, 0, 1);
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

    if (mech_section_is_destroyed(mech, tempLoc))
      continue;

    mech_set_recycle_limb(mech, tempLoc, PHYSICAL_RECYCLE_TIME);
  }
}
