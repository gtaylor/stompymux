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
#include "map_terrain.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
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
#include "mech_stagger.h"
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
#include "section_types.h"
#include "template_api.h"

static int mech_jump_to_hit_recycle(const Mech *mech) {
  return JUMP_TICK * 12 / (mech_class(mech) == CLASS_BSUIT ? 4 : 1);
}

void mech_land(DbRef player, void *data, char *buffer) {
  Mech *mech = data;
  BtechContext *context = mech_context(mech);

  if (!common_checks(player, mech, MECH_USUAL))
    return;
  if (mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW &&
      mech_class(mech) != CLASS_BSUIT && mech_class(mech) != CLASS_VEH_GROUND) {
    aero_land(player, data, buffer);
    return;
  }
  if (mech_is_jumping(mech)) {
    mech_notify(mech, MECHALL,
                "You abort your full jump and attempt to land early");
    if (MadePilotSkillRoll(mech, 0)) {
      mech_notify(mech, MECHALL, "You are able to abort the jump.");

      /*        mech_los_broadcast (mech, "lands abruptly!"); */
      mech_jump_land(mech);
    } else {
      mech_notify(mech, MECHALL, "You don't quite make it.");
      mech_los_broadcast(mech,
                         "attempts a landing, but crashes to the ground!");
      mech_fall(mech, 1, 0);
      mech_jump_complete(mech);
      mech_maybe_move(mech);
    }
  } else
    mecha_notify(btech_context_evaluation(context), player,
                 "You're not jumping!");
}

static bool mech_is_over_water(const Mech *mech) {
  return mech_movement_type(mech) == MOVE_HOVER ||
         mech_class(mech) == CLASS_MW ||
         mech_movement_type(mech) == MOVE_FOIL ||
         mech_movement_type(mech) == MOVE_HULL;
}

int mech_lower_surface_elevation(Mech *mech) {
  return mech_real_terrain_get(mech) != BATTLE_TERRAIN_BRIDGE
             ? mech_position_surface_elevation(mech)
             : bridge_w_elevation(mech);
}

int mech_drop_surface_elevation(Mech *mech) {
  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE) {
    if (mech_position_z(mech) < mech_position_elevation(mech)) {
      if (mech_is_over_water(mech))
        return 0;
      return bridge_w_elevation(mech);
    }
    return mech_position_surface_elevation(mech);
  }
  if (mech_is_over_water(mech) ||
      (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
       mech_position_z(mech) >= 0))
    return MAX(0, mech_position_surface_elevation(mech));
  else
    return mech_position_surface_elevation(mech);
}

void mech_drop_surface_set(Mech *mech, bool check_ice) {
  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE) {
    bridge_set_elevation(mech);
    return;
  }
  mech_position_z_set(mech, mech_drop_surface_elevation(mech));
  if (check_ice)
    if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
        mech_position_z(mech) >= 0)
      possibly_drop_thru_ice(mech);
}

int mech_drop_height_above_surface(Mech *mech) {
  int upper = mech_upper_surface_elevation(mech);
  int elevation =
      mech_position_z(mech) - (upper <= mech_position_z(mech)
                                   ? upper
                                   : mech_lower_surface_elevation(mech));

  return elevation - mech_drop_surface_elevation(mech);
}

int mech_upper_surface_elevation(Mech *mech) {
  return mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE
             ? 0
             : mech_position_surface_elevation(mech);
}

int mech_height_above_surface(Mech *mech) {
  int upper = mech_upper_surface_elevation(mech);
  return mech_position_z(mech) - (upper <= mech_position_z(mech)
                                      ? upper
                                      : mech_lower_surface_elevation(mech));
}

