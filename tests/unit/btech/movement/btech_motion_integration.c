#include "mech_motion_integration_api.h"

#include <assert.h>
#include <math.h>
#include <stddef.h>

#include "aero_move_api.h"
#include "equipment_types.h"
#include "map_conditions_api.h"
#include "map_coordinates.h"
#include "map_terrain.h"
#include "mech_charge_tracking_api.h"
#include "mech_classification_api.h"
#include "mech_hex_transition_api.h"
#include "mech_ice_api.h"
#include "mech_move_api.h"
#include "mech_notify_api.h"
#include "mech_position_api.h"
#include "mech_runtime_api.h"
#include "mech_specification_api.h"
#include "mech_update_api.h"
#include "mech_utils_api.h"

struct Mech {
  MechMovementType movement_type;
  bool jumping;
  bool landed;
  int z;
  float current_speed;
  int heading;
  int lateral;
  float real_x;
  float real_y;
  float real_z;
  float vector_x;
  float vector_y;
  float vector_z;
  float vertical_speed;
  float jump_speed;
  float jump_length;
};

struct BattleMap {
  float movement_modifier;
};

static int charge_record_count;
static float charge_delta_x;
static float charge_delta_y;
static int jump_land_count;

MechMovementType mech_movement_type(const Mech *mech) {
  return mech->movement_type;
}

float mech_jump_speed(const Mech *mech) { return mech->jump_speed; }

bool mech_is_under_gravity(const Mech *mech [[maybe_unused]]) { return false; }

int battle_map_gravity(const BattleMap *map [[maybe_unused]]) { return 100; }

float battle_map_movement_modifier(const BattleMap *map) {
  return map->movement_modifier;
}

int mech_position_z(const Mech *mech) { return mech->z; }

bool mech_is_jumping(const Mech *mech) { return mech->jumping; }

bool mech_is_landed(const Mech *mech) { return mech->landed; }

float mech_current_speed(const Mech *mech) { return mech->current_speed; }

int mech_heading_degrees(const Mech *mech) { return mech->heading; }

int mech_lateral_movement(const Mech *mech) { return mech->lateral; }

float mech_position_real_x(const Mech *mech) { return mech->real_x; }

float mech_position_real_y(const Mech *mech) { return mech->real_y; }

float mech_position_real_z(const Mech *mech) { return mech->real_z; }

float mech_motion_vector_x(const Mech *mech) { return mech->vector_x; }

float mech_motion_vector_y(const Mech *mech) { return mech->vector_y; }

float mech_motion_vector_z(const Mech *mech) { return mech->vector_z; }

float mech_vertical_speed(const Mech *mech) { return mech->vertical_speed; }

float mech_jump_length(const Mech *mech) { return mech->jump_length; }

float mech_jump_end_real_z(const Mech *mech [[maybe_unused]]) { return 0.0F; }

int mech_jump_apex_elevation(const Mech *mech [[maybe_unused]]) { return 0; }

int mech_jump_heading_degrees(const Mech *mech) { return mech->heading; }

int mech_position_x(const Mech *mech [[maybe_unused]]) { return 0; }

int mech_position_y(const Mech *mech [[maybe_unused]]) { return 0; }

int mech_jump_destination_x(const Mech *mech [[maybe_unused]]) { return 100; }

int mech_jump_destination_y(const Mech *mech [[maybe_unused]]) { return 100; }

char mech_real_terrain_get(Mech *mech [[maybe_unused]]) {
  return BATTLE_TERRAIN_GRASSLAND;
}

MapRealPosition map_vector_components(const MapPolarVector *vector) {
  const float RADIANS = (float)M_PI / 180.0F * (float)vector->bearing;
  return (MapRealPosition){
      .x = vector->magnitude * cosf(RADIANS),
      .y = vector->magnitude * sinf(RADIANS),
  };
}

void mech_position_real_xy_translate(Mech *mech, float delta_x, float delta_y) {
  mech->real_x += delta_x;
  mech->real_y += delta_y;
}

void mech_position_real_z_translate(Mech *mech, float delta_z) {
  mech->real_z += delta_z;
}

void mech_position_real_z_set(Mech *mech, float z) { mech->real_z = z; }

void mech_position_hex_z_set(Mech *mech, int z) { mech->z = z; }

void mech_position_real_xy_set(Mech *mech, MapRealPosition position) {
  mech->real_x = position.x;
  mech->real_y = position.y;
}

void mech_motion_vector_xy_set(Mech *mech, MapRealPosition vector) {
  mech->vector_x = vector.x;
  mech->vector_y = vector.y;
}

void mech_position_z_set(Mech *mech, int z) { mech->z = z; }

void mark_for_los_update(Mech *mech [[maybe_unused]]) {}

