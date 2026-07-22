/* object_list.h - Intrusive singly linked list operations for game objects. */

#pragma once

#include "mux/objects/db.h"

DbRef insert_first(GameDatabase *database, DbRef head, DbRef thing);
DbRef remove_first(GameDatabase *database, DbRef head, DbRef thing);
DbRef reverse_list(GameDatabase *database, DbRef list);
int member(GameDatabase *database, DbRef thing, DbRef list);
