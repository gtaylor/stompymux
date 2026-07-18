/* object_list.c - Intrusive singly linked list operations for game objects. */

#include "mux/world/object_list.h"

#include "mux/server/platform.h"

DbRef insert_first(GameDatabase *database, DbRef head, DbRef thing) {
  game_object_set_next(database, thing, head);
  return thing;
}

/*
 * Removes first object from a list
 */
DbRef remove_first(GameDatabase *database, DbRef head, DbRef thing) {
  DbRef prev;

  if (head == thing)
    return game_object_next(database, thing);

  for (prev = head; prev != NOTHING && game_object_next(database, prev) != prev;
       prev = game_object_next(database, prev)) {
    if (game_object_next(database, prev) == thing) {
      game_object_set_next(database, prev, game_object_next(database, thing));
      return head;
    }
  }
  return head;
}

/**
 * Reverse the order of members in a list.
 */
DbRef reverse_list(GameDatabase *database, DbRef list) {
  DbRef newlist, rest;

  newlist = NOTHING;
  while (list != NOTHING) {
    rest = game_object_next(database, list);
    game_object_set_next(database, list, newlist);
    newlist = list;
    list = rest;
  }
  return newlist;
}

/**
 * Indicate if thing is in list
 */
int member(GameDatabase *database, DbRef thing, DbRef list) {
  for (; list != NOTHING && game_object_next(database, list) != list;
       list = game_object_next(database, list)) {
    if (list == thing)
      return 1;
  }
  return 0;
}
