#include "mech_position_api.h"

#include "map_terrain.h"

#include <stdlib.h>

#include "checked_conversion.h"
#include "floatsim.h"
#include "map_terrain.h"
#include "mech_internal.h"
#include "mech_runtime_api.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"

void mech_position_reset_origin(Mech *mech) {
  mech->pd.last_x = 0;
  mech->pd.last_y = 0;
  mech->pd.x = 0;
  mech->pd.y = 0;
}

void mech_position_previous_capture(Mech *mech) {
  mech->pd.last_x = mech->pd.x;
  mech->pd.last_y = mech->pd.y;
}

void mech_position_hex_sync_from_real(Mech *mech) {
  RealCoordToMapCoord(&mech->pd.x, &mech->pd.y, mech->pd.fx, mech->pd.fy);
}

int mech_position_x(const Mech *mech) { return mech->pd.x; }

int mech_position_y(const Mech *mech) { return mech->pd.y; }

int mech_position_previous_x(const Mech *mech) { return mech->pd.last_x; }

int mech_position_previous_y(const Mech *mech) { return mech->pd.last_y; }

int mech_position_z(const Mech *mech) { return mech->pd.z; }

int mech_position_elevation(const Mech *mech) {
  return mech_hex_elevation_get(mech);
}

int mech_position_elevation_magnitude(const Mech *mech) {
  return mech_hex_elevation_magnitude_get(mech);
}

int mech_position_surface_elevation(Mech *mech) {
  return mech_hex_surface_elevation_get(mech);
}

DbRef mech_repair_stall_dbref(const Mech *mech) { return mech->pd.stall; }

float mech_position_real_x(const Mech *mech) { return mech->pd.fx; }

float mech_position_real_y(const Mech *mech) { return mech->pd.fy; }

float mech_position_real_z(const Mech *mech) { return mech->pd.fz; }

float mech_motion_vector_x(const Mech *mech) { return mech->rd.startfx; }

float mech_motion_vector_y(const Mech *mech) { return mech->rd.startfy; }

float mech_motion_vector_z(const Mech *mech) { return mech->rd.startfz; }

float mech_jump_length(const Mech *mech) { return mech->rd.jumplength; }

float mech_jump_end_real_z(const Mech *mech) { return mech->rd.endfz; }

int mech_jump_apex_elevation(const Mech *mech) { return mech->rd.jumptop; }

float mech_range_to(const Mech *mech, const Mech *target) {
  return FindRange(mech->pd.fx, mech->pd.fy, mech->pd.fz, target->pd.fx,
                   target->pd.fy, target->pd.fz);
}

float mech_vertical_speed(const Mech *mech) { return mech->rd.verticalspeed; }

int mech_heading_degrees(const Mech *mech) {
  return float_simulation_to_short(mech->pd.facing);
}

int mech_heading_fixed(const Mech *mech) { return mech->pd.facing; }

int mech_heading_fixed_difference(const Mech *mech) {
  return mech->pd.facing - short_to_float_simulation(mech->rd.desiredfacing);
}

bool mech_heading_changed(const Mech *mech) {
  return mech->rd.critstatus & CHEAD;
}

int mech_desired_heading_degrees(const Mech *mech) {
  return mech->rd.desiredfacing;
}

int mech_turret_heading_degrees(const Mech *mech) {
  return mech->rd.turretfacing;
}

int mech_jump_destination_x(const Mech *mech) { return mech->rd.goingx; }

int mech_jump_destination_y(const Mech *mech) { return mech->rd.goingy; }

int mech_jump_heading_degrees(const Mech *mech) { return mech->rd.jumpheading; }

int mech_desired_angle(const Mech *mech) { return mech->rd.angle; }

int mech_lateral_movement(const Mech *mech) { return mech->rd.lateral; }

void mech_lateral_movement_set(Mech *mech, int lateral_movement) {
  mech->rd.lateral = clamp_int_to_short(lateral_movement);
}

int mech_dropship_bearing_sector(const Mech *mech) {
  return ((mech_heading_degrees(mech) + 30) / 60) % 6;
}

int mech_unusable_weapon_arcs(const Mech *mech) {
  return mech->pd.unusable_arcs;
}

