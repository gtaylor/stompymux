/* object_list.h - Intrusive singly linked list operations for game objects. */

#pragma once

#include "mux/database/db.h"

DbRef insert_first(DbRef head, DbRef thing);
DbRef remove_first(DbRef head, DbRef thing);
DbRef reverse_list(DbRef list);
int member(DbRef thing, DbRef list);