int collision_check(const MovementCollisionCheck *check [[maybe_unused]]) {
  return 0;
}

void mech_notify(Mech *mech [[maybe_unused]],
                 MechNotifyAudience audience [[maybe_unused]],
                 const char *message [[maybe_unused]]) {}

void mech_los_broadcast(Mech *mech [[maybe_unused]],
                        const char *message [[maybe_unused]]) {}

void mech_fall(Mech *mech [[maybe_unused]], int levels [[maybe_unused]],
               bool show_message [[maybe_unused]]) {}

void mech_jump_land(Mech *mech) {
  jump_land_count++;
  mech->jumping = false;
  mech->landed = true;
}

void break_thru_ice(Mech *mech [[maybe_unused]]) {}

void drop_thru_ice(Mech *mech [[maybe_unused]]) {}

bool mech_is_dropship(const Mech *mech [[maybe_unused]]) { return false; }

int mech_desired_angle(const Mech *mech [[maybe_unused]]) { return 0; }

void mech_desired_angle_set(Mech *mech [[maybe_unused]],
                            int angle [[maybe_unused]]) {}

bool dropship_notification_is_due(Mech *mech [[maybe_unused]]) { return false; }

void dropship_land_warning(Mech *mech [[maybe_unused]],
                           int serious [[maybe_unused]]) {}

void map_coord_to_real_coord(int x [[maybe_unused]], int y [[maybe_unused]],
                             float *real_x, float *real_y) {
  *real_x = 0.0F;
  *real_y = 0.0F;
}

void mech_charge_distance_record(Mech *mech [[maybe_unused]], float delta_x,
                                 float delta_y) {
  charge_record_count++;
  charge_delta_x = delta_x;
  charge_delta_y = delta_y;
}

static void test_ground_step(void) {
  BattleMap map = {.movement_modifier = 0.8F};
  Mech mech = {.movement_type = MOVE_BIPED,
               .z = 3,
               .current_speed = 20.0F,
               .heading = 0,
               .lateral = 30,
               .real_x = 10.0F,
               .real_y = 20.0F};
  MechMotionStep step = {.delta_x = -1.0F, .delta_y = -1.0F};

  charge_record_count = 0;
  assert(mech_motion_integrate(&mech, &map, &step));
  const float DISTANCE = 20.0F * (float)MOVE_MOD * 0.8F;
  assert(fabsf(step.delta_x - DISTANCE * cosf((float)M_PI / 6.0F)) < 0.0001F);
  assert(fabsf(step.delta_y - DISTANCE * sinf((float)M_PI / 6.0F)) < 0.0001F);
  assert(step.previous_z == 3);
  assert(step.update_surface);
  assert(charge_record_count == 1);
  assert(fabsf(charge_delta_x - step.delta_x) < 0.0001F);
  assert(fabsf(charge_delta_y - step.delta_y) < 0.0001F);
}

static void test_stopped_and_invalid_jump(void) {
  BattleMap map = {.movement_modifier = 1.0F};
  Mech mech = {.movement_type = MOVE_TRACK, .z = 2};
  MechMotionStep step = {.update_surface = true};

  assert(!mech_motion_integrate(&mech, &map, &step));
  assert(step.previous_z == 2);
  assert(!step.update_surface);

  mech.movement_type = MOVE_BIPED;
  mech.jumping = true;
  mech.jump_speed = 10.0F;
  mech.jump_length = 0.0F;
  mech.real_x = 4.0F;
  mech.real_y = 5.0F;
  jump_land_count = 0;
  assert(!mech_motion_integrate(&mech, &map, &step));
  assert(jump_land_count == 1);
  assert(!mech.jumping);
  assert(mech.landed);
  assert(mech.real_x == 4.0F);
  assert(mech.real_y == 5.0F);
  assert(isfinite(mech.real_x) && isfinite(mech.real_y));
}

static void test_submersible_and_hull_steps(void) {
  BattleMap map = {.movement_modifier = 1.0F};
  Mech mech = {.movement_type = MOVE_SUB,
               .current_speed = 10.0F,
               .heading = 0,
               .vertical_speed = 4.0F,
               .real_z = 5.0F,
               .z = 1};
  MechMotionStep step;
  assert(mech_motion_integrate(&mech, &map, &step));
  assert(fabsf(mech.real_z - 7.0F) < 0.0001F);
  assert(mech.z == 0);

  mech.movement_type = MOVE_HULL;
  mech.current_speed = 10.0F;
  mech.real_z = 9.0F;
  assert(mech_motion_integrate(&mech, &map, &step));
  assert(mech.z == 0);
  assert(mech.real_z == 9.0F);
}

int main(void) {
  test_ground_step();
  test_stopped_and_invalid_jump();
  test_submersible_and_hull_steps();
  return 0;
}
