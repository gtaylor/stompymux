#include "mech_position_api.h"

#include <stdlib.h>

#include "floatsim.h"
#include "mech_internal.h"
#include "mech_status_types.h"
#include "mech_utils_api.h"

void mech_position_reset_origin(Mech *mech) {
  mech->pd.last_x = 0;
  mech->pd.last_y = 0;
  mech->pd.x = 0;
  mech->pd.y = 0;
}

int mech_position_x(const Mech *mech) { return mech->pd.x; }

int mech_position_y(const Mech *mech) { return mech->pd.y; }

int mech_position_z(const Mech *mech) { return mech->pd.z; }

int mech_position_elevation_magnitude(const Mech *mech) {
  return abs(mech->pd.elev);
}

float mech_position_real_x(const Mech *mech) { return mech->pd.fx; }

float mech_position_real_y(const Mech *mech) { return mech->pd.fy; }

float mech_position_real_z(const Mech *mech) { return mech->pd.fz; }

float mech_motion_vector_x(const Mech *mech) { return mech->rd.startfx; }

float mech_motion_vector_y(const Mech *mech) { return mech->rd.startfy; }

float mech_motion_vector_z(const Mech *mech) { return mech->rd.startfz; }

float mech_range_to(const Mech *mech, const Mech *target) {
  return FindRange(mech->pd.fx, mech->pd.fy, mech->pd.fz, target->pd.fx,
                   target->pd.fy, target->pd.fz);
}

float mech_vertical_speed(const Mech *mech) { return mech->rd.verticalspeed; }

int mech_heading_degrees(const Mech *mech) { return FSIM2SHO(mech->pd.facing); }

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

float mech_desired_speed(const Mech *mech) { return mech->rd.desired_speed; }

char mech_position_terrain(const Mech *mech) { return mech->pd.terrain; }

void mech_position_xy_set(Mech *mech, int x, int y) {
  mech->pd.x = x;
  mech->pd.last_x = x;
  mech->pd.y = y;
  mech->pd.last_y = y;
}

void mech_position_real_xy_set(Mech *mech, float x, float y) {
  mech->pd.fx = x;
  mech->pd.fy = y;
}

void mech_position_terrain_set(Mech *mech, char terrain) {
  mech->pd.terrain = terrain;
}

void mech_position_elevation_set(Mech *mech, int elevation) {
  mech->pd.elev = elevation;
}

void mech_position_z_set(Mech *mech, int z) {
  mech->pd.z = z;
  mech->pd.fz = ZSCALE * z;
}

void mech_position_land_if_flying(Mech *mech) {
  bool const is_dropship =
      mech->ud.type == CLASS_DS || mech->ud.type == CLASS_SPHEROID_DS;
  bool const is_flying =
      mech->ud.type == CLASS_AERO || is_dropship || mech->ud.move == MOVE_VTOL;
  if (!(mech->rd.status & LANDED) && is_flying)
    mech->rd.status |= LANDED;
}
