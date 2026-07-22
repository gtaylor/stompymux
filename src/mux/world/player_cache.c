/*
 * player_c.c -- Player cache routines
 */

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/server/server_api.h"
#include "mux/server/server_config.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/red_black_tree.h"
#include "mux/world/player_cache.h"

struct PlayerCache {
  RedBlackTree tree;
  PCACHE *head;
  const ServerConfiguration *configuration;
  GameDatabase *database;
};

static int compare_pcache(DbRef left, DbRef right) {
  return (left > right) - (left < right);
}

PlayerCache *player_cache_create(const ServerConfiguration *configuration,
                                 GameDatabase *database) {
  PlayerCache *cache = calloc(1, sizeof(*cache));

  if (cache == nullptr)
    return nullptr;
  cache->configuration = configuration;
  cache->database = database;
  cache->tree = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)compare_pcache, nullptr);
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
  pcache_sync(cache);
  while ((entry = cache->head) != nullptr) {
    cache->head = entry->next;
    free(entry);
  }
  red_black_tree_destroy(cache->tree);
  free(cache);
}

static void pcache_reload1(PlayerCache *cache, DbRef player, PCACHE *pp) {
  char *cp;

  cp = attribute_get_raw(cache->database, player, A_QUEUEMAX);
  if (cp && *cp)
    pp->qmax = atoi(cp);
  else if (!is_wizard(cache->database, player))
    pp->qmax = cache->configuration->queuemax;
  else
    pp->qmax = -1;
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
  pp = malloc(sizeof(PCACHE));
  pp->queue = 0;
  pp->cflags = PF_REF;
  pp->player = player;
  pcache_reload1(cache, player, pp);
  pp->next = cache->head;
  cache->head = pp;
  red_black_tree_insert(cache->tree, (void *)player, (void *)pp);
  return pp;
}

void pcache_reload(PlayerCache *cache, DbRef player) {
  PCACHE *pp;

  pp = pcache_find(cache, player);
  if (!pp)
    return;
  pcache_reload1(cache, player, pp);
}

static void pcache_save(PlayerCache *cache, PCACHE *pp) {
  IBUF tbuf;

  if (pp->cflags & PF_DEAD)
    return;
  if (pp->cflags & PF_QMAX_CH) {
    snprintf(tbuf, sizeof(tbuf), "%d", pp->qmax);
    attribute_add_raw(cache->database, pp->player, A_QUEUEMAX, tbuf);
  }
  pp->cflags &= ~PF_QMAX_CH;
}

void pcache_trim(PlayerCache *cache) {
  PCACHE *pp, *pplast, *ppnext;
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
      if (!(pp->cflags & PF_DEAD)) {
        pcache_save(cache, pp);
        red_black_tree_delete(cache->tree, (void *)pp->player);
      }
      free(pp);
      pp = ppnext;
    }
  }
}

void pcache_sync(PlayerCache *cache) {
  PCACHE *pp;

  pp = cache->head;
  while (pp) {
    pcache_save(cache, pp);
    pp = pp->next;
  }
}

int queue_adjust(PlayerCache *cache, DbRef player, int adj) {
  PCACHE *pp;

  if (is_player(cache->database, player)) {
    pp = pcache_find(cache, player);
    if (pp)
      pp->queue += adj;
    return pp->queue;
  }
  return 0;
}

void queue_set(PlayerCache *cache, DbRef player, int val) {
  PCACHE *pp;

  if (is_player(cache->database, player)) {
    pp = pcache_find(cache, player);
    if (pp)
      pp->queue = val;
  }
}

int queue_maximum(PlayerCache *cache, DbRef player) {
  PCACHE *pp;
  int m;

  m = 0;
  if (is_player(cache->database, player)) {
    pp = pcache_find(cache, player);
    if (pp) {
      if (pp->qmax >= 0) {
        m = pp->qmax;
      } else {
        m = cache->database->top + 1;
        if (m < cache->configuration->queuemax)
          m = cache->configuration->queuemax;
      }
    }
  }
  return m;
}
