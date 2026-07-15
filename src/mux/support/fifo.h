
/*
 * Author: Markus Stenberg <fingon@iki.fi>
 *
 *  Copyright (c) 1996 Markus Stenberg
 *       All rights reserved
 *
 * Created: Sun Dec  1 11:46:22 1996 fingon
 * Last modified: Sun Dec  1 12:43:04 1996 fingon
 *
 */

/* fifo.h - Generic first-in, first-out queue interface. */

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

/* Fifo.c */
int fifo_length(Fifo **foo);
void *fifo_pop(Fifo **foo);
void fifo_push(Fifo **foo, void *data);
void fifo_traverse_reverse(Fifo **foo, void (*func)(void *));
