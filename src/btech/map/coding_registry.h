#pragma once

enum {
  MAP_CODING_TERRAIN_COUNT = 128,
  MAP_CODING_ELEVATION_COUNT = 10,
  MAP_CODING_ENTRY_COUNT = 256,
};

typedef struct MapCodingEntry {
  char terrain;
  char elevation;
} MapCodingEntry;

typedef struct MapCodingRegistry {
  unsigned char data_to_id[MAP_CODING_TERRAIN_COUNT]
                          [MAP_CODING_ELEVATION_COUNT];
  MapCodingEntry id_to_data[MAP_CODING_ENTRY_COUNT];
  int next_id;
} MapCodingRegistry;