float mech_desired_speed(const Mech *mech) { return mech->rd.desired_speed; }

char mech_position_terrain(const Mech *mech) {
  return mech_hex_terrain_get(mech);
}

void mech_position_xy_set(Mech *mech, int x, int y) {
  mech->pd.x = clamp_int_to_short(x);
  mech->pd.last_x = clamp_int_to_short(x);
  mech->pd.y = clamp_int_to_short(y);
  mech->pd.last_y = clamp_int_to_short(y);
}

void mech_position_real_xy_set(Mech *mech, float x, float y) {
  mech->pd.fx = x;
  mech->pd.fy = y;
}

void mech_position_real_xy_translate(Mech *mech, float delta_x, float delta_y) {
  mech->pd.fx += delta_x;
  mech->pd.fy += delta_y;
}

void mech_position_real_z_set(Mech *mech, float z) { mech->pd.fz = z; }

void mech_position_real_z_translate(Mech *mech, float delta_z) {
  mech->pd.fz += delta_z;
}

void mech_position_z_set(Mech *mech, int z) {
  mech->pd.z = clamp_int_to_short(z);
  mech->pd.fz = (float)ZSCALE * (float)z;
}

void mech_position_hex_z_set(Mech *mech, int z) {
  mech->pd.z = clamp_int_to_short(z);
}

void mech_position_real_z_sync(Mech *mech) {
  mech->pd.fz = (float)ZSCALE * (float)mech->pd.z;
}

void mech_desired_speed_set(Mech *mech, float speed) {
  mech->rd.desired_speed = speed;
}

void mech_desired_heading_set(Mech *mech, int heading) {
  mech->rd.desiredfacing = clamp_int_to_short(AcceptableDegree(heading));
}

void mech_heading_set(Mech *mech, int heading) {
  mech->pd.facing =
      clamp_int_to_short(short_to_float_simulation(AcceptableDegree(heading)));
}

void mech_heading_fixed_set(Mech *mech, int heading) {
  mech->pd.facing = clamp_int_to_short(heading);
}

void mech_heading_rotate_toward_desired(Mech *mech, int fixed_offset) {
  int difference = mech_heading_fixed_difference(mech);
  if (difference < 0)
    difference += short_to_float_simulation(360);

  if (difference > short_to_float_simulation(180)) {
    mech->pd.facing += fixed_offset;
    if (mech_heading_degrees(mech) >= 360)
      mech->pd.facing = mech_heading_degrees(mech) % 360;
    difference += fixed_offset;
    if (difference >= short_to_float_simulation(360))
      mech->pd.facing =
          clamp_int_to_short(short_to_float_simulation(mech->rd.desiredfacing));
  } else {
    mech->pd.facing -= fixed_offset;
    if (mech->pd.facing < 0)
      mech->pd.facing += short_to_float_simulation(360);
    difference -= fixed_offset;
    if (difference < 0)
      mech->pd.facing =
          clamp_int_to_short(short_to_float_simulation(mech->rd.desiredfacing));
  }

  mech->rd.critstatus |= CHEAD;
  MarkForLOSUpdate(mech);
}

void mech_heading_change_clear(Mech *mech) { mech->rd.critstatus &= ~CHEAD; }

void mech_turret_heading_absolute_set(Mech *mech, int heading) {
  mech->rd.turretfacing = clamp_int_to_short(
      AcceptableDegree(heading - mech_heading_degrees(mech)));
}

void mech_turret_heading_relative_set(Mech *mech, int heading) {
  mech->rd.turretfacing = clamp_int_to_short(AcceptableDegree(heading));
}

int mech_turret_heading_absolute(const Mech *mech) {
  return AcceptableDegree(mech->rd.turretfacing + mech_heading_degrees(mech));
}

void mech_desired_angle_set(Mech *mech, int angle) {
  mech->rd.angle = clamp_int_to_short(angle);
}

void mech_vertical_speed_set(Mech *mech, float speed) {
  mech->rd.verticalspeed = speed;
}

void mech_motion_vector_reset(Mech *mech) {
  mech->rd.startfx = 0.0F;
  mech->rd.startfy = 0.0F;
  mech->rd.startfz = 0.0F;
}

