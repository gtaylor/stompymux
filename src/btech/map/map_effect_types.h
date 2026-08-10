/* Defines requests for effects applied to a map hex. */

#pragma once

#include <stdbool.h>

#include "map_coordinates.h"
#include "mech_api_types.h"

typedef struct TerrainHexEffectRequest {
  Mech *mech;
  MapHexPosition position;
  bool intentional;
} TerrainHexEffectRequest;
