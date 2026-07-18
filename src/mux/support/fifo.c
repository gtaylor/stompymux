
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Sun Dec  1 11:46:18 1996 fingon
 * Last modified: Sun Dec  1 12:43:01 1996 fingon
 *
 */

/* fifo.c - Generic first-in, first-out queue implementation. */

#include "mux/support/fifo.h"
#include <stdio.h>
#include <stdlib.h>

/* A little shortcut to save me some typing */
#define PFOO (*foo)

static void check_fifo(Fifo **foo) {
  if (PFOO == nullptr) {
    PFOO = malloc(sizeof(Fifo));
    PFOO->first = nullptr;
    PFOO->last = nullptr;
    PFOO->count = 0;
  }
}

int fifo_length(Fifo **foo) {
  check_fifo(foo);
  return PFOO->count;
}

void *fifo_pop(Fifo **foo) {
  void *tmpd;
  FifoEntry *tmp;

  check_fifo(foo);
  tmp = PFOO->last;
  /* Is the list empty? */
  if (tmp != nullptr) {
    /* Are we removeing the only element? */
    if (PFOO->first == PFOO->last) {
      PFOO->first = nullptr;
      PFOO->last = nullptr;
    } else
      tmp->prev->next = nullptr;
    PFOO->last = tmp->prev;
    /* Are we going down to only one element? */
    if (PFOO->last->prev == nullptr)
      PFOO->first = PFOO->last;
    PFOO->count--;
    tmpd = tmp->data;
    free(tmp);
    return tmpd;
  } else
    return nullptr;
}

void fifo_push(Fifo **foo, void *data) {
  FifoEntry *tmp;

  check_fifo(foo);
  tmp = malloc(sizeof(FifoEntry));
  tmp->data = data;
  tmp->next = PFOO->first;
  tmp->prev = nullptr;
  PFOO->count++;
  if (PFOO->first == nullptr) {
    PFOO->first = tmp;
    PFOO->last = tmp;
  } else
    PFOO->first->prev = tmp;
  PFOO->first = tmp;
}

void fifo_traverse_reverse(Fifo **foo, void (*func)(void *, void *),
                           void *context) {
  FifoEntry *tmp;

  check_fifo(foo);
  for (tmp = PFOO->last; tmp != nullptr; tmp = tmp->prev)
    func(tmp->data, context);
}
