#pragma once

#include "mech_api_types.h"

typedef struct MechJumpLaunch {
  int heading;
  int destination_x;
  int destination_y;
  int destination_elevation;
  int apex_elevation;
  float distance;
} MechJumpLaunch;

void mech_position_reset_origin(Mech *mech);
int mech_position_x(const Mech *mech);
int mech_position_y(const Mech *mech);
int mech_position_previous_x(const Mech *mech);
int mech_position_previous_y(const Mech *mech);
int mech_position_z(const Mech *mech);
int mech_position_elevation(const Mech *mech);
int mech_position_elevation_magnitude(const Mech *mech);
int mech_position_surface_elevation(Mech *mech);
float mech_position_real_x(const Mech *mech);
float mech_position_real_y(const Mech *mech);
float mech_position_real_z(const Mech *mech);
float mech_motion_vector_x(const Mech *mech);
float mech_motion_vector_y(const Mech *mech);
float mech_motion_vector_z(const Mech *mech);
float mech_range_to(const Mech *mech, const Mech *target);
float mech_vertical_speed(const Mech *mech);
int mech_heading_degrees(const Mech *mech);
int mech_heading_fixed(const Mech *mech);
int mech_heading_fixed_difference(const Mech *mech);
bool mech_heading_changed(const Mech *mech);
int mech_desired_heading_degrees(const Mech *mech);
int mech_turret_heading_degrees(const Mech *mech);
int mech_jump_destination_x(const Mech *mech);
int mech_jump_destination_y(const Mech *mech);
int mech_jump_heading_degrees(const Mech *mech);
int mech_desired_angle(const Mech *mech);
int mech_lateral_movement(const Mech *mech);
int mech_dropship_bearing_sector(const Mech *mech);
float mech_desired_speed(const Mech *mech);
char mech_position_terrain(const Mech *mech);
void mech_position_xy_set(Mech *mech, int x, int y);
void mech_position_real_xy_set(Mech *mech, float x, float y);
void mech_position_real_z_set(Mech *mech, float z);
void mech_position_terrain_set(Mech *mech, char terrain);
void mech_position_elevation_set(Mech *mech, int elevation);
void mech_position_z_set(Mech *mech, int z);
void mech_position_hex_z_set(Mech *mech, int z);
void mech_position_real_z_sync(Mech *mech);
void mech_desired_speed_set(Mech *mech, float speed);
void mech_desired_heading_set(Mech *mech, int heading);
void mech_heading_set(Mech *mech, int heading);
void mech_heading_fixed_set(Mech *mech, int heading);
void mech_heading_rotate_toward_desired(Mech *mech, int fixed_offset);
void mech_heading_change_clear(Mech *mech);
void mech_turret_heading_absolute_set(Mech *mech, int heading);
void mech_turret_heading_relative_set(Mech *mech, int heading);
int mech_turret_heading_absolute(const Mech *mech);
void mech_desired_angle_set(Mech *mech, int angle);
void mech_vertical_speed_set(Mech *mech, float speed);
void mech_motion_vector_reset(Mech *mech);
void mech_jump_destination_y_set(Mech *mech, int destination_y);
void mech_fall_heading_apply(Mech *mech, int offset);
void mech_jump_apex_elevation_set(Mech *mech, int elevation);
void mech_position_mirror(Mech *target, const Mech *source, int height_offset);
void mech_position_land_if_flying(Mech *mech);
void mech_position_rollback(Mech *mech, float delta_x, float delta_y,
                            int previous_z, char previous_terrain,
                            int previous_elevation);
void mech_jump_launch(Mech *mech, const MechJumpLaunch *launch);
