/* Declares the BattleTech unit sensor API. */

#pragma once

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

typedef struct SensorFlagText {
  char text[MBUF_SIZE];
} SensorFlagText;

typedef struct MechSensorToHitRequest {
  Mech *observer;
  Mech *target;
  int los_flags;
  int map_light;
  int ammunition_mode;
} MechSensorToHitRequest;

typedef struct MechSensorObservationRequest {
  Mech *observer;
  Mech *target;
  int *los_flags;
  int arc;
  float range;
  int map_visibility;
  int map_light;
  int cloud_base;
} MechSensorObservationRequest;

typedef struct MechSensorDetectionRequest {
  Mech *observer;
  Mech *target;
  int los_flags;
  int arc;
  float range;
  int sensor;
  int chance_divisor;
  int map_visibility;
  int map_light;
} MechSensorDetectionRequest;

typedef struct MechSensorVisibilityRequest {
  Mech *observer;
  unsigned short los_flags;
  float range;
  int x;
  int y;
  Mech *target;
  int map_visibility;
  int map_light;
  int cloud_base;
  int notification_level;
  int previous_visibility;
} MechSensorVisibilityRequest;

typedef struct MechSensorDescriptionRequest {
  char *buffer;
  size_t capacity;
  Mech *mech;
  int sensor;
  bool verbose;
} MechSensorDescriptionRequest;

/* mech.sensor.c */
int mech_sensor_to_hit_bonus(const MechSensorToHitRequest *request);
int mech_sensor_can_see(const MechSensorObservationRequest *request);
int mech_sensor_driver_base_chance(Mech *mech);
bool mech_sensor_detects(const MechSensorDetectionRequest *request);
int mech_sensor_detects_now(const MechSensorDetectionRequest *request);
SensorFlagText sensor_flag_text(int flags);
unsigned short
mech_sensor_visibility_update(const MechSensorVisibilityRequest *request);
void mech_sensor_map_los_update(DbRef obj, BattleMap *map);
void mech_sensor_description_append(
    const MechSensorDescriptionRequest *request);
char *mech_sensor_info(Mech *mech, char buffer[static LBUF_SIZE]);
bool mech_sensor_can_change_to(Mech *mech, int sensor);
void mech_sensors_disable_requiring(Mech *mech, int technology);
void sensor_light_availability_check(Mech *mech);
void mech_sensor(DbRef player, void *data, char *buffer);
void mech_sensor_visibility_refresh(Mech *mech);
typedef struct SensorScrambleRequest {
  Mech *source;
  int duration;
  int chance;
  const char *infrared_message;
  const char *light_amplification_message;
} SensorScrambleRequest;
void mech_sensors_scramble_infrared_and_liteamp(
    const SensorScrambleRequest *request);
