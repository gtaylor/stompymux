/*
 * Author: Cord Awtry <kipsta@mediaone.net>
 *  Copyright (c) 2001-2002 Cord Awtry
 *  Copyright (c) 1999-2005 Kevin Stevens
 *       All rights reserved
 *
 * Based on work that was:
 *  Copyright (c) 1997 Markus Stenberg
 *  Copyright (c) 1998-2001 Thomas Wouters
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>

#include "btconfig.h"
#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "command_handlers_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_bth_api.h"
#include "mech_classification_api.h"
#include "mech_combat_api.h"
#include "mech_crew_api.h"
#include "mech_electronics_api.h"
#include "mech_events.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_spot_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mux/network/mux_event_alloc.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "registry_api.h"
#include "section_types.h"
#include "weapon_catalogue_api.h"

typedef struct SpotLinkEventData {
  Mech *target;
  float observer_x;
  float observer_y;
  float target_x;
  float target_y;
} SpotLinkEventData;

static bool positions_differ(float first, float second) {
  return fabsf(first - second) > 0.0001F;
}

static float scaled_hex_elevation(int elevation) {
  return ZSCALE * (float)elevation;
}

static bool mech_is_in_water(Mech *mech) {
  const char terrain = mech_real_terrain_get(mech);
  return (terrain == BATTLE_TERRAIN_ICE || terrain == BATTLE_TERRAIN_WATER ||
          terrain == BATTLE_TERRAIN_BRIDGE) &&
         mech_position_z(mech) < 0;
}

bool mech_spot_has_artillery(Mech *mech) {
  int weapnum, section, critical, weaptype = -2;

  for (weapnum = 0; weaptype != -1; weapnum++) {
    weaptype = FindWeaponNumberOnMech(mech, weapnum, &section, &critical);
    if (weapon_catalogue_is_artillery(weaptype))
      return 1;
  }
  return 0;
}

static void mech_check_range(MuxEvent *e) {
  Mech *spotter = (Mech *)e->data2, *mech = (Mech *)e->data;
  float range;

  if (!mech)
    return;

  if (mech_spotter_dbref(mech) == -1)
    return;

  if (!spotter) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  range = mech_range_to(mech, spotter);
  const int maximum_range = 2 * mech_radio_range(spotter);
  if (range > (float)maximum_range || mech_spotter_dbref(spotter) == -1 ||
      mech_map_dbref(spotter) != mech_map_dbref(mech)) {
    mech_notify(mech, MECHALL, "You have lost link with your spotter!");
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)spotter);
}

static void mech_spot_event(MuxEvent *e) {
  Mech *target, *mech = (Mech *)e->data;
  SpotLinkEventData *sd = (SpotLinkEventData *)e->data2;

  target = sd->target;

  if (positions_differ(mech_position_real_x(mech), sd->observer_x) &&
      positions_differ(mech_position_real_y(mech), sd->observer_y) &&
      positions_differ(mech_position_real_x(target), sd->target_x) &&
      positions_differ(mech_position_real_y(target), sd->target_y)) {
    mech_notify(target, MECHALL,
                "The data link was not established due to movement!");
    mech_notify(mech, MECHALL,
                "The data link was not established due to movement!");
    free((void *)e->data2);
    return;
  }
  mech_printf(target, MECHALL, "Data link established with %s.",
              mech_to_mech_display_id(target, mech).text);
  mech_printf(mech, MECHALL,
              "Data link established with %s, you now have a forward observer.",
              mech_to_mech_display_id(target, mech).text);
  mech_spotter_dbref_set(mech, mech_dbref(target));
  mech_event_schedule(mech, EVENT_SPOT_CHECK, mech_check_range, SPOT_TICK,
                      (intptr_t)target);
  free((void *)e->data2);
}

void mech_spot_clear_fire_adjustments(BattleMap *map, DbRef mech) {
  int i;
  Mech *m;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if (battle_map_unit_dbref(map, i) >= 0) {
      if (!(m = btech_context_get_mech(battle_map_context(map),
                                       battle_map_unit_dbref(map, i))))
        continue;
      if (mech_dbref(m) == mech)
        continue;
      if (mech_spotter_dbref(m) == mech)
        mech_fire_adjustment_set(m, 0);
    }
}

void mech_spot(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data, *target;
  char *args[5];
  char targetID[3];
  int argc;
  int LOS = 1;
  DbRef targetref;
  float range;
  SpotLinkEventData *dat;
  BattleMap *mech_map;

  if (!common_checks(player, mech, MECH_USUALO))
    return;
  mech_map = btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  argc = mech_parseattributes(buffer, args, 5);
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You cannot spot while using a special movement mode.");
    return;
  }
#endif
  if (argc != 1) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You may only use mech ID's to set spotter!");
    return;
  }
  if (mech_class(mech) == CLASS_MW) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Spot ? You ? What with, your pretty blue eyes ? Hah!");
    return;
  }
  targetID[0] = args[0][0];
  targetID[1] = *checked_string_suffix(*args, 1);
  targetID[2] = 0;
  targetref = FindTargetDBREFFromMapNumber(mech, targetID);
  if (!strcmp(args[0], "-")) {
    if (mech_spotter_dbref(mech) == mech_dbref(mech)) {
      mech_notify(mech, MECHALL, "You spot no longer.");
      mech_spot_clear_fire_adjustments(mech_map, mech_dbref(mech));
    } else
      mech_notify(mech, MECHALL, "You disable the datalink to spotter.");
    mech_spotter_dbref_set(mech, -1);
    return;
  }
  if (!strcasecmp(targetID, mech_id(mech, false).text)) {
    if (mech_recycling_state(mech, CHECK_BOTH)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You have weapons recycling!");
      return;
    }
    mech_spotter_dbref_set(mech, mech_dbref(mech));
    mech_notify(mech, MECHALL, "You are now set as a spotter.");
    return;
  }
  target = btech_context_get_mech(mech_context(mech), targetref);
  if (target)
    LOS = mech_los_check(mech, target, mech_position_x(target),
                         mech_position_y(target), mech_range_to(mech, target));
  if (!target || (targetref == -1) || mech_team(target) != mech_team(mech)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target does not exist!");
    return;
  }

  if (mech_class(target) == CLASS_MW) {
    mecha_notify(
        btech_context_evaluation(mech_context(mech)), player,
        "Spot ? That puny being ?! What with, those clear brown eyes ? Hah!");
    return;
  }
  if (mech_spotter_dbref(target) != mech_dbref(target)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That 'mech is not set up as spotter!");
    return;
  }

  if (mech_spot_has_artillery(mech) && !LOS) {
    mech_notify(target, MECHALL,
                "Someone is trying to establish a data link with you!");
    mech_notify(mech, MECHALL,
                "You attempt to establish a data link..... please stand by.");
    range = mech_range_to(mech, target);
    const int maximum_range = 2 * mech_radio_range(target);
    if (range > (float)maximum_range) {
      mech_notify(mech, MECHALL, "That target is our of data link range!");
      return;
    }
    Create(dat, SpotLinkEventData, 1);
    dat->observer_y = mech_position_real_y(mech);
    dat->observer_x = mech_position_real_x(mech);
    dat->target_x = mech_position_real_x(target);
    dat->target_y = mech_position_real_y(target);
    dat->target = target;
    // NOLINTNEXTLINE(clang-analyzer-unix.Malloc)
    mech_event_schedule(mech, EVENT_SPOT_LOCK, mech_spot_event,
                        WEAPON_TICK * ((int)range / 10 + 5), (intptr_t)dat);
    return;
  } else if (!LOS) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "You do not have LOS to that target!");
    return;
  }
  mech_spotter_dbref_set(mech, targetref);
  mech_fire_adjustment_set(mech, 0);
  mech_printf(mech, MECHALL, "%s set as spotter.",
              mech_to_mech_display_id(mech, target).text);
}

int mech_spot_fire(DbRef player, Mech *mech, BattleMap *mech_map, int weaponnum,
                   int weapontype, int sight, int section, int critical) {
  /* Nim 9/11/96 */

  float spot_range, range;
  float enemyX, enemyY, enemyZ = 0;
  int LOS, mapx = 0, mapy = 0;
  Mech *target = nullptr, *spotter;
  int spotTerrain;
  bool found_target = false;

  /* No spotter or not IDF weapon lets get outta here */
  if (mech_spotter_dbref(mech) == -1 ||
      !weapon_catalogue_supports_indirect_fire(weapontype))
    return 0;

  spotter =
      btech_context_get_mech(mech_context(mech), mech_spotter_dbref(mech));
  if (!spotter) {
    mech_notify(mech, MECHPILOT, "There is no spotter avilable to IDF with!");
    return 1;
  }

  if (mech_spotter_dbref(spotter) != mech_dbref(spotter)) {
    mech_notify(mech, MECH_PILOT, "You do not have a spotter!");
    mech_spotter_dbref_set(mech, -1);
    return 1;
  }
  if (mech_pilot_is_unconscious(spotter)) {
    mech_notify(mech, MECHPILOT, "Your spotter is unconscious!");
    return 1;
  }
  if (mech_is_blinded(spotter)) {
    mech_notify(mech, MECHPILOT, "Your spotter can't see a thing!");
    return 1;
  }

  /* Is the spotter set to a Mech or to a Hex? */
  if (mech_target_dbref(spotter) != -1) {
    target =
        btech_context_get_mech(mech_context(mech), mech_target_dbref(spotter));
    if (!target) {
      mech_notify(mech, MECHPILOT, "Your spotter has invalid target!");
      return 1;
    }
    mapx = mech_position_x(target);
    mapy = mech_position_y(target);
    spot_range = mech_range_to(spotter, target);
    LOS = mech_los_check(spotter, target, mapx, mapy, spot_range);
    if (!LOS) {
      mech_notify(mech, MECHPILOT,
                  "You spotter does not have a target in LOS!");
      return 1;
    }
    range = mech_range_to(mech, target);
    if (mech_is_in_water(target) && !mech_is_in_water(mech)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You can't fire into water with that weapon from here.");
      return 0;
    }

    spotTerrain =
        weapon_catalogue_is_artillery(weapontype)
            ? 2
            : (1 +
               mech_los_terrain_modifier(spotter, target, mech_map, spot_range,
                                         0) +
               mech_attacker_movement_modifier(spotter) +
               ((mech_event_count(spotter, EVENT_LOCK) &&
                 mech_targeting_computer_type(spotter) != TARGCOMP_MULTI)
                    ? 2
                    : 0));
    if (weapon_catalogue_is_artillery(weapontype) && target) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "You can only target hexes with this kind of artillery.");
      return -1;
    }
    if (!sight) {
      AccumulateSpotXP(mech_pilot_dbref(spotter), spotter, target);
      AccumulateArtyXP(mech_pilot_dbref(mech), mech, target);
    }
    FireWeapon(mech, mech_map, target, 0, weapontype, weaponnum, section,
               critical, mech_position_real_x(target),
               mech_position_real_y(target), mapx, mapy, range, spotTerrain,
               sight, 2);
    return 1;
  }
  if (!(mech_target_hex_x(spotter) >= 0 && mech_target_hex_y(spotter) >= 0)) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "Your spotter has no target set!");
    return 1;
  }
  if (!weapon_catalogue_is_artillery(weapontype))
    if ((target = find_mech_in_hex(mech, mech_map, mech_target_hex_x(spotter),
                                   mech_target_hex_y(spotter), 0))) {
      enemyX = mech_position_real_x(target);
      enemyY = mech_position_real_y(target);
      enemyZ = mech_position_real_z(target);
      mapx = mech_position_x(target);
      mapy = mech_position_y(target);
      found_target = true;
    }
  if (!found_target) {
    target = nullptr;
    mapx = mech_target_hex_x(spotter);
    mapy = mech_target_hex_y(spotter);
    const int target_hex_z = mech_target_hex_z(spotter);
    enemyZ = scaled_hex_elevation(target_hex_z);
    MapCoordToRealCoord(mapx, mapy, &enemyX, &enemyY);
  }
  spot_range =
      FindRange(mech_position_real_x(spotter), mech_position_real_y(spotter),
                mech_position_real_z(spotter), enemyX, enemyY, enemyZ);
  LOS = mech_los_check(spotter, target, mapx, mapy, spot_range);
  if (!LOS) {
    mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                 "That target is not in your spotters line of sight!");
    return 0;
  }
  range = FindRange(mech_position_real_x(mech), mech_position_real_y(mech),
                    mech_position_real_z(mech), enemyX, enemyY, enemyZ);
  spotTerrain =
      weapon_catalogue_is_artillery(weapontype)
          ? 2
          : (1 + mech_attacker_movement_modifier(spotter) +
             ((mech_event_count(spotter, EVENT_LOCK) &&
               mech_targeting_computer_type(spotter) != TARGCOMP_MULTI)
                  ? 2
                  : 0));
  FireWeapon(mech, mech_map, target, 0, weapontype, weaponnum, section,
             critical, enemyX, enemyY, mapx, mapy, range, spotTerrain, sight,
             2);
  return 1;
}
