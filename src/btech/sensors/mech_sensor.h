
/* Declares unit sensor interfaces. */

#pragma once

#include "btconfig.h"
#include "mech_api_types.h"
#include "mech_status_types.h"

typedef struct BattleMap BattleMap;

typedef struct SensorVisibilityRequest {
  Mech *target;
  BattleMap *map;
  int sensor;
  float range;
  int condition_range;
  int light;
} SensorVisibilityRequest;

typedef struct SensorContactRequest {
  Mech *observer;
  Mech *target;
  BattleMap *map;
  float range;
  int flags;
} SensorContactRequest;

typedef struct SensorToHitRequest {
  Mech *observer;
  Mech *target;
  BattleMap *map;
  int flags;
  int light;
} SensorToHitRequest;
/*
   For all scanners chance of seeing a foe is modified by:
   - Side arcs are 70% chance
   - Rear arc is 40% chance
 */

typedef struct SensorDefinition {
  const char *sensor_name;
  const char *match_letter;

  /* Is the sensor 360 degree with just one of them? */
  int full_vision;

  /* Longest vis this sensor brand sees */
  int maximum_visibility;

  /* Variable factor in maxvis ; it changes by +- 1 every 30 seconds */
  int maximum_variation;

  /* Function for retrieving generic chance of spotting foe with
     this scanner at the range */
  /* first int = sensor type #, second = maxrange by conditions,
     third = lightning level */
  int (*see_chance)(const SensorVisibilityRequest *request);

  /* Do we really see 'em? Mainly checks for various things that
     vary between diff. sensors (and also seechancefunc > 0) */
  int (*can_see)(const SensorContactRequest *request);

  /* Chance of actually hitting someone */
  int (*to_hit_bonus)(const SensorToHitRequest *request);

  /* If <0, not used */
  int min_light;
  int max_light;

  int required_special;
  int specials_set; /* 1 if the original specials struct, 2 if the extended */

  int attribute_check;

  const char *range_description;
  const char *block_description;
  const char *special_description;
} SensorDefinition;

int vislight_see(const SensorVisibilityRequest *request);
int liteamp_see(const SensorVisibilityRequest *request);
int infrared_see(const SensorVisibilityRequest *request);
int electrom_see(const SensorVisibilityRequest *request);
int seismic_see(const SensorVisibilityRequest *request);
int radar_see(const SensorVisibilityRequest *request);
int bap_see(const SensorVisibilityRequest *request);
int blood_see(const SensorVisibilityRequest *request);

int vislight_csee(const SensorContactRequest *request);
int liteamp_csee(const SensorContactRequest *request);
int infrared_csee(const SensorContactRequest *request);
int electrom_csee(const SensorContactRequest *request);
int seismic_csee(const SensorContactRequest *request);
int radar_csee(const SensorContactRequest *request);
int bap_csee(const SensorContactRequest *request);
int blood_csee(const SensorContactRequest *request);

int vislight_tohit(const SensorToHitRequest *request);
int liteamp_tohit(const SensorToHitRequest *request);
int infrared_tohit(const SensorToHitRequest *request);
int electrom_tohit(const SensorToHitRequest *request);
int seismic_tohit(const SensorToHitRequest *request);
int radar_tohit(const SensorToHitRequest *request);
int bap_tohit(const SensorToHitRequest *request);
int blood_tohit(const SensorToHitRequest *request);

typedef enum SensorType {
  SENSOR_VIS = 0,
  SENSOR_LA = 1,
  SENSOR_IR = 2,
  SENSOR_EM = 3,
  SENSOR_SE = 4,
  SENSOR_RA = 5,
  SENSOR_BAP = 6,
  SENSOR_BHAP = 7,
  NUM_SENSORS = 9,
} SensorType;

typedef enum SensorAttribute {
  SENSOR_ATTR_NONE,
  SENSOR_ATTR_SEISMIC,
} SensorAttribute;

const SensorDefinition *mech_sensor_definition(int sensor);
extern const SensorDefinition SENSOR_DEFINITIONS[];
