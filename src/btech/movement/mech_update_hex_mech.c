/* Implements BattleTech movement mechanics for unit update hex mech. */

#include "mech_hex_transition_api.h"

#include <math.h>
#include <stdlib.h>

#include "btech/context.h"
#include "btech_event.h"
#include "btechstats_api.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "map_terrain.h"
#include "mech_classification_api.h"
#include "mech_condition_api.h"
#include "mech_crew_api.h"
#include "mech_events_api.h"
#include "mech_identity_api.h"
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

static bool mech_passes_cliff_check(Mech *mech, bool skid_cliff) {
  int modifier =
      skid_cliff
          ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
          : clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) / MP1) / 3;
  return mech_pilot_skill_roll_without_experience(&(PilotSkillRollRequest){
      .mech = mech, .modifier = modifier, .succeed_when_fallen = true});
}

HexTransitionResult
mech_hex_transition_resolve(const HexMechTransitionInput *input) {
  Mech *mech = input->mech;
  BattleMap *mech_map = input->map;
  float deltax = input->delta_x;
  float deltay = input->delta_y;
  int elevation = input->elevation;
  int lastelevation = input->last_elevation;
  int oldterrain = input->old_terrain;
  int le;
  int ed;
  int avoidbth;
  int done = 0;
  BtechContext *context = mech_context(mech);
  bool skid_cliff = btech_context_uses_skid_cliff_rules(context);
  MechConditionSummary condition = mech_condition_summary(mech);

  switch (mech_movement_type(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:

    if (mech_is_jumping(mech)) {

      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER)
        return (HexTransitionResult){.stop = true, .done = done};

      /* Did we hit something while jumping */
      if (collision_check(&(MovementCollisionCheck){.mech = mech,
                                                    .mode = JUMP,
                                                    .previous_elevation = 0,
                                                    .previous_terrain = 0})) {

        ed = 1 + mech_position_z(mech) -
             battle_map_hex_elevation(mech_map, mech_position_x(mech),
                                      mech_position_y(mech));
        ed = ed > 1 ? ed : 1;
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = deltay},
                                    .previous_z = lastelevation});
        mech_notify(mech, MECHALL,
                    "[bold]You attempt to jump over elevation that is too "
                    "high![reset]");
        if (mech_has_active_pilot(mech) &&
            made_pilot_skill_roll(
                mech, clamp_float_to_int(mech_position_real_z(mech) /
                                         (float)ZSCALE / 3.0F))) {

          mech_notify(mech, MECHALL, "[bold]You land safely.[reset]");
          mech_jump_land(mech);

        } else {

          mech_notify(mech, MECHALL,
                      "[bold]You crash into the obstacle and fall from the "
                      "sky![reset]");
          mech_los_broadcast(
              mech, "crashes into an obstacle and falls from the sky!");
          mech_fall(mech, ed, 0);
          mech_domino_resolve(mech, MECH_DOMINO_FALL);
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Walked into a wall silly */
    if (collision_check(
            &(MovementCollisionCheck){.mech = mech,
                                      .mode = WALK_WALL,
                                      .previous_elevation = lastelevation,
                                      .previous_terrain = oldterrain})) {

      mech_position_rollback(
          &(MechPositionRollback){.mech = mech,
                                  .delta = {.x = deltax, .y = deltay},
                                  .previous_z = lastelevation});
      mech_notify(mech, MECHALL,
                  "You attempt to climb a hill too steep for you.");

      if (mech_pilot_dbref(mech) == -1 ||
          mech_passes_cliff_check(mech, skid_cliff)) {

        mech_notify(mech, MECHALL, "You manage to stop before crashing.");
        mech_los_broadcast(mech, "stops suddenly to avoid a cliff!");

      } else {

        mech_notify(mech, MECHALL,
                    "You run headlong into the cliff and fall down!");
        mech_los_broadcast(mech, "runs headlong into a cliff and falls down!");
        if (!skid_cliff)
          mech_fall(mech,
                    (int)(1 + (mech_current_speed(mech) * MP_PER_KPH)) / 4, 0);
        else
          mech_fall(mech, 1, 0);
      }
      mech_movement_stop(mech);
      mech_position_z_set(mech, lastelevation);
      return (HexTransitionResult){.stop = true, .done = done};
    }
    if (collision_check(
            &(MovementCollisionCheck){.mech = mech,
                                      .mode = WALK_DROP,
                                      .previous_elevation = lastelevation,
                                      .previous_terrain = oldterrain})) {

      /* Walked off a cliff ... */
      mech_notify(mech, MECHALL, "You notice a large drop in front of you");
      avoidbth = skid_cliff
                     ? mech_skid_modifier(fabsf(mech_current_speed(mech)) / MP1)
                     : clamp_float_to_int(
                           fabsf(mech_current_speed(mech) + MP1) / MP1) /
                           3;

      if (mech_pilot_dbref(mech) == -1 ||
          (!condition.auto_fall &&
           mech_pilot_skill_roll_without_experience(
               &(PilotSkillRollRequest){.mech = mech,
                                        .modifier = avoidbth,
                                        .succeed_when_fallen = true}))) {

        mech_notify(mech, MECHALL, "You manage to stop before falling off.");
        mech_los_broadcast(mech,
                           "stops suddenly to avoid falling off a cliff!");
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = deltay},
                                    .previous_z = lastelevation});

      } else {

        mech_notify(mech, MECHALL,
                    "You run off the cliff and fall to the ground below.");
        mech_los_broadcast(mech,
                           "runs off a cliff and falls to the ground below!");
        mech_fall(mech, lastelevation - elevation, 0);
        mech_movement_stop(mech);
      }
      mech_movement_stop(mech);
      return (HexTransitionResult){.stop = true, .done = done};
    }
    if (btech_context_requires_backwalk_rolls(context) &&
        (mech_current_speed(mech) < 0) &&
        (collision_check(
            &(MovementCollisionCheck){.mech = mech,
                                      .mode = WALK_BACK,
                                      .previous_elevation = lastelevation,
                                      .previous_terrain = oldterrain}))) {

      mech_printf(mech, MECHALL, "You notice a %s behind you!",
                  (elevation > lastelevation ? "small incline" : "small drop"));

      if (mech_pilot_dbref(mech) == -1 ||
          (made_pilot_skill_roll(mech,
                                 collision_check(&(MovementCollisionCheck){
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

        mech_los_broadcast(mech,
                           elevation > lastelevation
                               ? "falls on its back walking up an incline."
                               : "falls off the back of a small incline.");
        mech_fall(mech, abs(lastelevation - elevation), 1);
        mech_movement_stop(mech);
        if (elevation > lastelevation) {
          mech_position_rollback(
              &(MechPositionRollback){.mech = mech,
                                      .delta = {.x = deltax, .y = deltay},
                                      .previous_z = lastelevation});
        }
      }
      return (HexTransitionResult){.stop = true, .done = done};
    }

    /* Slow the unit if its made an elevation change */
    le = elevation - lastelevation;
    le = (le < 0) ? -le : le;
    if (mech_position_z(mech) != elevation)
      le = 0;
    if (le > 0) {
      deltax = (le == 1) ? MP1 : MP2;
      float speed = mech_current_speed(mech);
      if (speed > 0) {
        speed -= deltax;
        mech_current_speed_set(mech, speed < 0 ? 0 : speed);
      } else if (speed < 0) {
        speed += deltax;
        mech_current_speed_set(mech, speed > 0 ? 0 : speed);
      }
    }

    if (mech_class(mech) == CLASS_BSUIT) {

      /* Are they in water, also make sure it affects them */
      if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
          (mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER ||
           (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
            (lastelevation < (elevation - 1)))) &&
          elevation < 0) {

        mech_notify(mech, MECHALL,
                    "You notice a body of water in front of you");

        if (mech_pilot_dbref(mech) == -1 ||
            made_pilot_skill_roll(
                mech, clamp_float_to_int(fabsf(mech_current_speed(mech) + MP1) /
                                         MP1) /
                          3)) {

          mech_notify(mech, MECHALL, "You manage to stop before falling in.");
          mech_los_broadcast(mech, "stops suddenly to avoid going for a swim!");
        } else {

          mech_notify(mech, MECHALL,
                      "You trip at the edge of the water and plunge in...");
          mech_flood(mech);
          return (HexTransitionResult){.stop = true, .done = done};
        }
        mech_position_rollback(
            &(MechPositionRollback){.mech = mech,
                                    .delta = {.x = deltax, .y = deltay},
                                    .previous_z = lastelevation});
        mech_movement_stop(mech);
        return (HexTransitionResult){.stop = true, .done = done};
      }

    } else if (!(mech_technology_flags_secondary(mech) & WATERPROOF_TECH) &&
               ((mech_real_terrain_get(mech) == BATTLE_TERRAIN_WATER &&
                 mech_position_z(mech) < 0) ||
                (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
                 mech_position_z(mech) < 0) ||
                (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE &&
                 mech_position_z(mech) < 0) ||
                mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER) &&
               mech_class(mech) != CLASS_MW) {

      int skillmod;
      int dammod;
      float walking_speed = 2.0F * mech_effective_maximum_speed(mech) / 3.0F;
      if (mech_desired_speed(mech) > walking_speed)
        mech_desired_speed_set(mech, walking_speed);
      if (mech_current_speed(mech) >
          (2.0F * mech_effective_maximum_speed(mech) / 3.0F) + 0.1F) {
        mech_notify(mech, MECHPILOT,
                    "You struggle to keep control as you run into the water!");
        skillmod = 2;
        dammod = 2;
      } else {
        mech_notify(mech, MECHPILOT,
                    "You use your piloting skill "
                    "to maneuver through the water.");
        skillmod = 0;
        dammod = 0;
      }
      int terrain_modifier = mech_position_elevation(mech) - 2;
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_HIGH_WATER)
        terrain_modifier = -2;
      else if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE)
        terrain_modifier = bridge_w_elevation(mech);
      else if (mech_position_elevation(mech) > 3)
        terrain_modifier = 1;
      skillmod += terrain_modifier;
      //
      // Stupid Frontiers cheaters. No XP gains here.
      if (!mech_pilot_skill_roll_without_experience(
              &(PilotSkillRollRequest){.mech = mech, .modifier = skillmod})) {
        mech_notify(mech, MECHALL, "You slip in the water and fall down");
        mech_los_broadcast(mech, "slips in the water and falls down!");
        mech_fall(mech, 1, dammod);
        done = 1;
      }
    }
    break;
  case MOVE_TRACK:
  case MOVE_WHEEL:
  case MOVE_HOVER:
  case MOVE_VTOL:
  case MOVE_HULL:
  case MOVE_FOIL:
  case MOVE_FLY:
  case MOVE_SUB:
  case MOVE_NONE:
    break;
  }
  return (HexTransitionResult){.done = done};
}
