/* Implements BattleTech movement mechanics for unit jump. */

#include <math.h>
#include <stdlib.h>

#include "aero_move_api.h"
#include "bsuit_api.h"
#include "btconfig.h"
#include "btech/context.h"
#include "btech_channel.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "btmux_build_config.h"
#include "checked_conversion.h"
#include "command_handlers_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_terrain.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_events.h"
#include "mech_events_api.h"
#include "mech_fire_api.h"
#include "mech_identity_api.h"
#include "mech_lifecycle.h"
#include "mech_los_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_sensor_state_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_targeting_api.h"
#include "mech_utils_api.h"
#include "mine_api.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/stringutil.h"
#include "registry_api.h"
#include "section_types.h"
#include "template_api.h"

static int mech_adjusted_jump_speed_mp(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);

  if (mech_is_under_gravity(mech) && map != nullptr) {
    const int gravity = MAX(50, battle_map_gravity(map));
    speed = speed * 100.0F / (float)gravity;
  }
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
  DbRef target;
  char targetID[2];
  short mapx, mapy;
  int bearing;
  float range = 0.0;
  float realx, realy;
  int sz, tz, jps;
  bool dfa_attack = false;

  mech_map = btech_context_get_map(context, mech_map_dbref(mech));
  if (!common_checks(player, mech, MECH_USUALO))
    return;
  condition = mech_condition_summary(mech);
  if (condition.fortified) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Your fortified state prevents you from moving.");
    return;
  }
