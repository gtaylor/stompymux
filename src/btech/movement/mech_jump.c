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
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
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
#include "mech_notify.h"
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
#include "section_types.h"
#include "template_api.h"

static int mech_adjusted_jump_speed_mp(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);

  if (mech_is_under_gravity(mech) && map != nullptr)
    speed = speed * 100 / MAX(50, battle_map_gravity(map));
  return (int)(speed * MP_PER_KPH);
}

void mech_jump(DbRef player, void *data, char *buffer) {
  Mech *mech = (Mech *)data;
  Mech *tempMech = nullptr;
  BattleMap *mech_map;
  BtechContext *context = mech_context(mech);
  MechConditionSummary condition;
  char *args[3];
  int argc;
  int target;
  char targetID[2];
  short mapx, mapy;
  int bearing;
  float range = 0.0;
  float realx, realy;
  int sz, tz, jps;
  bool dfa_attack = false;

  mech_map = btech_context_get_map(context, mech_map_dbref(mech));
  cch(MECH_USUALO);
  condition = mech_condition_summary(mech);
  DOCHECK_CONTEXT(context, condition.fortified,
                  "Your fortified state prevents you from moving.");
#ifdef BT_MOVEMENT_MODES
  DOCHECK_CONTEXT(context, mech_move_mode_locked(mech),
                  "Movement modes disallow jumping.");
#endif
  DOCHECK_CONTEXT(context,
                  mech_class(mech) != CLASS_MECH &&
                      mech_class(mech) != CLASS_MW &&
                      mech_class(mech) != CLASS_BSUIT &&
                      mech_class(mech) != CLASS_VEH_GROUND,
                  "This unit cannot jump.");
  DOCHECK_CONTEXT(context, mech_carried_dbref(mech) > 0,
                  "You can't jump while towing someone!");
  DOCHECK_CONTEXT(context,
                  (mech_maximum_speed(mech) -
                   mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) >
                      MP1,
                  "No, with this cargo you won't!");
  DOCHECK_CONTEXT(context, mech_is_fallen(mech),
                  "You can't Jump from a FALLEN position");
  DOCHECK_CONTEXT(context, condition.hull_down,
                  "You can't Jump while hulldown");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_CHANGING_HULLDOWN),
                  "You are busy changing your hulldown mode");
  DOCHECK_CONTEXT(context, mech_is_jumping(mech), "You're already jumping!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_JUMPSTABIL),
                  "You haven't stabilized from your last jump yet.");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_STAND),
                  "You haven't finished standing up yet.");
  DOCHECK_CONTEXT(context, fabs(mech_jump_speed(mech)) <= 0.0,
                  "This mech doesn't have jump jets!");
  argc = mech_parseattributes(buffer, args, 3);
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_DUMP),
                  "You can not jump while dumping ammo!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_UNJAM_AMMO),
                  "You can not jump while unjamming your weapon!");
  DOCHECK_CONTEXT(context, mech_event_count(mech, EVENT_REMOVE_PODS),
                  "You are too busy removing iNARC pods!");
  DOCHECK_CONTEXT(context, battle_map_is_underground(mech_map),
                  "Realize the ceiling in this grotto is a bit to low for "
                  "that!");
  DOCHECK_CONTEXT(context, mech_is_out_of_control(mech),
                  "You can't jump while orbital dropping!");

  DOCHECK_CONTEXT(context, condition.swarm_target > 0,
                  "Perhaps you should dismount your ride first!");

  if (condition.staggering) {
    mech_notify(mech, MECHALL, "The damage inhibits your coordination...");

    if (!MadePilotSkillRoll(mech, calcStaggerBTHMod(mech))) {
      mech_notify(mech, MECHALL, "... something you apparently can't handle!");
      mech_los_broadcast(
          mech,
          "engages jumpjets, rolls to the side and slams into the ground!");
      mech_fall(mech, 1, 0);
      return;
    }
  }

  if (bsuit_jettison_validate(mech))
    return;

  DOCHECK_CONTEXT(context, argc > 2, "Too many arguments to JUMP function!");
  DOCHECK_CONTEXT(context, argc < 0,
                  "Invalid number of arguments to JUMP function!");
  mech_dfa_attacking_set(mech, false); /* By default no DFA */
  switch (argc) {
  case 0:
    /* DFA current target... */

    DOCHECK_CONTEXT(context, mech_class(mech) != CLASS_MECH,
                    "Only mechs can do Death From Above attacks!");

    target = mech_target_dbref(mech);
    tempMech = btech_context_get_mech(context, target);
    DOCHECK_CONTEXT(context, !tempMech, "Invalid Target!");
    range = mech_range_to(mech, tempMech);
    DOCHECK_CONTEXT(context,
                    !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
                    "Target is not in line of sight!");
    DOCHECK_CONTEXT(context, mech_class(tempMech) == CLASS_MW,
                    "Even you can't aim your jump well enough to squish that!");
    mapx = mech_position_x(tempMech);
    mapy = mech_position_y(tempMech);
    mech_dfa_target_dbref_set(mech, mech_target_dbref(mech));
    break;
  case 1:
    /* Jump Target */
    DOCHECK_CONTEXT(context, mech_class(mech) != CLASS_MECH,
                    "Only mechs can do Death From Above attacks!");

    targetID[0] = args[0][0];
    targetID[1] = args[0][1];
    target = FindTargetDBREFFromMapNumber(mech, targetID);
    tempMech = btech_context_get_mech(context, target);
    DOCHECK_CONTEXT(context, !tempMech, "Target is not in line of sight!");
    range = mech_range_to(mech, tempMech);
    DOCHECK_CONTEXT(context,
                    !mech_los_check(mech, tempMech, mech_position_x(tempMech),
                                    mech_position_y(tempMech), range),
                    "Target is not in line of sight!");
    DOCHECK_CONTEXT(context, mech_class(tempMech) == CLASS_MW,
                    "Even you can't aim your jump well enough to squish that!");
    mapx = mech_position_x(tempMech);
    mapy = mech_position_y(tempMech);
    mech_dfa_target_dbref_set(mech, mech_dbref(tempMech));
    break;
  case 2:
    bearing = atoi(args[0]);
    range = atof(args[1]);
    FindXY(mech_position_real_x(mech), mech_position_real_y(mech), bearing,
           range, &realx, &realy);

    /* This is so we are jumping to the center of a hex */
    /* and the bearing jives with the target hex */
    RealCoordToMapCoord(&mapx, &mapy, realx, realy);
    break;
  }
  DOCHECK_CONTEXT(context,
                  !battle_map_coordinate_is_valid(mech_map, mapx, mapy),
                  "That would take you off the map!");
  DOCHECK_CONTEXT(
      context, mech_position_x(mech) == mapx && mech_position_y(mech) == mapy,
      "You're already in the target hex.");
  sz = mech_position_z(mech);
  if (map_real_terrain_get(mech_map, mapx, mapy) == BATTLE_TERRAIN_ICE)
    tz = 0;
  else
    tz = battle_map_hex_elevation(mech_map, mapx, mapy);
  jps = mech_adjusted_jump_speed_mp(mech, mech_map);
  DOCHECK_CONTEXT(context, range > jps, "That target is out of range!");
  dfa_attack = mech_class(mech) != CLASS_BSUIT && tempMech;
  if (dfa_attack)
    mech_dfa_attacking_set(mech, true);
  /* New idea: JumpTop = (JP + 1 - range / 3) - in another words,
     SDR jumping for 1 hexes has 8 + 1 = 9 hex elevation in mid-flight,
     SDR jumping for 8 hexes has 8 + 1 - 2 = 7 hex elevation in mid-flight,
     TDR jumping for 4 hexes has 4 + 1 - 1 = 4 hex elevation in mid-flight

     Come to think of it, the last SDR figure was ridiculous. New
     value: 2 * 1 + 2 = 4
   */
  mech_jump_apex_elevation_set(mech, MIN(jps + 1 - range / 3, 2 * range + 2));
  DOCHECK_CONTEXT(context, (tz - sz) > jps,
                  "That target's high for you to reach with a single jump!");
  DOCHECK_CONTEXT(context, (sz - tz) > jps,
                  "That target's low for you to reach with a single jump!");
  DOCHECK_CONTEXT(context, sz < -1, "Glub glub glub.");
  MapCoordToRealCoord(mapx, mapy, &realx, &realy);
  bearing = FindBearing(mech_position_real_x(mech), mech_position_real_y(mech),
                        realx, realy);

  /* TAKE OFF! */
  const MechJumpLaunch launch = {
      .heading = bearing,
      .destination_x = mapx,
      .destination_y = mapy,
      .destination_elevation = tz,
      .apex_elevation = MIN(jps + 1 - range / 3, 2 * range + 2),
      .distance =
          length_hypotenuse((double)(realx - mech_position_real_x(mech)),
                            (double)(realy - mech_position_real_y(mech))),
  };
  mech_jump_launch(mech, &launch);
  if (dfa_attack)
    mech_notify(mech, MECHALL,
                "You engage your jump jets for a Death From Above attack!");
  else
    mech_notify(mech, MECHALL, "You engage your jump jets.");
  mech_los_broadcast(mech, "engages jumpjets!");
  mech_event_schedule(mech, EVENT_JUMP, mech_jump_event, JUMP_TICK, 0);
}
