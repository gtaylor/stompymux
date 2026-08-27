/* Implements vehicle-specific BattleTech hex transition mechanics. */

#include "checked_conversion.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_combat_misc_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_hex_transition_api.h"
#include "mech_lifecycle.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_status_types.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "section_types.h"

#include <math.h>
#include <stdlib.h>

static int mech_hex_maximum_int(int first, int second) {
  return first > second ? first : second;
}

static bool mech_passes_cliff_check(Mech *mech, bool skid_cliff) {
  int modifier =
      skid_cliff
          ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
          : clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) / 3;
  return mech_pilot_skill_roll_without_experience(&(PilotSkillRollRequest){
      .mech = mech, .modifier = modifier, .succeed_when_fallen = true});
}

static bool
tracked_hex_transition_resolve(const HexVehicleTransitionInput *input) {
  Mech *mech = input->mech;
  float deltax = input->delta_x;
  const float DELTAY = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  bool skid_cliff = input->skid_cliff;
  bool roll_on_backwalk = input->roll_on_backwalk;
  bool new_terrain = input->new_terrain;
  int avoidbth;
  int tt;
  int le;
  float f;

  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_WALL,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL,
                "You attempt to climb a hill too steep for you.");
    if (mech_pilot_dbref(mech) == -1 ||
        mech_passes_cliff_check(mech, skid_cliff)) {
      mech_notify(mech, MECHALL, "You manage to stop before crashing.");
      mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");
    } else {
      if (!skid_cliff) {
        mech_notify(mech, MECHALL, "You smash into a cliff!");
        mech_los_broadcast(mech, "crashes to a cliff!");
        mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4),
                  false);
      } else {
        mech_notify(mech, MECHALL, "You skid to a violent halt!");
        mech_los_broadcast(mech, "goes into a skid!");
        mech_fall(mech, 0, false);
      }
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_DROP,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL, "You notice a large drop in front of you");
    avoidbth =
        skid_cliff
            ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
            : clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                  3;
    if (mech_pilot_dbref(mech) == -1 ||
        (!mech_condition_summary(mech).auto_fall &&
         mech_pilot_skill_roll_without_experience(
             &(PilotSkillRollRequest){.mech = mech,
                                      .modifier = avoidbth,
                                      .succeed_when_fallen = true}))) {
      mech_notify(mech, MECHALL, "You manage to stop before falling off.");
      mech_los_broadcast(mech, "stops suddenly to avoid falling off a cliff!");
    } else {
      mech_notify(mech, MECHALL,
                  "You drive off the cliff and fall to the ground below.");
      mech_los_broadcast(mech,
                         "drives off a cliff and falls to the ground below.");
      mech_fall(mech, lastelevation - elevation, false);
      mech_domino_resolve(mech, MECH_DOMINO_FALL);
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
          !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        mech_destroy(mech, mech, false, KILL_TYPE_FLOOD);
      }
      return true;
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
      (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_BACK,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain}))) {
    mech_printf(mech, MECHALL, "You notice a %s behind you!",
                (elevation > lastelevation ? "small incline" : "small drop"));
    if (mech_pilot_dbref(mech) == -1 ||
        (made_pilot_skill_roll(mech, collision_check(&(MovementCollisionCheck){
                                         .mech = mech,
                                         .mode = WALK_BACK,
                                         .previous_elevation = lastelevation,
                                         .previous_terrain = oldterrain}) -
                                         1))) {
      mech_notify(mech, MECHALL, "You manage to overcome the obstacle.");
    } else {
      mech_printf(mech, MECHALL, "%s",
                  (elevation > lastelevation
                       ? "You stumble on your rear and fall down."
                       : "You fall on your rear off the small incline."));
      mech_los_broadcast(mech, elevation > lastelevation
                                   ? "falls on its back walking up an incline."
                                   : "falls off the back of a small incline.");
      mech_fall(mech, abs(lastelevation - elevation), true);
      mech_movement_stop(mech);
      if (elevation > lastelevation) {
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = DELTAY},
                                    .previous_z = lastelevation});
      }
    }
    return true;
  }
  if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
      (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
       (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
        (lastelevation < (elevation - 1)))) &&
      elevation < 0) {
    mech_notify(mech, MECHALL, "You notice a body of water in front of you");
    if (mech_pilot_dbref(mech) == -1 ||
        made_pilot_skill_roll(
            mech,
            clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                3)) {
      mech_notify(mech, MECHALL, "You manage to stop before falling in.");
      mech_los_broadcast(mech,
                         "stops suddenly to avoid driving into the water!");
    } else {
      mech_notify(
          mech, MECHALL,
          "You drive into the water and your vehicle becomes inoperable.");
      mech_destroy(mech, mech, false, KILL_TYPE_FLOOD);
      return true;
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  /* New terrain restrictions */
  if (new_terrain) {
    tt = (unsigned char)mech_real_terrain_get(mech);
    if ((tt == BATTLE_TERRAIN_HEAVY_FOREST) &&
        fabsf(mech_current_speed(mech)) > MP1) {
      mech_notify(mech, MECHALL, "You try to dodge the larger trees..");
      if (mech_pilot_dbref(mech) == -1 ||
          made_pilot_skill_roll(
              mech, clamp_float_to_int(fabsf(mech_current_speed(mech)) / MP1 /
                                       6.0F))) {
        mech_notify(mech, MECHALL, "You manage to dodge 'em!");
      } else {
        mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
        mech_los_broadcast(mech, "cruises headlong at a tree!");
        f = fabsf(mech_current_speed(mech));
        mech_current_speed_scale(mech, 0.5F);
        mech_fall(
            mech,
            mech_hex_maximum_int(1, clamp_float_to_int(sqrtf(f / MP1 / 2.0F))),
            false);
      }
    }
  }
  /* Slow them if they made an elevation change */
  le = elevation - lastelevation;
  le = (le < 0) ? -le : le;
  if (le > 0) {
    deltax = (le == 1) ? MP2 : MP3;
    mech_current_speed_reduce_toward_zero(mech, deltax);
  }

  return false;
}

static bool
wheeled_hex_transition_resolve(const HexVehicleTransitionInput *input) {
  Mech *mech = input->mech;
  float deltax = input->delta_x;
  const float DELTAY = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  bool skid_cliff = input->skid_cliff;
  bool roll_on_backwalk = input->roll_on_backwalk;
  bool new_terrain = input->new_terrain;
  int avoidbth;
  int tt;
  int le;
  float f;

  /* Cliff ! */
  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_WALL,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL,
                "You attempt to climb a hill too steep for you.");
    if (mech_pilot_dbref(mech) == -1 ||
        mech_passes_cliff_check(mech, skid_cliff)) {
      mech_notify(mech, MECHALL, "You manage to stop before crashing.");
      mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");
    } else {
      if (!skid_cliff) {
        mech_notify(mech, MECHALL, "You smash into a cliff!");
        mech_los_broadcast(mech, "crashes to a cliff!");
        mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4),
                  false);
      } else {
        mech_notify(mech, MECHALL, "You skid to a violent halt!");
        mech_los_broadcast(mech, "skids to a halt!");
        mech_fall(mech, 0, false);
      }
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_DROP,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL, "You notice a large drop in front of you");
    avoidbth =
        skid_cliff
            ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
            : clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                  3;
    if (mech_pilot_dbref(mech) == -1 ||
        (!mech_condition_summary(mech).auto_fall &&
         mech_pilot_skill_roll_without_experience(
             &(PilotSkillRollRequest){.mech = mech,
                                      .modifier = avoidbth,
                                      .succeed_when_fallen = true}))) {
      mech_notify(mech, MECHALL, "You manage to stop before falling off.");
      mech_los_broadcast(mech, "stops suddenly to avoid driving off a cliff!");
    } else {
      mech_notify(mech, MECHALL,
                  "You drive off the cliff and fall to the ground below.");
      mech_los_broadcast(mech,
                         "drives off a cliff and falls to the ground below.");
      mech_fall(mech, lastelevation - elevation, false);
      mech_domino_resolve(mech, MECH_DOMINO_FALL);
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
          !(mech_technology_flags_secondary(mech) & WATERPROOF_TECH)) {
        mech_notify(
            mech, MECHALL,
            "You drive into the water and your vehicle becomes inoperable.");
        mech_destroy(mech, mech, false, KILL_TYPE_FLOOD);
      }
      return true;
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
      (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_BACK,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain}))) {
    mech_printf(mech, MECHALL, "You notice a %s behind you!",
                (elevation > lastelevation ? "small incline" : "small drop"));
    if (mech_pilot_dbref(mech) == -1 ||
        (made_pilot_skill_roll(mech, collision_check(&(MovementCollisionCheck){
                                         .mech = mech,
                                         .mode = WALK_BACK,
                                         .previous_elevation = lastelevation,
                                         .previous_terrain = oldterrain}) -
                                         1))) {
      mech_notify(mech, MECHALL, "You manage to overcome the obstacle.");
    } else {
      mech_printf(mech, MECHALL, "%s",
                  (elevation > lastelevation
                       ? "You stumble on your rear and fall down."
                       : "You fall on your rear off the small incline."));
      mech_los_broadcast(mech, elevation > lastelevation
                                   ? "falls on its back walking up an incline."
                                   : "falls off the back of a small incline.");
      mech_fall(mech, abs(lastelevation - elevation), true);
      mech_movement_stop(mech);
      if (elevation > lastelevation) {
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = DELTAY},
                                    .previous_z = lastelevation});
      }
    }
    return true;
  }
  if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
      (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
       (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
        (lastelevation < (elevation - 1)))) &&
      elevation < 0) {
    mech_notify(mech, MECHALL, "You notice a body of water in front of you");
    if (mech_pilot_dbref(mech) == -1 ||
        made_pilot_skill_roll(
            mech,
            clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                3)) {
      mech_notify(mech, MECHALL, "You manage to stop before falling in.");
      mech_los_broadcast(mech, "stops suddenly to driving into the water!");
    } else {
      mech_notify(
          mech, MECHALL,
          "You drive into the water and your vehicle becomes inoperable.");
      mech_destroy(mech, mech, false, KILL_TYPE_FLOOD);
      return true;
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  /* New terrain restrictions */
  if (new_terrain) {
    tt = (unsigned char)mech_real_terrain_get(mech);
    if ((tt == BATTLE_TERRAIN_HEAVY_FOREST ||
         tt == BATTLE_TERRAIN_LIGHT_FOREST) &&
        fabsf(mech_current_speed(mech)) > MP1) {
      mech_notify(mech, MECHALL, "You try to dodge the larger trees..");
      if (mech_pilot_dbref(mech) == -1 ||
          made_pilot_skill_roll(
              mech, (tt == BATTLE_TERRAIN_HEAVY_FOREST ? 3 : 0) +
                        clamp_float_to_int(fabsf(mech_current_speed(mech)) /
                                           MP1 / 6.0F))) {
        mech_notify(mech, MECHALL, "You manage to dodge 'em!");
      } else {
        mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
        mech_los_broadcast(mech, "cruises headlong at a tree!");
        f = fabsf(mech_current_speed(mech));
        mech_current_speed_scale(mech, 0.5F);
        mech_fall(
            mech,
            mech_hex_maximum_int(1, clamp_float_to_int(sqrtf(f / MP1 / 2.0F))),
            false);
      }
    } else if ((tt == BATTLE_TERRAIN_ROUGH) &&
               fabsf(mech_current_speed(mech)) > MP1) {
      mech_notify(mech, MECHALL, "You try to avoid the rocks..");
      if (mech_pilot_dbref(mech) == -1 ||
          made_pilot_skill_roll(
              mech, clamp_float_to_int(fabsf(mech_current_speed(mech)) / MP1 /
                                       6.0F))) {
        mech_notify(mech, MECHALL, "You manage to dodge 'em!");
      } else {
        mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
        mech_los_broadcast(mech, "cruises headlong at a rock!");
        f = fabsf(mech_current_speed(mech));
        mech_current_speed_scale(mech, 0.5F);
        mech_fall(
            mech,
            mech_hex_maximum_int(1, clamp_float_to_int(sqrtf(f / MP1 / 2.0F))),
            false);
      }
    }
  }
  /* Slow them down if they change elevations */
  le = elevation - lastelevation;
  le = (le < 0) ? -le : le;
  if (le > 0) {
    deltax = (le == 1) ? MP2 : MP3;
    mech_current_speed_reduce_toward_zero(mech, deltax);
  }

  return false;
}

static bool
naval_hex_transition_resolve(const HexVehicleTransitionInput *input) {
  Mech *mech = input->mech;
  float deltax = input->delta_x;
  const float DELTAY = input->delta_y;
  int lastelevation = input->last_elevation;

  if ((mech_real_terrain_get(mech) != BATTLE_TERRAIN_WATER &&
       mech_real_terrain_get(mech) != BATTLE_TERRAIN_BRIDGE) ||
      abs(mech_position_elevation(mech)) <
          (abs(mech_position_z(mech)) +
           (mech_movement_type(mech) == MOVE_FOIL ? -1 : 0))) {
    /* Run aground */
    mech_notify(mech, MECHALL, "You attempt to get too close with ground!");
    if (mech_pilot_dbref(mech) == -1 ||
        made_pilot_skill_roll(
            mech,
            clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                3)) {
      mech_notify(mech, MECHALL, "You manage to stop before crashing.");
      mech_los_broadcast(mech, "stops suddenly to avoid running aground!");
      mech_position_rollback(
          &(MechPositionRollback){.mech = mech,
                                  .delta = {.x = deltax, .y = DELTAY},
                                  .previous_z = lastelevation});
    } else {
      mech_notify(mech, MECHALL, "You smash into the ground!");
      mech_los_broadcast(mech, "smashes aground!");
      mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4), false);
    }
    mech_movement_stop(mech);
    mech_vertical_speed_set(mech, 0.0F);
    return true;
  }

  return false;
}

