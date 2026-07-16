/*
 * player_c.c -- Player cache routines
 */

#include "mux/server/platform.h"

#include "mux/database/attrs.h"
#include "mux/database/db.h"
#include "mux/server/server_api.h"
#include "mux/server/server_state.h"
#include "mux/support/alloc.h"
#include "mux/support/hash_table.h"
#include "mux/support/red_black_tree.h"
#include "mux/world/player_cache.h"

RedBlackTree pcache_tree;
PCACHE *pcache_head;

static int compare_pcache(DbRef left, DbRef right) {
  return (left > right) - (left < right);
}

void pcache_init(void) {
  pcache_tree = red_black_tree_init(
      (int (*)(void *, void *, void *))(GenericFnPtr)compare_pcache, nullptr);
  pcache_head = nullptr;
}

static void pcache_reload1(DbRef player, PCACHE *pp) {
  char *cp;

  cp = attribute_get_raw(player, A_QUEUEMAX);
  if (cp && *cp)
    pp->qmax = atoi(cp);
  else if (!is_wizard(player))
    pp->qmax = mudconf.queuemax;
  else
    pp->qmax = -1;
}

PCACHE *pcache_find(DbRef player) {
  PCACHE *pp;

  if (!is_good_obj(player) || !is_owns_others(player))
    return nullptr;

  pp = (PCACHE *)red_black_tree_find(pcache_tree, (void *)player);
  if (pp) {
    pp->cflags |= PF_REF;
    return pp;
  }
  pp = malloc(sizeof(PCACHE));
  pp->queue = 0;
  pp->cflags = PF_REF;
  pp->player = player;
  pcache_reload1(player, pp);
  pp->next = pcache_head;
  pcache_head = pp;
  red_black_tree_insert(pcache_tree, (void *)player, (void *)pp);
  return pp;
}

void pcache_reload(DbRef player) {
  PCACHE *pp;

  pp = pcache_find(player);
  if (!pp)
    return;
  pcache_reload1(player, pp);
}

static void pcache_save(PCACHE *pp) {
  IBUF tbuf;

  if (pp->cflags & PF_DEAD)
    return;
  if (pp->cflags & PF_QMAX_CH) {
    snprintf(tbuf, sizeof(tbuf), "%d", pp->qmax);
    attribute_add_raw(pp->player, A_QUEUEMAX, tbuf);
  }
  pp->cflags &= ~PF_QMAX_CH;
}

void pcache_trim(void) {
  PCACHE *pp, *pplast, *ppnext;
  return;

  pp = pcache_head;
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
        pcache_head = ppnext;
      if (!(pp->cflags & PF_DEAD)) {
        pcache_save(pp);
        red_black_tree_delete(pcache_tree, (void *)pp->player);
      }
      free(pp);
      pp = ppnext;
    }
  }
}

void pcache_sync(void) {
  PCACHE *pp;

  pp = pcache_head;
  while (pp) {
    pcache_save(pp);
    pp = pp->next;
  }
}

int queue_adjust(DbRef player, int adj) {
  PCACHE *pp;

  if (is_owns_others(player)) {
    pp = pcache_find(player);
    if (pp)
      pp->queue += adj;
    return pp->queue;
  }
  return 0;
}

void queue_set(DbRef player, int val) {
  PCACHE *pp;

  if (is_owns_others(player)) {
    pp = pcache_find(player);
    if (pp)
      pp->queue = val;
  }
}

int queue_maximum(DbRef player) {
  PCACHE *pp;
  int m;

  m = 0;
  if (is_owns_others(player)) {
    pp = pcache_find(player);
    if (pp) {
      if (pp->qmax >= 0) {
        m = pp->qmax;
      } else {
        m = mudstate.db_top + 1;
        if (m < mudconf.queuemax)
          m = mudconf.queuemax;
      }
    }
  }
  return m;
}
