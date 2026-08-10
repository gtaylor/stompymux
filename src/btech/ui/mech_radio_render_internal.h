#pragma once

#include "mech_api_types.h"

#include <stdbool.h>

typedef struct RadioColorRequest {
  char *buffer;
  Mech *mech;
  int channel;
  bool observer;
  int team;
} RadioColorRequest;
void radio_color_code(const RadioColorRequest *request);
