/* Shared isolated BattleTech LOS fixture for sensor unit tests. */

#pragma once

#include "map.h"
#include "mech_api_types.h"
#include "mech_condition_api.h"
#include "section_types.h"

#include <stdbool.h>

enum { LOS_FIXTURE_WIDTH = 80, LOS_FIXTURE_HEIGHT = 60 };

struct Mech {
  int x;
  int y;
  int z;
  DbRef dbref;
  int technology;
  int sensors[2];
  int map_slot;
  int weapon_arc;
  UnitClass unit_class;
  MechMovementType movement;
  char terrain;
  bool fallen;
  bool dropship;
  bool clairvoyant;
  bool jellied;
  bool searchlight;
  bool partial_cover;
  MechConditionSummary condition;
};

void los_fixture_reset(BattleMap *map);
void los_fixture_set_hex(int x, int y, char terrain, char real_terrain,
                         int elevation);
/* Fills the inclusive vertical line from y_start through y_end at x. */
void los_fixture_fill_line(int x, int y_start, int y_end, char terrain,
                           char real_terrain, int elevation);
Mech los_fixture_make_mech(int x, int y, int z);
void los_fixture_map_unit_set(int index, Mech *unit);
int los_fixture_channel_error_count(void);
void los_fixture_sensor_can_see_result_set(int result);
int los_fixture_sensor_observation_visibility(void);
int los_fixture_sensor_observation_light(void);
int los_fixture_sensor_observation_cloud_base(void);
void los_fixture_sensor_set(int sensor, int maximum_visibility,
                            bool full_vision, int wood_limit, int water_limit,
                            bool sees_mountains);
int los_fixture_flags(BattleMap *map, Mech *observer, Mech *target);
int los_fixture_flags_to_hex(BattleMap *map, Mech *observer, int x, int y,
                             float range);

int bounded(int lower, int value, int upper);