void mech_motion_vector_xy_set(Mech *mech, float x, float y) {
  mech->rd.startfx = x;
  mech->rd.startfy = y;
}

void mech_motion_vector_set(Mech *mech, float x, float y, float z) {
  mech->rd.startfx = x;
  mech->rd.startfy = y;
  mech->rd.startfz = z;
}

void mech_jump_destination_y_set(Mech *mech, int destination_y) {
  mech->rd.goingy = clamp_int_to_short(destination_y);
}

void mech_fall_heading_apply(Mech *mech, int offset) {
  mech->pd.facing += short_to_float_simulation(offset);
  mech->pd.facing = clamp_int_to_short(short_to_float_simulation(
      AcceptableDegree(float_simulation_to_short(mech->pd.facing))));
  mech->rd.desiredfacing =
      clamp_int_to_short(float_simulation_to_short(mech->pd.facing));
}

void mech_jump_apex_elevation_set(Mech *mech, int elevation) {
  mech->rd.jumptop = clamp_int_to_char(elevation);
}

void mech_jump_launch(Mech *mech, const MechJumpLaunch *launch) {
  mech->rd.cocoon = 0;
  mech->rd.jumpheading = clamp_int_to_short(launch->heading);
  mech->rd.status |= JUMPING;
  mech->rd.startfx = mech->pd.fx;
  mech->rd.startfy = mech->pd.fy;
  mech->rd.startfz = mech->pd.fz;
  mech->rd.jumplength = clamp_float_to_short(launch->distance);
  mech->rd.goingx = clamp_int_to_short(launch->destination_x);
  mech->rd.goingy = clamp_int_to_short(launch->destination_y);
  mech->rd.endfz = (float)ZSCALE * (float)launch->destination_elevation;
  mech->rd.jumptop = clamp_int_to_char(launch->apex_elevation);
  mech->rd.speed = 0.0F;
  mech->rd.swarming = -1;
}

bool mech_jump_destination_was_overshot(const Mech *mech) {
  return mech_is_jumping(mech) && mech->pd.last_x == mech->rd.goingx &&
         mech->pd.last_y == mech->rd.goingy &&
         (mech->pd.x != mech->pd.last_x || mech->pd.y != mech->pd.last_y);
}

void mech_jump_overshoot_restore(Mech *mech, float delta_x, float delta_y) {
  mech->pd.fx -= delta_x;
  mech->pd.fy -= delta_y;
  mech->pd.fz = mech->rd.endfz;
  mech->pd.x = mech->rd.goingx;
  mech->pd.y = mech->rd.goingy;
  MapCoordToRealCoord(mech->pd.x, mech->pd.y, &mech->pd.fx, &mech->pd.fy);
  mech->pd.z = clamp_float_to_short(mech->pd.fz / (float)ZSCALE);
}

void mech_position_mirror(Mech *target, const Mech *source, int height_offset) {
  target->pd.fx = source->pd.fx;
  target->pd.fy = source->pd.fy;
  target->pd.fz = source->pd.fz + (float)height_offset * (float)ZSCALE;
  target->pd.x = source->pd.x;
  target->pd.y = source->pd.y;
  target->pd.z = clamp_int_to_short(source->pd.z + height_offset);
  target->pd.last_x = source->pd.last_x;
  target->pd.last_y = source->pd.last_y;
}

void mech_position_land_if_flying(Mech *mech) {
  bool const is_dropship =
      mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS;
  bool const is_flying =
      mech->ud.type == CLASS_AERO || is_dropship || mech->ud.move == MOVE_VTOL;
  if (!(mech->rd.status & LANDED) && is_flying)
    mech->rd.status |= LANDED;
}

void mech_position_rollback(Mech *mech, float delta_x, float delta_y,
                            int previous_z, int previous_terrain,
                            int previous_elevation) {
  mech->pd.fx -= delta_x;
  mech->pd.fy -= delta_y;
  mech->pd.x = mech->pd.last_x;
  mech->pd.y = mech->pd.last_y;
  mech->pd.z = clamp_int_to_short(previous_z);
  mech->pd.fz = (float)previous_z * (float)ZSCALE;
  (void)previous_terrain;
  (void)previous_elevation;
}
