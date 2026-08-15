#include "map_name_api.h"

#include "map.h"
#include "mux/server/platform.h"

void battle_map_name_set(BattleMap *map, const char *name) {
  (void)string_copy_bounded(map->mapname, sizeof(map->mapname), name);
}