static bool
hover_hex_transition_resolve(const HexVehicleTransitionInput *input) {
  Mech *mech = input->mech;
  float deltax = input->delta_x;
  const float DELTAY = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  bool skid_cliff = input->skid_cliff;
  bool roll_on_backwalk = input->roll_on_backwalk;
  constexpr int IS_UNDER = 0;
  int avoidbth;
  int tt;
  int le;
  float f;

  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_WALL,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL,
                "You attempt to climb a hill too steep for you.");
    if (mech_pilot_dbref(mech) == -1 ||
        mech_passes_cliff_check(mech, skid_cliff)) {
      mech_notify(mech, MECHALL, "You manage to stop before crashing.");
      mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");
    } else {
      if (!skid_cliff) {
        mech_notify(mech, MECHALL, "You smash into a cliff!");
        mech_los_broadcast(mech, "smashes into a cliff!");
        mech_fall(mech, (int)(mech_current_speed(mech) * MP_PER_KPH / 4),
                  false);
      } else {
        mech_notify(mech, MECHALL, "You skid to a violent halt!");
        mech_los_broadcast(mech, "skids to a halt!");
        mech_fall(mech, 0, false);
      }
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_DROP,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL, "You notice a large drop in front of you");
    avoidbth =
        skid_cliff
            ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
            : clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                  3;
    if (mech_pilot_dbref(mech) == -1 ||
        (!mech_condition_summary(mech).auto_fall &&
         mech_pilot_skill_roll_without_experience(
             &(PilotSkillRollRequest){.mech = mech,
                                      .modifier = avoidbth,
                                      .succeed_when_fallen = true}))) {
      mech_notify(mech, MECHALL, "You manage to stop before falling off.");
      mech_los_broadcast(mech, "stops suddenly to avoid falling off a cliff!");
    } else {
      mech_notify(mech, MECHALL,
                  "You drive off the cliff and fall to the ground below.");
      mech_los_broadcast(mech,
                         "drives off a cliff and falls to the ground below.");
      mech_fall(mech, lastelevation - elevation, false);
      return true;
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = HIT_UNDER_BRIDGE,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) {
    mech_notify(mech, MECHALL,
                "You notice the underside of the bridge in front of you!");
    if (mech_pilot_dbref(mech) == -1 ||
        (!skid_cliff &&
         made_pilot_skill_roll(
             mech,
             clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) /
                 3)) ||
        (skid_cliff && made_pilot_skill_roll(
                           mech, mech_skid_modifier(
                                     fabsf(mech_current_speed(mech)) / MP1)))) {
      mech_notify(mech, MECHALL,
                  "You manage to stop before slamming into the bridge.");
      mech_los_broadcast(mech,
                         "stops suddenly to avoid slamming into the bridge!");
    } else {
      mech_notify(mech, MECHALL,
                  "You drive right into the underside of the bridge.");
      mech_los_broadcast(mech,
                         "drives right into the underside of the bridge.");
      mech_fall(mech, 1, false);
    }
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_movement_stop(mech);
    return true;
  }
  if (roll_on_backwalk && (mech_current_speed(mech) < 0) &&
      (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_BACK,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain})) &&
      !IS_UNDER) {
    mech_printf(mech, MECHALL, "You notice a %s behind you!",
                (elevation > lastelevation ? "small incline" : "small drop"));
    if (mech_pilot_dbref(mech) == -1 ||
        (made_pilot_skill_roll(mech, collision_check(&(MovementCollisionCheck){
                                         .mech = mech,
                                         .mode = WALK_BACK,
                                         .previous_elevation = lastelevation,
                                         .previous_terrain = oldterrain}) -
                                         1))) {
      mech_notify(mech, MECHALL, "You manage to overcome the obstacle.");
    } else {
      mech_printf(mech, MECHALL, "%s",
                  (elevation > lastelevation
                       ? "You stumble on your rear and fall down."
                       : "You fall on your rear off the small incline."));
      mech_los_broadcast(mech, elevation > lastelevation
                                   ? "falls on its back walking up an incline."
                                   : "falls off the back of a small incline.");
      mech_fall(mech, abs(lastelevation - elevation), true);
      mech_movement_stop(mech);
      if (elevation > lastelevation) {
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = DELTAY},
                                    .previous_z = lastelevation});
      }
    }
    return true;
  }
  tt = (unsigned char)mech_real_terrain_get(mech);
  if ((tt == BATTLE_TERRAIN_HEAVY_FOREST ||
       tt == BATTLE_TERRAIN_LIGHT_FOREST) &&
      fabsf(mech_current_speed(mech)) > MP1) {
    mech_notify(mech, MECHALL, "You try to dodge the larger trees..");
    if (mech_pilot_dbref(mech) == -1 ||
        made_pilot_skill_roll(
            mech, (tt == BATTLE_TERRAIN_HEAVY_FOREST ? 3 : 0) +
                      clamp_float_to_int(fabsf(mech_current_speed(mech)) / MP1 /
                                         6.0F))) {
      mech_notify(mech, MECHALL, "You manage to dodge 'em!");
    } else {
      mech_notify(mech, MECHALL, "You swerve, but not enough! This'll hurt!");
      mech_los_broadcast(mech, "cruises headlong at a tree!");
      f = fabsf(mech_current_speed(mech));
      mech_current_speed_scale(mech, 0.5F);
      mech_fall(
          mech,
          mech_hex_maximum_int(1, clamp_float_to_int(sqrtf(f / MP1 / 2.0F))),
          false);
    }
  }
  /* Slow the unit down if its made an elevation change */
  le = elevation - lastelevation;
  le = (le < 0) ? -le : le;
  if (le > 0) {
    deltax = (le == 1) ? MP2 : MP3;
    mech_current_speed_reduce_toward_zero(mech, deltax);
  }

  return false;
}

