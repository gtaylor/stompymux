/* player_cache.c -- Player-cache lifecycle tests. */

#include <stddef.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/server/server_config.h"
#include "mux/world/player_cache.h"

bool is_good_obj(GameDatabase *database, DbRef x) {
  return x >= 0 && x < database->top &&
         typeof_obj(database, x) != OBJECT_TYPE_INVALID &&
         typeof_obj(database, x) != OBJECT_TYPE_NOTYPE;
}

int main(void) {
  GameObject objects[4] = {};
  GameDatabase database = {
      .object_storage = objects,
      .top = 3,
      .size = 3,
  };
  const ServerConfiguration CONFIGURATION = {.command_queue_limit = 100};
  PlayerCache *cache;

  game_object_set_type(&database, 0, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 1, OBJECT_TYPE_PLAYER);
  game_object_set_type(&database, 2, OBJECT_TYPE_THING);

  cache = player_cache_create(&CONFIGURATION, &database);
  if (cache == nullptr)
    return 1;

  queue_set(&(PlayerQueueAssignment){.cache = cache, .player = 0, .value = 1});
  queue_set(&(PlayerQueueAssignment){.cache = cache, .player = 1, .value = 0});
  queue_set(&(PlayerQueueAssignment){.cache = cache, .player = 2, .value = 9});
  if (player_cache_size(cache) != 2 || queue_maximum(cache, 0) != 100 ||
      queue_maximum(cache, 2) != 0) {
    player_cache_destroy(cache);
    return 1;
  }

  player_cache_trim(cache);
  if (player_cache_size(cache) != 2) {
    player_cache_destroy(cache);
    return 1;
  }

  player_cache_trim(cache);
  if (player_cache_size(cache) != 1) {
    player_cache_destroy(cache);
    return 1;
  }

  if (queue_adjust(&(PlayerQueueAdjustment){
          .cache = cache, .player = 1, .delta = 0}) != 0 ||
      player_cache_size(cache) != 2) {
    player_cache_destroy(cache);
    return 1;
  }
  player_cache_trim(cache);
  if (player_cache_size(cache) != 2) {
    player_cache_destroy(cache);
    return 1;
  }
  player_cache_trim(cache);
  if (player_cache_size(cache) != 1) {
    player_cache_destroy(cache);
    return 1;
  }

  if (queue_adjust(&(PlayerQueueAdjustment){
          .cache = cache, .player = 1, .delta = 1}) != 1 ||
      player_cache_size(cache) != 2 ||
      queue_adjust(&(PlayerQueueAdjustment){
          .cache = cache, .player = 2, .delta = 1}) != 0 ||
      player_cache_size(cache) != 2) {
    player_cache_destroy(cache);
    return 1;
  }

  player_cache_trim(cache);
  player_cache_trim(cache);
  if (player_cache_size(cache) != 2) {
    player_cache_destroy(cache);
    return 1;
  }

  player_cache_destroy(cache);
  return 0;
}