#ifdef BT_MOVEMENT_MODES
  if (mech_move_mode_locked(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Movement modes disallow jumping.");
    return;
  }
#endif
  if (mech_class(mech) != CLASS_MECH && mech_class(mech) != CLASS_MW &&
      mech_class(mech) != CLASS_BSUIT && mech_class(mech) != CLASS_VEH_GROUND) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This unit cannot jump.");
    return;
  }
  if (mech_carried_dbref(mech) > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't jump while towing someone!");
    return;
  }
  if ((mech_maximum_speed(mech) -
       mech_cargo_maximum_speed(mech, mech_maximum_speed(mech))) > MP1) {
    mecha_notify(btech_context_evaluation(context), player,
                 "No, with this cargo you won't!");
    return;
  }
  if (mech_is_fallen(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't Jump from a FALLEN position");
    return;
  }
  if (condition.hull_down) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't Jump while hulldown");
    return;
  }
  if (mech_event_count(mech, EVENT_CHANGING_HULLDOWN)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are busy changing your hulldown mode");
    return;
  }
  if (mech_is_jumping(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're already jumping!");
    return;
  }
  if (mech_event_count(mech, EVENT_JUMPSTABIL)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You haven't stabilized from your last jump yet.");
    return;
  }
  if (mech_event_count(mech, EVENT_STAND)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You haven't finished standing up yet.");
    return;
  }
  if (fabsf(mech_jump_speed(mech)) <= 0.0F) {
    mecha_notify(btech_context_evaluation(context), player,
                 "This mech doesn't have jump jets!");
    return;
  }
  argc = mech_parseattributes(buffer, args, 3);
  if (mech_event_count(mech, EVENT_DUMP)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not jump while dumping ammo!");
    return;
  }
  if (mech_event_count(mech, EVENT_UNJAM_AMMO)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can not jump while unjamming your weapon!");
    return;
  }
  if (mech_event_count(mech, EVENT_REMOVE_PODS)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You are too busy removing iNARC pods!");
    return;
  }
  if (battle_map_is_underground(mech_map)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Realize the ceiling in this grotto is a bit to low for "
                 "that!");
    return;
  }
  if (mech_is_out_of_control(mech)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You can't jump while orbital dropping!");
    return;
  }

  if (condition.swarm_target > 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Perhaps you should dismount your ride first!");
    return;
  }

  if (condition.staggering) {
    mech_notify(mech, MECHALL, "The damage inhibits your coordination...");

    if (!MadePilotSkillRoll(mech, mech_stagger_modifier(mech))) {
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

  if (argc > 2) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Too many arguments to JUMP function!");
    return;
  }
  if (argc < 0) {
    mecha_notify(btech_context_evaluation(context), player,
                 "Invalid number of arguments to JUMP function!");
    return;
  }
  mech_dfa_attacking_set(mech, false); /* By default no DFA */
  switch (argc) {
  case 0:
    /* DFA current target... */

    if (mech_class(mech) != CLASS_MECH) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Only mechs can do Death From Above attacks!");
      return;
    }

    target = mech_target_dbref(mech);
    tempMech = btech_context_get_mech(context, target);
    if (!tempMech) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Invalid Target!");
      return;
    }
    range = mech_range_to(mech, tempMech);
    if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                        mech_position_y(tempMech), range)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Target is not in line of sight!");
      return;
    }
    if (mech_class(tempMech) == CLASS_MW) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Even you can't aim your jump well enough to squish that!");
      return;
    }
    mapx = clamp_int_to_short(mech_position_x(tempMech));
    mapy = clamp_int_to_short(mech_position_y(tempMech));
    mech_dfa_target_dbref_set(mech, mech_target_dbref(mech));
    break;
  case 1:
    /* Jump Target */
    if (mech_class(mech) != CLASS_MECH) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Only mechs can do Death From Above attacks!");
      return;
    }

    char **target_argument_slot = checked_storage_at(args, 3, sizeof(*args), 0);
    targetID[0] = *checked_string_suffix(*target_argument_slot, 0);
    targetID[1] = *checked_string_suffix(*target_argument_slot, 1);
    target = FindTargetDBREFFromMapNumber(mech, targetID);
    tempMech = btech_context_get_mech(context, target);
    if (!tempMech) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Target is not in line of sight!");
      return;
    }
    range = mech_range_to(mech, tempMech);
    if (!mech_los_check(mech, tempMech, mech_position_x(tempMech),
                        mech_position_y(tempMech), range)) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Target is not in line of sight!");
      return;
    }
    if (mech_class(tempMech) == CLASS_MW) {
      mecha_notify(btech_context_evaluation(context), player,
                   "Even you can't aim your jump well enough to squish that!");
      return;
    }
    mapx = clamp_int_to_short(mech_position_x(tempMech));
    mapy = clamp_int_to_short(mech_position_y(tempMech));
    mech_dfa_target_dbref_set(mech, mech_dbref(tempMech));
    break;
  case 2:
    if (!parse_int_checked(args[0], &bearing) ||
        !parse_float_checked(args[1], &range)) {
      mecha_notify(btech_context_evaluation(mech_context(mech)), player,
                   "Invalid jump coordinates!");
      return;
    }
    FindXY(mech_position_real_x(mech), mech_position_real_y(mech), bearing,
           range, &realx, &realy);

    /* This is so we are jumping to the center of a hex */
    /* and the bearing jives with the target hex */
    RealCoordToMapCoord(&mapx, &mapy, realx, realy);
    break;
  }
  if (!battle_map_coordinate_is_valid(mech_map, mapx, mapy)) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That would take you off the map!");
    return;
  }
  if (mech_position_x(mech) == mapx && mech_position_y(mech) == mapy) {
    mecha_notify(btech_context_evaluation(context), player,
                 "You're already in the target hex.");
    return;
  }
  sz = mech_position_z(mech);
  if (map_real_terrain_get(mech_map, mapx, mapy) == BATTLE_TERRAIN_ICE)
    tz = 0;
  else
    tz = battle_map_hex_elevation(mech_map, mapx, mapy);
  jps = mech_adjusted_jump_speed_mp(mech, mech_map);
  if (range > (float)jps) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target is out of range!");
    return;
  }
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
  const float apex_candidate =
      fminf((float)jps + 1.0F - range / 3.0F, 2.0F * range + 2.0F);
  const int apex_elevation = (int)apex_candidate;
  mech_jump_apex_elevation_set(mech, apex_elevation);
  if ((tz - sz) > jps) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target's high for you to reach with a single jump!");
    return;
  }
  if ((sz - tz) > jps) {
    mecha_notify(btech_context_evaluation(context), player,
                 "That target's low for you to reach with a single jump!");
    return;
  }
  if (sz < -1) {
    mecha_notify(btech_context_evaluation(context), player, "Glub glub glub.");
    return;
  }
  MapCoordToRealCoord(mapx, mapy, &realx, &realy);
  bearing = FindBearing(mech_position_real_x(mech), mech_position_real_y(mech),
                        realx, realy);

  /* TAKE OFF! */
  const double jump_distance =
      length_hypotenuse((double)(realx - mech_position_real_x(mech)),
                        (double)(realy - mech_position_real_y(mech)));
  MechJumpLaunch launch = {
      .heading = bearing,
      .destination_x = mapx,
      .destination_y = mapy,
      .destination_elevation = tz,
      .apex_elevation = apex_elevation,
      .distance = (float)jump_distance,
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
