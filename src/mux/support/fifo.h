/** @file
 * Declares a generic first-in, first-out queue interface.
 */
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
/** Executes fifo length. @param[in,out] foo Foo. */

int fifo_length(Fifo **foo);
/** Pops fifo. @param[in,out] foo Foo. */

void *fifo_pop(Fifo **foo);
/** Pushes fifo. @param[in,out] foo Foo. @param[in,out] data Caller-provided
 * data. */

void fifo_push(Fifo **foo, void *data);
/** Executes fifo traverse reverse. @param[in,out] foo Foo. @param[in] visitor
 * Visitor. @param[in,out] context Operation context. */

void fifo_traverse_reverse(Fifo **foo, FifoVisitor visitor, void *context);
