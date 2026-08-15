
/* Implements a generic first-in, first-out queue. */

#include "mux/support/fifo.h"
#include "mux/support/checked_storage.h"

#include <stdlib.h>

static void check_fifo(Fifo **foo) {
  if ((*foo) == nullptr) {
    (*foo) = checked_storage_allocate(sizeof(Fifo));
    (*foo)->first = nullptr;
    (*foo)->last = nullptr;
    (*foo)->count = 0;
  }
}

int fifo_length(Fifo **foo) {
  check_fifo(foo);
  return (*foo)->count;
}

void *fifo_pop(Fifo **foo) {
  void *tmpd;
  FifoEntry *tmp;

  check_fifo(foo);
  tmp = (*foo)->last;
  /* Is the list empty? */
  if (tmp != nullptr) {
    /* Are we removeing the only element? */
    if ((*foo)->first == (*foo)->last) {
      (*foo)->first = nullptr;
      (*foo)->last = nullptr;
    } else {
      tmp->prev->next = nullptr;
      (*foo)->last = tmp->prev;
      /* Are we going down to only one element? */
      if ((*foo)->last->prev == nullptr)
        (*foo)->first = (*foo)->last;
    }
    (*foo)->count--;
    tmpd = tmp->data;
    free(tmp);
    return tmpd;
  }
  return nullptr;
}

void fifo_push(Fifo **foo, void *data) {
  FifoEntry *tmp;

  check_fifo(foo);
  tmp = checked_storage_allocate(sizeof(FifoEntry));
  tmp->data = data;
  tmp->next = (*foo)->first;
  tmp->prev = nullptr;
  (*foo)->count++;
  if ((*foo)->first == nullptr) {
    (*foo)->first = tmp;
    (*foo)->last = tmp;
  } else {
    (*foo)->first->prev = tmp;
  }
  (*foo)->first = tmp;
}

void fifo_traverse_reverse(Fifo **foo, FifoVisitor visitor, void *context) {
  FifoEntry *tmp;

  check_fifo(foo);
  for (tmp = (*foo)->last; tmp != nullptr; tmp = tmp->prev)
    visitor(&(FifoVisit){.item = tmp->data, .context = context});
}
