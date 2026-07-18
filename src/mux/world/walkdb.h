/* walkdb.h - Database-wide search, statistics, and ownership helper interface.
 */

#pragma once

#include "mux/database/db.h"

typedef struct ObjectListBlock ObjectListBlock;
struct ObjectListBlock {
  ObjectListBlock *next;
  DbRef data[(LBUF_SIZE - sizeof(ObjectListBlock *)) / sizeof(DbRef)];
};

typedef struct ObjectList ObjectList;
struct ObjectList {
  ObjectListBlock *head;
  ObjectListBlock *tail;
  ObjectListBlock *cursor_block;
  int count;
  int cursor_index;
};

int chown_all(GameDatabase *database, DbRef from_player, DbRef to_player);
void object_list_initialize(ObjectList *list);
void object_list_destroy(ObjectList *list);
void object_list_add(ObjectList *list, DbRef object);
DbRef object_list_first(ObjectList *list);
DbRef object_list_next(ObjectList *list);
