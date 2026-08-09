
/* Implements compact encoding for map hexes. */

/* Simple coding scheme to reduce space used by map hexes to 1 byte/hex */

/* NOTE: if we _ever_ use more than 255 terrain/elevation combinations,
   this code becomes a bomb. */

#include "map_coding.h"

#include <assert.h>
#include <limits.h>
#include <stdlib.h>

#include "coding_registry.h"
#include "map_coding_api.h"
#include "mux/support/checked_storage.h"

static unsigned char *map_coding_id_slot(MapCodingRegistry *registry,
                                         char terrain, char elevation) {
  if (terrain < 0 || elevation < 0)
    abort();
  unsigned char (*terrain_row)[MAP_CODING_ELEVATION_COUNT] =
      checked_storage_at(registry->data_to_id, MAP_CODING_TERRAIN_COUNT,
                         sizeof(*registry->data_to_id), (size_t)terrain);
  return checked_storage_at(*terrain_row, MAP_CODING_ELEVATION_COUNT,
                            sizeof(**terrain_row), (size_t)elevation);
}

static MapCodingEntry *map_coding_entry(MapCodingRegistry *registry,
                                        int index) {
  if (index < 0)
    abort();
  return checked_storage_at(registry->id_to_data, MAP_CODING_ENTRY_COUNT,
                            sizeof(*registry->id_to_data), (size_t)index);
}

static const MapCodingEntry *
map_coding_entry_const(const MapCodingRegistry *registry, int index) {
  if (index < 0)
    abort();
  return checked_storage_at_const(registry->id_to_data, MAP_CODING_ENTRY_COUNT,
                                  sizeof(*registry->id_to_data), (size_t)index);
}

int map_coding_get_index(MapCodingRegistry *registry, char terrain,
                         char elevation) {
  int i;

  unsigned char *id_slot = map_coding_id_slot(registry, terrain, elevation);
  if ((i = *id_slot))
    return i - 1;
  assert(registry->next_id < UCHAR_MAX);
  *map_coding_entry(registry, registry->next_id) = (MapCodingEntry){
      .terrain = terrain,
      .elevation = elevation,
  };
  registry->next_id++;
  *id_slot = (unsigned char)registry->next_id;
  return registry->next_id - 1;
}

char map_coding_get_elevation(const MapCodingRegistry *registry, int index) {
  return map_coding_entry_const(registry, index)->elevation;
}

char map_coding_get_terrain(const MapCodingRegistry *registry, int index) {
  return map_coding_entry_const(registry, index)->terrain;
}
