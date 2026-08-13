#include "aero_move_api.h"
#include "btconfig.h"
#include "checked_conversion.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "mech_charge_tracking_api.h"
#include "mech_classification_api.h"
#include "mech_hex_transition_api.h"
#include "mech_ice_api.h"
#include "mech_motion_integration_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"
#include "mux/server/platform.h"
#include "mux/support/alloc.h"
#include "section_types.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
static float mech_motion_jump_speed(const Mech *mech, const BattleMap *map) {
  float speed = mech_jump_speed(mech);
  if (!mech_is_under_gravity(mech) || !map)
    return speed;
  int gravity = battle_map_gravity(map);
  int const EFFECTIVE_GRAVITY = gravity > 50 ? gravity : 50;
  return speed * 100.0F / (float)EFFECTIVE_GRAVITY;
}
static float motion_hypotenuse(float x, float y) {
  return sqrtf((x * x) + (y * y));
}
static void motion_step_delta_set(MechMotionStep *step, MapPolarVector vector) {
  MapRealPosition delta = map_vector_components(&vector);
  step->delta_x = delta.x;
  step->delta_y = delta.y;
}
bool mech_motion_integrate(Mech *mech, BattleMap *map, MechMotionStep *step) {
  float jump_position;
  float target_x;
  float target_y;
#ifdef ODDJUMP
  float remaining_jump;
  float midpoint_modifier;
#endif
  char message_buffer[MBUF_SIZE];
  float movement_modifier = battle_map_movement_modifier(map);
  float jump_speed = mech_motion_jump_speed(mech, map);
  *step = (MechMotionStep){.previous_z = mech_position_z(mech)};
  switch (mech_movement_type(mech)) {
  case MOVE_BIPED:
  case MOVE_QUAD:
    if (mech_is_jumping(mech)) {
      mark_for_los_update(mech);
      motion_step_delta_set(
          step, (MapPolarVector){.magnitude = jump_speed * (float)MOVE_MOD *
                                              movement_modifier,
                                 .bearing = mech_jump_heading_degrees(mech)});
      mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
      jump_position = motion_hypotenuse(
          mech_position_real_x(mech) - mech_motion_vector_x(mech),
          mech_position_real_y(mech) - mech_motion_vector_y(mech));
#ifndef ODDJUMP
      float jump_length = mech_jump_length(mech);
      mech_position_real_z_set(
          mech,
          ((4.0F * (float)clamp_float_to_int(jump_speed * MP_PER_KPH) *
            (float)ZSCALE) /
           (jump_length * jump_length)) *
                  jump_position * (jump_length - jump_position) +
              mech_motion_vector_z(mech) +
              jump_position *
                  (mech_jump_end_real_z(mech) - mech_motion_vector_z(mech)) /
                  (jump_length * HEXLEVEL));
#else
      remaining_jump = mech_jump_length(mech) - jump_position;
      if (remaining_jump < 0.0F)
        remaining_jump = 0.0F;
      midpoint_modifier = jump_position / mech_jump_length(mech);
      midpoint_modifier = (midpoint_modifier - 0.5F) * 2.0F;
      int const JUMP_APEX_ELEVATION = mech_jump_apex_elevation(mech);
      float const JUMP_APEX_ELEVATION_FLOAT = (float)JUMP_APEX_ELEVATION;
      if (mech_jump_apex_elevation(mech) >=
          (1 + clamp_float_to_int(jump_speed * MP_PER_KPH))) {
        midpoint_modifier = (1.0F - (midpoint_modifier * midpoint_modifier)) *
                            JUMP_APEX_ELEVATION_FLOAT;
      } else {
        midpoint_modifier = (1.0F - (midpoint_modifier * midpoint_modifier *
                                     midpoint_modifier * midpoint_modifier)) *
                            JUMP_APEX_ELEVATION_FLOAT;
      }
      mech_position_real_z_set(mech,
                               (((remaining_jump * mech_motion_vector_z(mech)) +
                                 (jump_position * mech_jump_end_real_z(mech))) /
                                mech_jump_length(mech)) +
                                   (midpoint_modifier * (float)ZSCALE));
#endif
      mech_position_hex_z_set(
          mech, clamp_float_to_int(
                    (mech_position_real_z(mech) / (float)ZSCALE) + 0.5F));
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_BRIDGE &&
          collision_check(&(MovementCollisionCheck){.mech = mech,
                                                    .mode = JUMP,
                                                    .previous_elevation = 0,
                                                    .previous_terrain = 0}) &&
          mech_position_z(mech) > 0) {
        mech_notify(mech, MECHALL, "CRASH! You crash into the bridge!");
        mech_los_broadcast(mech, "crashes into the bridge!");
        mech_fall(mech, 1, 0);
        return false;
      }
      if (mech_position_x(mech) == mech_jump_destination_x(mech) &&
          mech_position_y(mech) == mech_jump_destination_y(mech)) {
        map_coord_to_real_coord(mech_position_x(mech), mech_position_y(mech),
                                &target_x, &target_y);
#ifdef ODDJUMP
        if (motion_hypotenuse(target_x - mech_motion_vector_x(mech),
                              target_y - mech_motion_vector_y(mech)) <=
            motion_hypotenuse(
                mech_position_real_x(mech) - mech_motion_vector_x(mech),
                mech_position_real_y(mech) - mech_motion_vector_y(mech))) {
          mech_jump_land(mech);
          mech_position_real_xy_set(
              mech, (MapRealPosition){.x = target_x, .y = target_y});
        }
#else
        mech_jump_land(mech);
        mech_position_real_xy_set(
            mech, (MapRealPosition){.x = target_x, .y = target_y});
#endif
      }
      if (mech_real_terrain_get(mech) == BATTLE_TERRAIN_ICE) {
        if (step->previous_z < -1 && mech_position_z(mech) >= -1)
          break_thru_ice(mech);
        else if (step->previous_z >= -1 && mech_position_z(mech) < -1)
          drop_thru_ice(mech);
      }
    } else if (fabsf(mech_current_speed(mech)) > 0.0F) {
      motion_step_delta_set(
          step,
          (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                        (float)MOVE_MOD * movement_modifier,
                           .bearing = mech_lateral_movement(mech) +
                                      mech_heading_degrees(mech)});
      mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
      step->update_surface = true;
      mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    } else {
      return false;
    }
    break;
  case MOVE_TRACK:
  case MOVE_WHEEL:
    if (fabsf(mech_current_speed(mech)) <= 0.0F)
      return false;
#ifndef BT_MOVEMENT_MODES
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_heading_degrees(mech)});
#else
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_lateral_movement(mech) +
                                          mech_heading_degrees(mech)});
