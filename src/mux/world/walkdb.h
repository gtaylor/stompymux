/** @file
 * Database-wide search and statistics helper interface.
 */
#pragma once

#include "mux/server/platform.h"
#include "mux/support/alloc.h"

struct ObjectList; // IWYU pragma: keep

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

/** Initializes object list. @param[out] list List. */

void object_list_initialize(ObjectList *list);
/** Destroys object list. @param[in,out] list List. */

void object_list_destroy(ObjectList *list);
/** Adds object list. @param[in,out] list List. @param[in] item Item. */

void object_list_add(ObjectList *list, DbRef item);
/** Executes object list first. @param[in,out] list List. */

DbRef object_list_first(ObjectList *list);
/** Executes object list next. @param[in,out] list List. */

DbRef object_list_next(ObjectList *list);
