#include "mech_position_api.h"

#include "mech_internal.h"
#include "mech_status_types.h"

void mech_position_reset_origin(Mech *mech) {
  mech->pd.last_x = 0;
  mech->pd.last_y = 0;
  mech->pd.x = 0;
  mech->pd.y = 0;
}

int mech_position_x(const Mech *mech) { return mech->pd.x; }

int mech_position_y(const Mech *mech) { return mech->pd.y; }

int mech_position_z(const Mech *mech) { return mech->pd.z; }

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