static bool
flight_hex_transition_resolve(const HexVehicleTransitionInput *input) {
  Mech *mech = input->mech;
  float deltax = input->delta_x;
  const float DELTAY = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  bool roll_on_backwalk = input->roll_on_backwalk;
  int ot = input->old_terrain;

  if ((mech_is_landed(mech) &&
       mech_real_terrain_get(mech) != BATTLE_TERRAIN_ROAD &&
       mech_real_terrain_get(mech) != BATTLE_TERRAIN_BRIDGE &&
       mech_real_terrain_get(mech) != BATTLE_TERRAIN_GRASSLAND &&
       mech_real_terrain_get(mech) != BATTLE_TERRAIN_BUILDING) ||
      (battle_terrain_is_forest(mech_real_terrain_get(mech)) &&
       mech_position_z(mech) < (mech_position_surface_elevation(mech) + 2))) {
    mech_notify(mech, MECHALL,
                "You go where no flying thing has ever gone before..");
    if (mech_has_active_pilot(mech) && made_pilot_skill_roll(mech, 5)) {
      mech_notify(mech, MECHALL, "You stop in time!");
      mech_position_rollback(
          &(MechPositionRollback){.mech = mech,
                                  .delta = {.x = deltax, .y = DELTAY},
                                  .previous_z = lastelevation});
    } else {
      mech_notify(mech, MECHALL, "Eww.. You've a bad feeling about this.");
      mech_los_broadcast(mech, "crashes!");
      mech_fall(mech, 1, false);
    }
    mech_movement_stop(mech);
    return true;
  }
  if (mech_is_landed(mech) && roll_on_backwalk &&
      (mech_current_speed(mech) < 0) &&
      (collision_check(
          &(MovementCollisionCheck){.mech = mech,
                                    .mode = WALK_BACK,
                                    .previous_elevation = lastelevation,
                                    .previous_terrain = oldterrain}))) {
    mech_printf(mech, MECHALL, "You notice a %s behind you!",
                (elevation > lastelevation ? "small incline" : "small drop"));
    if (mech_pilot_dbref(mech) == -1 ||
        (made_pilot_skill_roll(mech, collision_check(&(MovementCollisionCheck){
                                         .mech = mech,
                                         .mode = WALK_BACK,
                                         .previous_elevation = lastelevation,
                                         .previous_terrain = oldterrain}) -
                                         1))) {
      mech_notify(mech, MECHALL, "You manage to overcome the obstacle.");
    } else {
      mech_printf(mech, MECHALL, "%s",
                  (elevation > lastelevation
                       ? "You stumble on your rear and fall down."
                       : "You fall on your rear off the small incline."));
      mech_los_broadcast(mech, elevation > lastelevation
                                   ? "falls on its back walking up an incline."
                                   : "falls off the back of a small incline.");
      mech_fall(mech, (abs(lastelevation - elevation) + 1000), true);
      mech_movement_stop(mech);
      if (elevation > lastelevation) {
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = DELTAY},
                                    .previous_z = lastelevation});
      }
    }
    return true;
  }
  if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER)
    return true;
  if (collision_check(&(MovementCollisionCheck){.mech = mech,
                                                .mode = JUMP,
                                                .previous_elevation = 0,
                                                .previous_terrain = 0})) {
    mech_position_rollback(
        &(MechPositionRollback){.mech = mech,
                                .delta = {.x = deltax, .y = DELTAY},
                                .previous_z = lastelevation});
    mech_notify(mech, MECHALL,
                "You attempt to fly over elevation that is too high!");
    if (mech_pilot_dbref(mech) == -1 ||
        (made_pilot_skill_roll(
             mech, (int)(mech_position_real_z(mech) / ZSCALE / 3)) &&
         (ot == BATTLE_TERRAIN_GRASSLAND || ot == BATTLE_TERRAIN_ROAD ||
          ot == BATTLE_TERRAIN_BUILDING))) {
      mech_notify(mech, MECHALL, "You land safely.");
      mech_landed_set(mech, true);
      mech_current_speed_set(mech, 0.0F);
      mech_vertical_speed_set(mech, 0.0F);
    } else {
      mech_notify(mech, MECHALL,
                  "You crash into the obstacle and fall from the sky!");
      mech_los_broadcast(mech,
                         "crashes into an obstacle and falls from the sky!");
      mech_fall(mech, mech_drop_height_above_surface(mech) + 1, false);
      mech_domino_resolve(mech, MECH_DOMINO_FALL);
    }
  }

  return false;
}

bool mech_vehicle_hex_transition_resolve(
    const HexVehicleTransitionInput *input) {
  switch (mech_movement_type(input->mech)) {
  case MOVE_TRACK:
    return tracked_hex_transition_resolve(input);
  case MOVE_WHEEL:
    return wheeled_hex_transition_resolve(input);
  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_SUB:
    return naval_hex_transition_resolve(input);
  case MOVE_HOVER:
    return hover_hex_transition_resolve(input);
  case MOVE_VTOL:
  case MOVE_FLY:
    return flight_hex_transition_resolve(input);
  case MOVE_BIPED:
  case MOVE_QUAD:
  case MOVE_NONE:
    return false;
  }
}
