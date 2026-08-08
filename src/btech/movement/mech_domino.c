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
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_crew_api.h"
#include "mech_damage_api.h"
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
#include "mech_targeting_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/objects/flags.h"
#include "mux/server/game.h"
#include "mux/server/platform.h"
#include "mux/support/formatting.h"
#include "registry_api.h"
#include "template_api.h"
int battle_map_mech_count_in_hex(BattleMap *map, int x, int y, int friendly,
                                 int team) {
  Mech *mech;
  int i, cnt = 0;

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((mech = btech_context_get_mech(battle_map_context(map),
                                       battle_map_unit_dbref(map, i)))) {
      if (mech_position_x(mech) != x || mech_position_y(mech) != y)
        continue;
      if (mech_is_destroyed(mech))
        continue;
      if (!(mech_technology_flags_secondary(mech) & CARRIER_TECH) &&
          mech_is_dropship(mech) &&
          (mech_is_landed(mech) || !mech_is_started(mech))) {
        cnt += 2;
        continue;
      }
      if (mech_class(mech) != CLASS_MECH)
        continue;
      if (mech_is_jumping(mech) || mech_is_out_of_control(mech))
        continue;
      if (friendly < 0 || ((mech_team(mech) == team) == friendly))
        cnt++;
    }
  return cnt;
}

typedef enum CollisionDamageTable {
  COLLISION_DAMAGE_NORMAL,
  COLLISION_DAMAGE_PUNCH,
  COLLISION_DAMAGE_KICK,
} CollisionDamageTable;

static void collision_apply_damage(Mech *att, Mech *mech, int dam,
                                   CollisionDamageTable table) {
  int hitGroup, isrear, iscrit = 0, hitloc = 0;
  int i, sp = (dam - 1) / 5;

  if (!dam)
    return;
  if (att == mech)
    hitGroup = FRONT;
  else
    hitGroup = mech_hit_group(att, mech);
  isrear = (hitGroup == BACK);
  if (mech_is_fallen(mech))
    table = COLLISION_DAMAGE_NORMAL;
  for (i = 0; i <= sp; i++) {
    switch (table) {
    case COLLISION_DAMAGE_NORMAL:
      hitloc = mech_hit_location(mech, hitGroup, &iscrit, &isrear);
      break;
    case COLLISION_DAMAGE_PUNCH:
      if (mech_class(mech) != CLASS_MECH) {
        hitloc = mech_hit_location(mech, hitGroup, &iscrit, &isrear);
      } else {
        hitloc = mech_punch_hit_location(mech, hitGroup);
      }
      break;
    case COLLISION_DAMAGE_KICK:
      if (mech_class(mech) != CLASS_MECH) {
        hitloc = mech_hit_location(mech, hitGroup, &iscrit, &isrear);
      } else {
        hitloc = mech_kick_hit_location(mech, hitGroup);
      }
      break;
    }
    if (dam <= 0)
      return;
    DamageMech(mech, att, (att == mech) ? 0 : 1,
               (att == mech) ? -1 : mech_pilot_dbref(att), hitloc, isrear,
               iscrit, dam > 5 ? 5 : dam, 0, -1, 0, -1, 0, 0);
    dam -= 5;
  }
}

static int mech_adjusted_jump_speed_mp(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);

  if (mech_is_under_gravity(mech) && map != nullptr) {
    const int gravity = MAX(50, battle_map_gravity(map));
    speed = speed * 100.0F / (float)gravity;
  }
  return (int)(speed * MP_PER_KPH);
}

