#include "btech_los_test.h"

#include "map.h"
#include "map_los_api.h"
#include "map_los_types.h"
#include "map_units_api.h"
#include "mech_api_types.h"
#include "mech_identity_api.h"
#include "mux/support/checked_storage.h"

#include <stdlib.h>

struct Mech {
  int slot;
};

int mech_map_slot(const Mech *mech) { return mech->slot; }

static void map_los_allocate(BattleMap *map, int size) {
  map->dynamic_size = size;
  map->first_free = size;
  map->lo_sinfo = calloc((size_t)size, sizeof(*map->lo_sinfo));
  if (!map->lo_sinfo)
    abort();
  for (int i = 0; i < size; ++i) {
    unsigned short **row = checked_storage_at(map->lo_sinfo, (size_t)size,
                                              sizeof(*map->lo_sinfo), (size_t)i);
    *row = calloc((size_t)size, sizeof(**row));
    if (!*row)
      abort();
  }
}

static void map_los_destroy(BattleMap *map) {
  for (int i = 0; i < map->dynamic_size; ++i) {
    unsigned short **row =
        checked_storage_at(map->lo_sinfo, (size_t)map->dynamic_size,
                           sizeof(*map->lo_sinfo), (size_t)i);
    free(*row);
  }
  free(map->lo_sinfo);
  map->lo_sinfo = nullptr;
}

int main(void) {
  LosTestState state = {0};
  BattleMap map = {0};
  Mech first = {.slot = 0};
  Mech second = {.slot = 1};
  map_los_allocate(&map, 3);

  unsigned short forward = BATTLE_MAP_LOS_SEEN | BATTLE_MAP_LOS_BLOCKED |
                           (unsigned short)(3 * BATTLE_MAP_LOS_WOOD) |
                           (unsigned short)(2 * BATTLE_MAP_LOS_WATER);
  battle_map_los_flags_set(&map, 0, 1, forward);
  battle_map_los_flags_set(&map, 1, 0, BATTLE_MAP_LOS_SEEN_PRIMARY);
  los_expect_true(&state, "seen helper uses observer direction",
                  battle_map_unit_is_seen(&map, &first, &second));
  los_expect_true(&state, "blocked helper decodes cache flag",
                  battle_map_unit_los_is_blocked(&map, &first, &second));
  los_expect_int(&state, "cache woods count", 3,
                 battle_map_unit_los_wood_count(&map, &first, &second));
  los_expect_int(&state, "cache water count", 2,
                 battle_map_unit_los_water_count(&map, &first, &second));
  los_expect_int(&state, "reverse cache is independent",
                 BATTLE_MAP_LOS_SEEN_PRIMARY, battle_map_los_flags(&map, 1, 0));

  battle_map_los_flags_set(&map, 0, 2, BATTLE_MAP_LOS_SEEN_SECONDARY);
  battle_map_los_observer_clear(&map, 0);
  los_expect_int(&state, "observer clear removes first target", 0,
                 battle_map_los_flags(&map, 0, 1));
  los_expect_int(&state, "observer clear removes every target", 0,
                 battle_map_los_flags(&map, 0, 2));
  los_expect_int(&state, "observer clear preserves reverse direction",
                 BATTLE_MAP_LOS_SEEN_PRIMARY, battle_map_los_flags(&map, 1, 0));

  map_los_destroy(&map);
  return los_test_result(&state);
}
