/* help.h - Help-index loading and player help command declarations. */

#pragma once

#include "mux/server/platform.h"
#include "mux/support/hash_table.h"

int helpindex_read(HashTable *table, char *filename);
void helpindex_load(DbRef player);
void helpindex_init(void);

#ifndef MKINDX
void help_write(DbRef, char *, HashTable *, char *, int);
int helpindex_read(HashTable *, char *);
#endif
constexpr int LINE_SIZE = 400;

constexpr int TOPIC_NAME_LEN = 30;

typedef struct {
  long pos;                       /* index into help file */
  int len;                        /* length of help entry */
  char topic[TOPIC_NAME_LEN + 1]; /* topic of help entry */
} help_indx;