static int mech_domino_resolve_in_hex(BattleMap *map, Mech *me, int x, int y,
                                      int friendly, MechDominoMode mode,
                                      int cnt) {
  int tar = btech_random_range_int(mech_context(me), 0, cnt - 1);
  int i, head, td;
  Mech *mech = nullptr;
  int team = mech_team(me);

  for (i = 0; i < battle_map_unit_count(map); i++)
    if ((mech = btech_context_get_mech(battle_map_context(map),
                                       battle_map_unit_dbref(map, i)))) {
      if (mech_position_x(mech) != x || mech_position_y(mech) != y)
        continue;
      if (mech == me)
        continue;
      if (mech_is_dropship(mech) &&
          (mech_is_landed(mech) || !mech_is_started(mech))) {
        tar -= 2;
      } else {
        if (!mech_is_started(mech))
          continue;
        if (mech_class(mech) != CLASS_MECH)
          continue;
        if (mech_is_jumping(mech) || mech_is_out_of_control(mech))
          continue;
        if (friendly < 0 || ((mech_team(mech) == team) == friendly))
          tar--;
        else
          continue;
      }
      if (tar <= 0)
        break;
    }
  if (i == battle_map_unit_count(map))
    return 0;
  /* Now we got a mech we hit, accidentally or otherwise */
  /* Next, we figure out what'll happen */

  /* 'wannabe-charge' is entirely based on the directional difference */
  /* Multiplied by the speed - if both go in same direction at same speed,
     nothing untoward happens (unlikely, though) */
  /* Jumping to a hex with multiple guys is BAD Thing(tm), though */

  switch (mode) {
  case MECH_DOMINO_JUMP:
  case MECH_DOMINO_FALL:
    head = mech_jump_heading_degrees(me);
    td = mech_adjusted_jump_speed_mp(me, map) *
         (mech_calculated_weight(me) / 1024 + 5) / 10;
    break;
  case MECH_DOMINO_GROUND:
  default:
    head = mech_heading_degrees(me) + mech_lateral_movement(me);
    const int heading_delta =
        head - (mech_heading_degrees(mech) + mech_lateral_movement(mech));
    const float relative_speed =
        mech_current_speed(me) -
        mech_current_speed(mech) *
            cosf((float)heading_delta * (float)M_PI / 180.0F);
    const int mech_weight = mech_calculated_weight(me);
    const float damage = fabsf(relative_speed * MP_PER_KPH) *
                         ((float)mech_weight / 1024.0F + 5.0F) / 15.0F;
    td = (int)damage;
    break;
  }
  if (td > 10)
    td = 10 + (td - 10) / 3;
  if (td <= 1) /* No point in 1pt hits */
    return 0;
  switch (mode) {
  case MECH_DOMINO_JUMP:
  case MECH_DOMINO_FALL:
    if (btech_context_stacking_mode(mech_context(mech)) == 2) {
      int factor = btech_context_stacking_damage(mech_context(mech));
      mech_printf(me, MECHALL, "You land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      mech_los_broadcast_unit(me, mech, "lands on %s!");
      if (mech_is_dropship(mech)) {
        collision_apply_damage(me, mech, MAX(1, td * factor / 500),
                               COLLISION_DAMAGE_PUNCH);
        collision_apply_damage(me, me, MAX(1, td * factor / 100),
                               COLLISION_DAMAGE_KICK);
      } else {
        collision_apply_damage(me, mech, MAX(1, td * factor / 100),
                               COLLISION_DAMAGE_PUNCH);
        collision_apply_damage(me, me, MAX(1, td * factor / 500),
                               COLLISION_DAMAGE_KICK);
      }
    } else {
      mech_printf(me, MECHALL, "You nearly land on %s!",
                  mech_to_mech_display_id(me, mech).text);
      mech_printf(mech, MECHALL, "%s nearly lands on you!",
                  mech_to_mech_display_id(mech, me).text);
      mech_los_broadcast_unit(me, mech, "nearly lands on %s!");
      if (!MadePilotSkillRoll(me,
                              cnt + mech_adjusted_jump_speed_mp(me, map) / 2))
        mech_fall(me, 1, mech_adjusted_jump_speed_mp(me, map) / 2);
    }
    return 1;
  case MECH_DOMINO_GROUND:
  default:
    break;
  }
  if (btech_context_stacking_mode(mech_context(mech)) == 2) {
    int factor = btech_context_stacking_damage(mech_context(mech));
    mech_printf(me, MECHALL, "You bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    mech_los_broadcast_unit(me, mech, "bumps into %s!");
    if (mech_is_dropship(mech)) {
      collision_apply_damage(me, mech, MAX(1, td * factor / 500),
                             COLLISION_DAMAGE_NORMAL);
      collision_apply_damage(me, me, MAX(1, td * factor / 100),
                             COLLISION_DAMAGE_NORMAL);
    } else {
      collision_apply_damage(me, mech, MAX(1, td * factor / 100),
                             COLLISION_DAMAGE_NORMAL);
      collision_apply_damage(me, me, MAX(1, td * factor / 500),
                             COLLISION_DAMAGE_NORMAL);
    }
  } else {
    mech_printf(me, MECHALL, "You nearly bump into %s!",
                mech_to_mech_display_id(me, mech).text);
    mech_printf(mech, MECHALL, "%s nearly bumps into you!",
                mech_to_mech_display_id(mech, me).text);
    mech_los_broadcast_unit(me, mech, "nearly bumps into %s!");
    if (!MadePilotSkillRoll(me, cnt))
      mech_fall(me, 1, 0);
    mech_movement_stop(me);
  }
  mech_charge_reset(me);
  return 1;
}

int mech_domino_resolve(Mech *mech, MechDominoMode mode) {
  BattleMap *map =
      btech_context_get_map(mech_context(mech), mech_map_dbref(mech));
  int cnt, fcnt;

  if (!map)
    return 0;
  if (mech_class(mech) != CLASS_MECH)
    return 0;
  if (btech_context_stacking_mode(mech_context(mech)) == 0)
    return 0;
  cnt = battle_map_mech_count_in_hex(map, mech_position_x(mech),
                                     mech_position_y(mech), -1, 0);
  if (cnt <= 2)
    return 0;
  /* Possible nastiness */
  if ((fcnt = battle_map_mech_count_in_hex(map, mech_position_x(mech),
                                           mech_position_y(mech), 1,
                                           mech_team(mech))) > 2)
    return mech_domino_resolve_in_hex(map, mech, mech_position_x(mech),
                                      mech_position_y(mech), 1, mode, fcnt);
  else if (cnt > 6)
    return mech_domino_resolve_in_hex(map, mech, mech_position_x(mech),
                                      mech_position_y(mech), 0, mode,
                                      cnt - fcnt);
  return 0;
}
