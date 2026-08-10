
/* Declares a generic first-in, first-out queue interface. */

#pragma once

typedef struct FifoEntry {
  void *data;
  struct FifoEntry *next;
  struct FifoEntry *prev;
} FifoEntry;

typedef struct Fifo {
  FifoEntry *first; /* First entry (last put in) */
  FifoEntry *last;  /* Last entry (first to get out) */
  int count;        /* Number of entries in the queue */
} Fifo;

typedef struct FifoVisit {
  void *item;
  void *context;
} FifoVisit;

typedef void (*FifoVisitor)(const FifoVisit *visit);

/* Fifo.c */
int fifo_length(Fifo **foo);
void *fifo_pop(Fifo **foo);
void fifo_push(Fifo **foo, void *data);
void fifo_traverse_reverse(Fifo **foo, FifoVisitor visitor, void *context);
