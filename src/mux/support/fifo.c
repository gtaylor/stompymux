
/* Implements a generic first-in, first-out queue. */

#include "mux/support/fifo.h"

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
    } else {
      tmp->prev->next = nullptr;
      PFOO->last = tmp->prev;
      /* Are we going down to only one element? */
      if (PFOO->last->prev == nullptr)
        PFOO->first = PFOO->last;
    }
    PFOO->count--;
    tmpd = tmp->data;
    free(tmp);
    return tmpd;
  }
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

void fifo_traverse_reverse(Fifo **foo, FifoVisitor visitor, void *context) {
  FifoEntry *tmp;

  check_fifo(foo);
  for (tmp = PFOO->last; tmp != nullptr; tmp = tmp->prev)
    visitor(&(FifoVisit){.item = tmp->data, .context = context});
}
