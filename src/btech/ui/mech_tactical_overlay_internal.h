#pragma once

#include <stdbool.h>

typedef struct BattleMap BattleMap;
typedef struct Mech Mech;

typedef struct TacticalSketch {
  char *canvas;
  BattleMap *map;
  Mech *mech;
  int start_x;
  int start_y;
  int width;
  int height;
  int display_columns;
  int top_offset;
  int left_offset;
  int cliff_size;
  bool color;
  int labels;
} TacticalSketch;

enum {
  TACTICAL_MAX_WIDTH = 40,
  TACTICAL_MAX_HEIGHT = 24,
  TACTICAL_TOP_LABEL = 3,
  TACTICAL_LEFT_LABEL = 4,
  TACTICAL_RIGHT_LABEL = 3,
  MAP_SKETCH_CAPACITY =
      (((TACTICAL_LEFT_LABEL + 1 + (TACTICAL_MAX_WIDTH * 3) +
         TACTICAL_RIGHT_LABEL + 1) *
        (TACTICAL_TOP_LABEL + 1 + (TACTICAL_MAX_HEIGHT * 2))) +
       2) *
      5,
};

void tactical_sketch_landing_zones(const TacticalSketch *sketch);
void tactical_sketch_mines(const TacticalSketch *sketch);