#endif
    mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
    step->update_surface = true;
    mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    break;
  case MOVE_HOVER:
    if (fabsf(mech_current_speed(mech)) <= 0.0F)
      return false;
#ifndef BT_MOVEMENT_MODES
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_heading_degrees(mech)});
#else
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_lateral_movement(mech) +
                                          mech_heading_degrees(mech)});
#endif
    mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
    step->update_surface = true;
    mech_charge_distance_record(mech, step->delta_x, step->delta_y);
    break;
  case MOVE_VTOL:
    if (mech_is_landed(mech))
      return false;
    [[fallthrough]];
  case MOVE_SUB:
    mark_for_los_update(mech);
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_heading_degrees(mech)});
    mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
    mech_position_real_z_translate(mech,
                                   mech_vertical_speed(mech) * (float)MOVE_MOD);
    mech_position_hex_z_set(
        mech, clamp_float_to_int(mech_position_real_z(mech) / (float)ZSCALE));
    break;
  case MOVE_FLY:
    if (!mech_is_landed(mech)) {
      mark_for_los_update(mech);
      mech_position_real_z_translate(mech, mech_motion_vector_z(mech) *
                                               (float)MOVE_MOD);
      mech_position_hex_z_set(
          mech, clamp_float_to_int(mech_position_real_z(mech) / (float)ZSCALE));
      mech_position_real_xy_translate(
          mech, mech_motion_vector_x(mech) * (float)MOVE_MOD,
          mech_motion_vector_y(mech) * (float)MOVE_MOD);
      if (mech_is_dropship(mech)) {
        if (mech_position_z(mech) < 10 && step->previous_z >= 10) {
          dropship_land_warning(mech, 1);
        } else if (mech_position_z(mech) < 50 && step->previous_z >= 50) {
          dropship_land_warning(mech, 0);
        } else if (mech_position_z(mech) < 100 && step->previous_z >= 100) {
          if (abs(mech_desired_angle(mech)) != 90) {
            if (dropship_notification_is_due(mech)) {
              mech_notify(mech, MECHALL,
                          "As the craft enters the lower atmosphere, its nose "
                          "rises up for a clean landing..");
              (void)snprintf(message_buffer, MBUF_SIZE,
                             "starts descending towards %d, %d..",
                             mech_position_x(mech), mech_position_y(mech));
              mech_los_broadcast(mech, message_buffer);
            } else {
              mech_notify(mech, MECHALL,
                          "Due to low altitude, climbing angle set to 90 "
                          "degrees.");
            }
            mech_desired_angle_set(mech, 90);
          }
          mech_motion_vector_xy_set(mech, (MapRealPosition){0});
          dropship_land_warning(mech, -1);
        }
      }
    } else {
      if (fabsf(mech_current_speed(mech)) <= 0.0F)
        return false;
      motion_step_delta_set(
          step,
          (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                        (float)MOVE_MOD * movement_modifier,
                           .bearing = mech_heading_degrees(mech)});
      mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
      step->update_surface = true;
    }
    break;
  case MOVE_HULL:
  case MOVE_FOIL:
    if (fabsf(mech_current_speed(mech)) <= 0.0F)
      return false;
    motion_step_delta_set(
        step, (MapPolarVector){.magnitude = mech_current_speed(mech) *
                                            (float)MOVE_MOD * movement_modifier,
                               .bearing = mech_heading_degrees(mech)});
    mech_position_real_xy_translate(mech, step->delta_x, step->delta_y);
    mech_position_z_set(mech, 0);
    break;
  case MOVE_NONE:
    return false;
  }
  return true;
}
