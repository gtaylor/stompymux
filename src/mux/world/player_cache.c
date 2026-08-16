/*
 * player_c.c -- Player cache routines
 */

#include "mux/server/server_config.h" // IWYU pragma: keep
#include <stdlib.h>

#include "mux/objects/db.h"
#include "mux/objects/flags.h"
#include "mux/server/platform.h"
#include "mux/support/checked_storage.h"
#include "mux/support/red_black_tree.h"
#include "mux/world/player_cache.h"

typedef struct PlayerCacheEntry PlayerCacheEntry;
struct PlayerCacheEntry {
  DbRef player;
  int queue;
  bool recently_referenced;
  PlayerCacheEntry *next;
};

struct PlayerCache {
  RedBlackTree tree;
  PlayerCacheEntry *head;
  const ServerConfiguration *configuration;
  GameDatabase *database;
};

static int compare_pcache(const RedBlackTreeCompareCall *call) {
  const void *left_key = call->lhs;
  const void *right_key = call->rhs;
  const DbRef LEFT = (DbRef)left_key;
  const DbRef RIGHT = (DbRef)right_key;

  return (LEFT > RIGHT) - (LEFT < RIGHT);
}

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database) {
  PlayerCache *cache = checked_storage_try_allocate_array(1, sizeof(*cache));

  if (cache == nullptr)
    return nullptr;
  cache->configuration = configuration;
  cache->database = database;
  cache->tree = red_black_tree_init(compare_pcache, nullptr);
  if (cache->tree == nullptr) {
    free(cache);
    return nullptr;
  }
  return cache;
}

void player_cache_destroy(PlayerCache *cache) {
  PlayerCacheEntry *entry;

  if (cache == nullptr)
    return;
  while ((entry = cache->head) != nullptr) {
    cache->head = entry->next;
    free(entry);
  }
  red_black_tree_destroy(cache->tree);
  free(cache);
}

size_t player_cache_size(const PlayerCache *cache) {
  return (size_t)red_black_tree_size(cache->tree);
}

static PlayerCacheEntry *player_cache_find(PlayerCache *cache, DbRef player) {
  PlayerCacheEntry *entry;

  if (!is_good_obj(cache->database, player) ||
      !is_player(cache->database, player))
    return nullptr;

  entry = red_black_tree_find(cache->tree, (void *)player);
  if (entry) {
    entry->recently_referenced = true;
    return entry;
  }
  entry = checked_storage_allocate(sizeof(*entry));
  entry->player = player;
  entry->queue = 0;
  entry->recently_referenced = true;
  entry->next = cache->head;
  cache->head = entry;
  red_black_tree_insert(cache->tree, (void *)player, entry);
  return entry;
}

void player_cache_trim(PlayerCache *cache) {
  PlayerCacheEntry *entry;
  PlayerCacheEntry *previous;
  PlayerCacheEntry *next;

  entry = cache->head;
  previous = nullptr;
  while (entry) {
    if (entry->queue != 0 || entry->recently_referenced) {
      entry->recently_referenced = false;
      previous = entry;
      entry = entry->next;
    } else {
      next = entry->next;
      if (previous)
        previous->next = next;
      else
        cache->head = next;
      red_black_tree_delete(cache->tree, (void *)entry->player);
      free(entry);
      entry = next;
    }
  }
}

int queue_adjust(const PlayerQueueAdjustment *adjustment) {
  PlayerCache *cache = adjustment->cache;
  DbRef player = adjustment->player;
  PlayerCacheEntry *entry;

  if (is_player(cache->database, player)) {
    entry = player_cache_find(cache, player);
    if (entry)
      entry->queue += adjustment->delta;
    return entry ? entry->queue : 0;
  }
  return 0;
}

void queue_set(const PlayerQueueAssignment *assignment) {
  PlayerCache *cache = assignment->cache;
  DbRef player = assignment->player;
  PlayerCacheEntry *entry;

  if (is_player(cache->database, player)) {
    entry = player_cache_find(cache, player);
    if (entry)
      entry->queue = assignment->value;
  }
}

int queue_maximum(PlayerCache *cache, DbRef player) {
  return is_player(cache->database, player)
             ? cache->configuration->command_queue_limit
             : 0;
}
