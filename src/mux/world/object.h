/* object.h - Low-level object creation, deletion, and parent operations. */

#pragma once

#include "mux/database/db.h"

DbRef start_home(void);
DbRef default_home(void);
int can_set_home(DbRef player, DbRef thing, DbRef home);
DbRef new_home(DbRef player);
DbRef clone_home(DbRef player, DbRef thing);

DbRef create_obj(DbRef player, int object_type, char *name);
void destroy_obj(DbRef player, DbRef object);
void divest_object(DbRef object);
void empty_obj(DbRef object);
void destroy_exit(DbRef exit);
void destroy_thing(DbRef thing);
void destroy_player(DbRef player);