void mech_jump_land(Mech *mech) {
  Mech *target;
  BtechContext *context = mech_context(mech);
  BattleMap *mech_map = btech_context_get_map(context, mech_map_dbref(mech));
  MechConditionSummary condition = mech_condition_summary(mech);
  int dfa = 0;
  int done = 0;

  /*
   * Added check to see if we're actually awake when we try to land
   * - Kipsta
   * - 8/3/99
   */

  if (mech_pilot_is_unconscious(mech)) {
    mech_notify(mech, MECHALL,
                "Your lack of conciousness makes you fall to the ground. Not "
                "like you can read this anyway.");
    mech_fall(mech, 1, 0);
    dfa = 1;
    done = 1;
  } else {
    /* Handle DFA attack */
    if (condition.dfa_attacking) {
      /* is the target here? */
      target = btech_context_get_mech(context, mech_dfa_target_dbref(mech));
      if (target) {
        if (mech_position_x(target) == mech_position_x(mech) &&
            mech_position_y(target) == mech_position_y(mech))
          dfa = DeathFromAbove(mech, target);
        else
          mech_notify(mech, MECHPILOT, "Your DFA target has moved!");
      } else
        mech_notify(mech, MECHPILOT, "Your target has become invalid.");
    }

    if (!dfa)
      mech_notify(mech, MECHALL, "You finish your jump.");

    /* Better reset the FZ */
    mech_position_elevation_set(mech, map_elevation_get(mech_map,
                                                        mech_position_x(mech),
                                                        mech_position_y(mech)));
    mech_position_z_set(mech, mech_position_elevation(mech) - 1);
    mech_drop_surface_set(mech, true);

    if (condition.staggering) {
      mech_notify(mech, MECHALL,
                  "The damage you've taken makes the landing a bit harder...");

      if (!MadePilotSkillRoll(mech, calcStaggerBTHMod(mech))) {
        mech_notify(mech, MECHALL,
                    "... something you apparently can't handle!");
        mech_los_broadcast(mech, "lands, staggers, and falls down!");
        mech_fall(mech, 1, 0);
        return;
      }
    }

    /* Check piloting rolls, etc. */
    if (mech_class(mech) == CLASS_MECH) {
      if (CountDestroyedLegs(mech) > 0) {
        mech_notify(mech, MECHPILOT,
                    "Your missing leg makes it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your missing leg has caused you to fall upon landing!");
          mech_los_broadcast(mech, "lands, unbalanced, and falls down!");
          dfa = 1;
          mech_fall(mech, 1, 0);
          done = 1;
        }
      } else if (mech_section_base_to_hit(mech, RLEG) ||
                 mech_section_base_to_hit(mech, LLEG)) {
        mech_notify(mech, MECHPILOT,
                    "Your damaged leg actuators make it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your damaged leg actuators have caused you to fall upon "
                      "landing!");
          mech_los_broadcast(mech, "lands, stumbles, and falls down!");
          dfa = 1;
          done = 1;
          mech_fall(mech, 1, 0);
        }
      } else if (mech_has_damaged_gyro(mech)) {
        mech_notify(mech, MECHPILOT,
                    "Your damaged gyro makes it harder to land");
        if (!MadePilotSkillRoll(mech, 0)) {
          mech_notify(mech, MECHALL,
                      "Your damaged gyro has caused you to fall upon landing!");
          mech_los_broadcast(mech, "lands, twists awkwardly, and falls down!");
          dfa = 1;
          done = 1;
          mech_fall(mech, 1, 0);
        }
      }
    }
  }

  if (mech_class(mech) == CLASS_MECH && bsuit_swarmer_count(mech)) {
    mech_notify(mech, MECHALL,
                "The suits hanging off you make landing harder!");

    if (MadePilotSkillRoll(mech, 4)) {
      bsuit_swarmers_stop(
          btech_context_find_object(context, mech_map_dbref(mech)), mech, 0);
    } else {
      mech_notify(mech, MECHALL,
                  "You fail to properly control your unbalanced landing!");
      mech_los_broadcast(mech,
                         "lands and crashes to the ground from the weight "
                         "of the battlesuits!");
      mech_fall(mech, 1, 0);
    }
  }

  if (!dfa && !mech_is_fallen(mech) &&
      !mech_domino_resolve(mech, MECH_DOMINO_JUMP)) {
    if (mech_class(mech) != CLASS_VEH_GROUND)
      mech_los_broadcast(mech, "lands gracefully.");
    else
      mech_los_broadcast(mech, "returns to the ground where it belongs.");
  }

  /* If we aren't jumping anymore, we already took care of the event.
     (e.g. in mech_fall()) */
  if (mech_is_jumping(mech))
    mech_event_schedule(mech, EVENT_JUMPSTABIL, mech_stabilizing_event,
                        mech_jump_to_hit_recycle(mech), 0);
  mech_jump_complete(mech);
  mech_event_cancel(mech, EVENT_JUMP); /* Kill the event for moving 'round */
  mech_maybe_move(mech);               /* Possibly start movin' on da ground */

  if (!done)
    mine_field_trigger(mech, MINE_LAND);

  mech_flood(mech);
  mech_inferno_extinguish_in_water(mech);
  // this is only for non-new-stagger
  if (!btech_context_stagger_mode(mech_context(mech)))
    mech_stop_stagger_check(mech);
}
