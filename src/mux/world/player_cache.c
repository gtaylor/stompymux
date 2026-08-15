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

struct PlayerCache {
  RedBlackTree tree;
  PCACHE *head;
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
  PCACHE *entry;

  if (cache == nullptr)
    return;
  while ((entry = cache->head) != nullptr) {
    cache->head = entry->next;
    free(entry);
  }
  red_black_tree_destroy(cache->tree);
  free(cache);
}

PCACHE *pcache_find(PlayerCache *cache, DbRef player) {
  PCACHE *pp;

  if (!is_good_obj(cache->database, player) ||
      !is_player(cache->database, player))
    return nullptr;

  pp = (PCACHE *)red_black_tree_find(cache->tree, (void *)player);
  if (pp) {
    pp->cflags |= PF_REF;
    return pp;
  }
  pp = checked_storage_allocate(sizeof(PCACHE));
  pp->queue = 0;
  pp->cflags = PF_REF;
  pp->player = player;
  pp->next = cache->head;
  cache->head = pp;
  red_black_tree_insert(cache->tree, (void *)player, (void *)pp);
  return pp;
}

void pcache_trim(PlayerCache *cache) {
  PCACHE *pp;
  PCACHE *pplast;
  PCACHE *ppnext;
  return;

  pp = cache->head;
  pplast = nullptr;
  while (pp) {
    if (!(pp->cflags & PF_DEAD) && (pp->queue || (pp->cflags & PF_REF))) {
      pp->cflags &= ~PF_REF;
      pplast = pp;
      pp = pp->next;
    } else {
      ppnext = pp->next;
      if (pplast)
        pplast->next = ppnext;
      else
        cache->head = ppnext;
      if (!(pp->cflags & PF_DEAD))
        red_black_tree_delete(cache->tree, (void *)pp->player);
      free(pp);
      pp = ppnext;
    }
  }
}

int queue_adjust(const PlayerQueueAdjustment *adjustment) {
  PlayerCache *cache = adjustment->cache;
  DbRef player = adjustment->player;
  PCACHE *pp;

  if (is_player(cache->database, player)) {
    pp = pcache_find(cache, player);
    if (pp)
      pp->queue += adjustment->delta;
    return pp->queue;
  }
  return 0;
}

void queue_set(const PlayerQueueAssignment *assignment) {
  PlayerCache *cache = assignment->cache;
  DbRef player = assignment->player;
  PCACHE *pp;

  if (is_player(cache->database, player)) {
    pp = pcache_find(cache, player);
    if (pp)
      pp->queue = assignment->value;
  }
}

int queue_maximum(PlayerCache *cache, DbRef player) {
  return is_player(cache->database, player)
             ? cache->configuration->command_queue_limit
             : 0;
}
