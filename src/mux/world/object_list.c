/* object_list.c - Intrusive singly linked list operations for game objects. */

#include "mux/world/object_list.h"

#include "mux/server/platform.h"

DbRef insert_first(DbRef head, DbRef thing) {
  s_next(thing, head);
  return thing;
}

/*
 * Removes first object from a list
 */
DbRef remove_first(DbRef head, DbRef thing) {
  DbRef prev;

  if (head == thing)
    return (obj_next(thing));

  DOLIST(prev, head) {
    if (obj_next(prev) == thing) {
      s_next(prev, obj_next(thing));
      return head;
    }
  }
  return head;
}

/**
 * Reverse the order of members in a list.
 */
DbRef reverse_list(DbRef list) {
  DbRef newlist, rest;

  newlist = NOTHING;
  while (list != NOTHING) {
    rest = obj_next(list);
    s_next(list, newlist);
    newlist = list;
    list = rest;
  }
  return newlist;
}

/**
 * Indicate if thing is in list
 */
int member(DbRef thing, DbRef list) {
  DOLIST(list, list) {
    if (list == thing)
      return 1;
  }
  return 0;
}
