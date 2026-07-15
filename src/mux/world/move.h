/* move.h - Object movement and enter-command helper interface. */

#pragma once

#include "mux/database/db.h"

void move_object(DbRef thing, DbRef destination);
void move_via_generic(DbRef player, DbRef thing, DbRef destination, int key);
void move_via_exit(DbRef player, DbRef thing, DbRef exit, DbRef destination,
                   int key);
int move_via_teleport(DbRef player, DbRef thing, DbRef destination, int key);
void move_exit(DbRef player, DbRef exit, int key, const char *name, int quiet);
void do_enter_internal(DbRef player, DbRef target, int key);
