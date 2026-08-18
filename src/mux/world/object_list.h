/** @file
 * Intrusive singly linked list operations for game objects.
 */
#pragma once

#include "mux/objects/db.h"
#include "mux/server/platform.h"

/** Executes insert first. @param[in,out] database Game database. @param[in]
 * head Head. @param[in] thing Thing. */

DbRef insert_first(GameDatabase *database, DbRef head, DbRef thing);
/** Executes remove first. @param[in,out] database Game database. @param[in]
 * head Head. @param[in] thing Thing. */

DbRef remove_first(GameDatabase *database, DbRef head, DbRef thing);
/** Executes reverse list. @param[in,out] database Game database. @param[in]
 * list List. */

DbRef reverse_list(GameDatabase *database, DbRef list);
/** Executes member. @param[in,out] database Game database. @param[in] thing
 * Thing. @param[in] list List. */

bool member(GameDatabase *database, DbRef thing, DbRef list);
