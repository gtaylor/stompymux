/* object_spatial.h - Object containment, location, and exit visibility
 * interface. */

#pragma once

#include "mux/database/db.h"

DbRef where_is(DbRef object);
DbRef where_room(DbRef object);
int locatable(DbRef player, DbRef object, DbRef cause);
int nearby(DbRef player, DbRef object);
int exit_visible(DbRef exit, DbRef player, int key);
int exit_displayable(DbRef exit, DbRef player, int key);
