#pragma once

#include "mech_api_types.h"

void mech_position_reset_origin(Mech *mech);
int mech_position_x(const Mech *mech);
int mech_position_y(const Mech *mech);
int mech_position_z(const Mech *mech);
char mech_position_terrain(const Mech *mech);
void mech_position_xy_set(Mech *mech, int x, int y);
void mech_position_real_xy_set(Mech *mech, float x, float y);
void mech_position_terrain_set(Mech *mech, char terrain);
void mech_position_elevation_set(Mech *mech, int elevation);
void mech_position_z_set(Mech *mech, int z);
void mech_position_land_if_flying(Mech *mech);
